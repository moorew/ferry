//! Tauri commands (SPEC 10.7). All async; none of them touch disk on the UI thread.

use std::collections::HashMap;

use tailcopy_core::{ClipId, SearchContext, model::hash_hex};
use tauri::{AppHandle, Manager, State};

#[cfg(target_os = "linux")]
use crate::dto::ExtensionState;
use crate::dto::{self, ClipDetail, ClipRow, Settings, Status, Toast};
use crate::picker;
use crate::state::{AppState, clips_changed, status_changed};

type CmdResult<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[tauri::command]
pub async fn list_clips(
    state: State<'_, AppState>,
    query: String,
    limit: usize,
    offset: usize,
    today_start_ms: Option<u64>,
    yesterday_start_ms: Option<u64>,
) -> CmdResult<Vec<ClipRow>> {
    let ctx = SearchContext {
        today_start_ms: today_start_ms.unwrap_or(0),
        yesterday_start_ms: yesterday_start_ms.unwrap_or(0),
    };
    let limit = limit.clamp(1, 500);
    state
        .db
        .run(move |core| {
            let me = core.store.device_id().clone();
            let view = core.store.view();
            let devices: HashMap<_, _> = view
                .devices()
                .unwrap_or_default()
                .into_iter()
                .map(|d| (d.device_id.clone(), d))
                .collect();
            let rows: Vec<_> = if query.trim().is_empty() {
                core.index
                    .rows()
                    .skip(offset)
                    .take(limit)
                    .cloned()
                    .collect()
            } else {
                core.index
                    .search(Some(&view), &query, &ctx, offset + limit)?
                    .into_iter()
                    .skip(offset)
                    .map(|h| h.row)
                    .collect()
            };
            Ok(rows.iter().map(|r| dto::row(r, &me, &devices)).collect())
        })
        .await
        .map_err(|e: tailcopy_core::Error| err(e))
}

#[tauri::command]
pub async fn get_clip(state: State<'_, AppState>, id: String) -> CmdResult<Option<ClipDetail>> {
    state
        .db
        .run(move |core| {
            let me = core.store.device_id().clone();
            let id = ClipId::from(id);
            let view = core.store.view();
            let Some(row) = core.index.rows().find(|r| r.clip_id == id).cloned() else {
                return Ok(None);
            };
            let devices: HashMap<_, _> = view
                .devices()
                .unwrap_or_default()
                .into_iter()
                .map(|d| (d.device_id.clone(), d))
                .collect();
            let detail = view.get_clip(&id)?;
            let text = view.plain_text(&id)?;
            let html = view
                .read_format(&id, "text/html")?
                .map(|b| dto::sanitize_html(&String::from_utf8_lossy(&b)));
            let formats = detail
                .map(|d| d.formats.iter().map(|f| f.mime.clone()).collect())
                .unwrap_or_default();
            let image_url = row
                .image_hash
                .as_ref()
                .filter(|h| view.read_blob(h).ok().flatten().is_some())
                .map(|h| dto::thumb_url(&hash_hex(h), true));
            Ok(Some(ClipDetail {
                looks_like_code: text.as_deref().is_some_and(dto::looks_like_code),
                copied_ms: tailcopy_core::hlc::wall_ms(row.created_hlc),
                row: dto::row(&row, &me, &devices),
                text,
                html,
                image_url,
                formats,
            }))
        })
        .await
        .map_err(|e: tailcopy_core::Error| err(e))
}

#[tauri::command]
pub async fn paste(app: AppHandle, id: String, mode: String) -> CmdResult<Option<Toast>> {
    picker::choose(&app, &id, mode == "plain").await;
    Ok(None)
}

#[tauri::command]
pub async fn copy(app: AppHandle, id: String) -> CmdResult<()> {
    picker::copy(&app, &id).await;
    Ok(())
}

#[tauri::command]
pub async fn set_pinned(
    app: AppHandle,
    state: State<'_, AppState>,
    ids: Vec<String>,
    pinned: bool,
) -> CmdResult<()> {
    state
        .db
        .run(move |core| {
            for id in ids {
                core.store.set_pinned(&ClipId::from(id), pinned)?;
            }
            core.refresh();
            Ok(())
        })
        .await
        .map_err(|e: tailcopy_core::Error| err(e))?;
    clips_changed(&app);
    Ok(())
}

#[tauri::command]
pub async fn set_label(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    label: Option<String>,
) -> CmdResult<()> {
    state
        .db
        .run(move |core| {
            let label = label.filter(|l| !l.trim().is_empty());
            core.store.set_label(&ClipId::from(id), label.as_deref())?;
            core.refresh();
            Ok(())
        })
        .await
        .map_err(|e: tailcopy_core::Error| err(e))?;
    clips_changed(&app);
    Ok(())
}

#[tauri::command]
pub async fn delete(app: AppHandle, state: State<'_, AppState>, ids: Vec<String>) -> CmdResult<()> {
    let n = ids.len();
    state
        .db
        .run(move |core| {
            for id in ids {
                core.store.delete_row(&ClipId::from(id))?;
            }
            core.refresh();
            Ok(())
        })
        .await
        .map_err(|e: tailcopy_core::Error| err(e))?;
    tracing::info!(rows = n, "deleted");
    clips_changed(&app);
    status_changed(&app);
    Ok(())
}

#[tauri::command]
pub async fn clear_history(
    app: AppHandle,
    state: State<'_, AppState>,
    include_pinned: bool,
) -> CmdResult<()> {
    state
        .db
        .run(move |core| {
            core.store.clear_history(include_pinned)?;
            core.refresh();
            Ok(())
        })
        .await
        .map_err(|e: tailcopy_core::Error| err(e))?;
    tracing::info!(include_pinned, "history cleared");
    clips_changed(&app);
    status_changed(&app);
    Ok(())
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> CmdResult<Settings> {
    Ok(Settings::from_config(&state.config()))
}

#[tauri::command]
pub async fn set_settings(app: AppHandle, settings: Settings) -> CmdResult<Settings> {
    let state = app.state::<AppState>();
    let (config, hotkeys_changed) = {
        let mut file = state.config.lock().unwrap();
        let before = file.config.hotkeys.clone();
        settings.apply_to(&mut file.config);
        file.save(&state.config_path).map_err(err)?;
        (file.config.clone(), before != file.config.hotkeys)
    };
    crate::apply_config(&app, &config, hotkeys_changed).await;
    crate::state::status_changed(&app);
    Ok(Settings::from_config(&config))
}

#[tauri::command]
pub async fn set_paused(app: AppHandle, paused: bool, minutes: Option<u32>) -> CmdResult<()> {
    match minutes {
        Some(m) if paused && m > 0 => crate::state::pause_for(&app, m.min(24 * 60)),
        _ => crate::state::set_paused(&app, paused),
    }
    Ok(())
}

#[tauri::command]
pub async fn get_status(app: AppHandle) -> CmdResult<Status> {
    Ok(build_status(&app).await)
}

pub async fn build_status(app: &AppHandle) -> Status {
    let state = app.state::<AppState>();
    let (counts, this_device, device_id) = state
        .db
        .run(|core| {
            let me = core.store.device_id().clone();
            let device_id = me.0.clone();
            let this_device = core
                .store
                .view()
                .devices()
                .unwrap_or_default()
                .iter()
                .find(|d| d.device_id == me)
                .map(dto::device);
            (core.index.counts(), this_device, device_id)
        })
        .await;
    let peers = state
        .sync
        .get()
        .map(|e| {
            e.snapshot()
                .peers
                .into_iter()
                .map(|p| dto::PeerLine {
                    name: p.display_name,
                    connected: p.connected && p.caught_up,
                    last_sync_ms: p.last_sync_ms,
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let session = state.session;
    let paused_until_ms = *state.paused_until.lock().unwrap();
    #[cfg_attr(windows, allow(unused_mut))]
    let mut status = Status {
        version: env!("CARGO_PKG_VERSION"),
        session: match session {
            tailcopy_platform::Session::Windows => "windows",
            tailcopy_platform::Session::MacOs => "macos",
            tailcopy_platform::Session::Gnome => "gnome",
            tailcopy_platform::Session::Degraded => "degraded",
        },
        paused: state.is_paused(),
        paused_until_ms,
        capture_ready: state
            .capture_ready
            .load(std::sync::atomic::Ordering::Relaxed),
        degraded_reason: None,
        tailscale: crate::tailscale::state().await,
        extension: None,
        accessibility: None,
        first_run_done: state.first_run_done(),
        clip_count: counts.all,
        pinned_count: counts.pinned,
        counts: counts.into(),
        this_device,
        tailscale_name: crate::tailscale::hostname().await,
        theme: state.config().general.theme.as_str(),
        device_id,
        synced_with: peers.iter().filter(|p| p.connected).count(),
        peers,
    };
    #[cfg(target_os = "linux")]
    {
        use tailcopy_platform::gnome::ExtensionStatus;
        if session == tailcopy_platform::Session::Gnome {
            let ext = match state.gnome() {
                Some(g) => g.status().await,
                None => ExtensionStatus::Broken("couldn't reach GNOME Shell".into()),
            };
            let (s, message) = match ext {
                ExtensionStatus::Active => ("active", None),
                ExtensionStatus::Disabled => ("disabled", None),
                ExtensionStatus::NeedsRelogin => ("needsRelogin", None),
                ExtensionStatus::Missing => ("missing", None),
                ExtensionStatus::Broken(m) => ("broken", Some(m)),
            };
            if !status.capture_ready {
                status.degraded_reason = Some(match s {
                    "active" => "Connecting to GNOME Shell…".into(),
                    _ => "TailCopy can't see the clipboard yet. Open Settings > Run setup again."
                        .into(),
                });
            }
            status.extension = Some(ExtensionState { state: s, message });
        } else {
            status.degraded_reason = Some(
                "This desktop isn't supported yet. History is still here, but new copies aren't saved."
                    .into(),
            );
        }
    }
    #[cfg(target_os = "macos")]
    {
        status.accessibility = Some(tailcopy_platform::macos::accessibility_trusted());
    }
    status
}

#[tauri::command]
pub async fn run_first_run_step(app: AppHandle, step: String, action: String) -> CmdResult<Status> {
    tracing::info!(%step, %action, "setup step");
    let state = app.state::<AppState>();
    match action.as_str() {
        #[cfg(target_os = "linux")]
        "enable-extension" => {
            if let Some(g) = state.gnome() {
                g.enable().await.map_err(err)?;
            }
        }
        #[cfg(target_os = "linux")]
        "log-out" => {
            if let Some(g) = state.gnome() {
                g.log_out().await.map_err(err)?;
            }
        }
        #[cfg(target_os = "macos")]
        "open-accessibility" => {
            tailcopy_platform::macos::request_accessibility();
            tailcopy_platform::macos::open_accessibility_settings();
        }
        "finish" => {
            std::fs::write(state.data_dir.join("setup-complete"), b"1").map_err(err)?;
            let config = state.config();
            crate::apply_config(&app, &config, true).await;
        }
        _ => {}
    }
    Ok(build_status(&app).await)
}

#[tauri::command]
pub async fn hide_picker(app: AppHandle) -> CmdResult<()> {
    picker::hide_picker(&app);
    Ok(())
}

#[tauri::command]
pub async fn open_manager(app: AppHandle, route: Option<String>) -> CmdResult<()> {
    picker::hide_picker(&app);
    picker::show_manager(&app, route.as_deref());
    Ok(())
}

#[tauri::command]
pub async fn picker_focused(state: State<'_, AppState>) -> CmdResult<()> {
    if !state
        .focus_logged
        .swap(true, std::sync::atomic::Ordering::Relaxed)
    {
        picker::log_since_hotkey(&state, "picker focused");
    }
    Ok(())
}

#[tauri::command]
pub async fn open_log_folder(app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_path(state.data_dir.join("logs").to_string_lossy(), None::<&str>)
        .map_err(err)
}

#[tauri::command]
pub async fn open_tailscale() -> CmdResult<()> {
    crate::tailscale::open_app().map_err(err)
}

/// "Try again" on the Tailscale-not-running state: ask Tailscale now.
#[tauri::command]
pub async fn recheck_tailscale(app: AppHandle) -> CmdResult<()> {
    crate::tailscale::forget();
    crate::state::status_changed(&app);
    Ok(())
}

#[tauri::command]
pub async fn diagnostics(app: AppHandle) -> CmdResult<String> {
    let status = build_status(&app).await;
    let state = app.state::<AppState>();
    let config = state.config();
    let devices = crate::sync::devices(&app).await;
    let mut out = format!(
        "TailCopy {}\nos: {} {}\nsession: {}\ncapture ready: {}\npaused: {}\ntailscale: {}\nextension: {}\naccessibility: {}\nitems: {} ({} pinned)\nretention: {} items or {} days\nprivacy rules: {}\nstrict same login: {}\nsync: {}{}\nlive mirror: {}\nimages sync: {} (up to {} bytes)\nrelay: {}\n",
        status.version,
        std::env::consts::OS,
        std::env::consts::ARCH,
        status.session,
        status.capture_ready,
        status.paused,
        status.tailscale,
        status.extension.map(|e| e.state).unwrap_or("n/a"),
        status
            .accessibility
            .map(|a| a.to_string())
            .unwrap_or_else(|| "n/a".into()),
        status.clip_count,
        status.pinned_count,
        config.retention.max_items,
        config.retention.max_age_days,
        config.privacy.rules.len(),
        config.sync.strict_same_login,
        devices.sync,
        devices
            .sync_detail
            .map(|d| format!(" ({d})"))
            .unwrap_or_default(),
        config.general.live_mirror,
        config.sync.images,
        config.sync.max_image_bytes,
        if config.sync.relay.is_empty() {
            "none"
        } else {
            "set"
        },
    );
    for d in &devices.devices {
        out.push_str(&format!(
            "peer: {} role={} online={} connected={} caught_up={} path={} send={} receive={}{}\n",
            d.tailscale_name.as_deref().unwrap_or(&d.name),
            d.role,
            d.online,
            d.connected,
            d.caught_up,
            d.path.unwrap_or("-"),
            d.send,
            d.receive,
            d.error
                .as_deref()
                .map(|e| format!(" error={e}"))
                .unwrap_or_default(),
        ));
    }
    out.push_str(&format!(
        "tailnet peers without TailCopy: {}\nrefused: {}\n",
        devices.others.len(),
        devices.rejected.join(", ")
    ));
    Ok(out)
}

/// Devices on the tailnet and their sync state (SPEC 10.7 `get_devices`).
#[tauri::command]
pub async fn get_devices(app: AppHandle) -> CmdResult<crate::sync::Devices> {
    Ok(crate::sync::devices(&app).await)
}

/// Per-device Send and Receive switches (D-023).
#[tauri::command]
pub async fn set_device_sync(
    app: AppHandle,
    id: String,
    send: bool,
    receive: bool,
) -> CmdResult<()> {
    crate::sync::set_flags(&app, &id, tailcopy_core::SyncFlags { send, receive })
        .await
        .map_err(err)
}

/// A warning about a proposed shortcut, or None if it looks fine (SPEC 10.3).
#[tauri::command]
pub async fn check_shortcut(shortcut: String) -> CmdResult<Option<String>> {
    let s = shortcut.to_lowercase();
    let parts: Vec<&str> = s.split('+').collect();
    let has = |m: &str| parts.contains(&m);
    let warning = if cfg!(target_os = "macos")
        && has("cmd")
        && parts.len() == 2
        && ["c", "v", "x", "z", "a", "q", "w", "h", "m"].contains(&parts[1])
    {
        Some("That's a standard macOS shortcut. Add Shift or Option.".into())
    } else if cfg!(windows) && has("ctrl") && has("alt") && !has("shift") {
        Some(
            "On some keyboard layouts Ctrl+Alt types a character. If that happens, add Shift or choose another shortcut."
                .into(),
        )
    } else if cfg!(target_os = "linux")
        && has("super")
        && parts.len() == 2
        && ["v", "a", "l", "d"].contains(&parts[1])
    {
        Some("GNOME already uses that shortcut. Choose another.".into())
    } else {
        None
    };
    Ok(warning)
}
