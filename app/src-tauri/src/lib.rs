//! Tauri shell: startup, windows, tray, hotkeys, the GNOME connection and the CLI.

mod cli;
mod commands;
mod db;
mod dto;
mod logging;
mod picker;
mod state;
mod sync;
mod tailscale;
mod tray;
mod update;

use std::path::PathBuf;

use std::time::Duration;

use directories::ProjectDirs;
use tailcopy_core::{
    Config, ConfigFile, Platform, Store, StoreOptions,
    names::{PROJECT_APPLICATION, PROJECT_ORGANIZATION, PROJECT_QUALIFIER},
};
use tailcopy_platform::Session;
use tauri::{AppHandle, Manager, RunEvent, WindowEvent};

pub use cli::{Command, attach_console, forward};
use state::AppState;

const RETENTION_INTERVAL: Duration = Duration::from_secs(60 * 60);

struct Paths {
    data: PathBuf,
    config: PathBuf,
}

fn paths() -> Paths {
    let dirs = ProjectDirs::from(PROJECT_QUALIFIER, PROJECT_ORGANIZATION, PROJECT_APPLICATION)
        .expect("a home directory");
    Paths {
        data: dirs.data_local_dir().to_path_buf(),
        config: dirs.config_dir().join("config.toml"),
    }
}

fn hostname() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .or_else(|_| std::fs::read_to_string("/etc/hostname").map(|s| s.trim().to_string()))
        .unwrap_or_else(|_| "This device".into())
}

pub fn run(initial: Command, autostart: bool) {
    // reqwest is built with rustls here (the update check); every client, including
    // the Tailscale LocalAPI and sync ones, needs a crypto provider installed first.
    let _ = rustls::crypto::ring::default_provider().install_default();
    let paths = paths();
    let log_dir = paths.data.join("logs");
    let _ = std::fs::create_dir_all(&log_dir);
    let guard = logging::init(&log_dir);
    tracing::info!(version = env!("CARGO_PKG_VERSION"), "TailCopy starting");

    #[allow(unused_mut)]
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--autostart"]),
        ));

    // GNOME binds shortcuts through the extension; X11, Windows and macOS through the
    // plugin, which can't work on Wayland.
    let session = tailcopy_platform::detect_session();
    if !cfg!(target_os = "linux") || session == Session::X11 {
        builder = builder.plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state != tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        return;
                    }
                    hotkeys::dispatch(app, shortcut);
                })
                .build(),
        );
    }

    let app = builder
        .register_asynchronous_uri_scheme_protocol("thumb", thumbs::handle)
        .invoke_handler(tauri::generate_handler![
            commands::list_clips,
            commands::get_clip,
            commands::paste,
            commands::copy,
            commands::set_pinned,
            commands::set_label,
            commands::delete,
            commands::clear_history,
            commands::get_settings,
            commands::set_settings,
            commands::set_paused,
            commands::get_status,
            commands::run_first_run_step,
            commands::hide_picker,
            commands::open_manager,
            commands::picker_focused,
            commands::open_log_folder,
            commands::open_link,
            commands::diagnostics,
            commands::open_tailscale,
            commands::recheck_tailscale,
            commands::check_shortcut,
            commands::approve_tailscale_login,
            commands::open_update,
            commands::check_update,
            commands::get_devices,
            commands::set_device_sync,
            commands::get_pair_code,
        ])
        .on_window_event(|window, event| {
            if window.label() == picker::MANAGER
                && let WindowEvent::CloseRequested { api, .. } = event
            {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .setup(move |app| {
            setup(app.handle(), paths, initial, autostart)?;
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("TailCopy failed to start");

    app.run(move |_, event| match event {
        // Closing the last window keeps TailCopy in the tray; Quit passes an exit code.
        RunEvent::ExitRequested {
            api, code: None, ..
        } => api.prevent_exit(),
        RunEvent::Exit => {
            tracing::info!("TailCopy exiting");
            let _ = &guard;
        }
        _ => {}
    });
}

fn setup(
    app: &AppHandle,
    paths: Paths,
    initial: Command,
    autostart: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let session = tailcopy_platform::detect_session();
    let platform = match session {
        Session::Gnome => Platform::Gnome,
        _ => Platform::current(),
    };
    // An unreadable config.toml must not quietly drop the user's privacy rules:
    // keep a copy of it and start with capture paused (fail safe), and say so.
    let mut config_problem = None;
    let config = ConfigFile::load(&paths.config, platform).unwrap_or_else(|e| {
        let kept = paths.config.with_extension("toml.bad");
        let _ = std::fs::copy(&paths.config, &kept);
        tracing::error!(
            "config.toml couldn't be read ({e}); kept a copy as {}, capture paused",
            kept.display()
        );
        config_problem = Some(format!(
            "Your settings file has a problem ({e}), so capture is paused. A copy was kept as config.toml.bad; fix the file and restart, or resume to use the default settings."
        ));
        ConfigFile::default_for(platform)
    });
    let display_name = match config.config.general.device_name.as_str() {
        "" => hostname(),
        name => name.to_string(),
    };
    let store = Store::open(
        &paths.data,
        StoreOptions {
            display_name,
            limits: config.config.size_limits(),
            sync_source_app: config.config.privacy.sync_source_app,
            ..StoreOptions::default()
        },
    )?;
    crate::tailscale::init(&config.config);
    let db = db::Db::spawn(store)?;
    app.manage(AppState::new(db, config, paths.config, paths.data, session));
    tracing::info!(?session, "session detected");
    if let Some(problem) = &config_problem {
        state::set_paused(app, true);
        picker::notify(app, problem);
    }

    #[cfg(target_os = "macos")]
    if let Err(e) = app.set_activation_policy(tauri::ActivationPolicy::Accessory) {
        tracing::warn!("couldn't hide the Dock icon: {e}");
    }

    picker::create_picker(app)?;
    tray::create(app)?;
    update::start(app);

    match session {
        #[cfg(any(windows, target_os = "macos"))]
        Session::Windows | Session::MacOs => state::start_capture(app),
        #[cfg(target_os = "linux")]
        Session::X11 => state::start_capture(app),
        #[cfg(target_os = "linux")]
        Session::Gnome => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move { gnome::run(app).await });
        }
        _ => tracing::warn!("no clipboard backend for this desktop; capture is off"),
    }

    let config = app.state::<AppState>().config();
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        crate::sync::start(&handle);
        apply_config(&handle, &config, true).await;
    });

    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        match cli::listen().await {
            Ok(rx) => serve_cli(handle, rx).await,
            Err(e) => tracing::warn!(
                "local socket unavailable, CLI commands won't reach this instance: {e}"
            ),
        }
    });

    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(RETENTION_INTERVAL).await;
            let state = handle.state::<AppState>();
            let policy = state.config().retention_policy();
            let trimmed = state
                .db
                .run(move |core| {
                    let n = core.store.enforce_retention(&policy);
                    core.refresh();
                    n
                })
                .await;
            match trimmed {
                Ok(n) if n > 0 => {
                    tracing::info!(trimmed = n, "retention");
                    state::clips_changed(&handle);
                }
                Err(e) => tracing::warn!("retention failed: {e}"),
                _ => {}
            }
        }
    });

    let first_run = !app.state::<AppState>().first_run_done();
    if first_run || (initial == Command::Open && !autostart) {
        picker::show_manager(app, if first_run { Some("setup") } else { None });
    }
    if initial != Command::Open {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            handle_command(&app, initial).await;
        });
    }
    Ok(())
}

async fn serve_cli(app: AppHandle, mut rx: tokio::sync::mpsc::Receiver<cli::Request>) {
    while let Some(req) = rx.recv().await {
        let reply = handle_command(&app, req.command).await;
        let _ = req.reply.send(reply);
    }
}

async fn handle_command(app: &AppHandle, command: Command) -> String {
    match command {
        Command::Open => picker::show_manager(app, None),
        Command::Picker => picker::toggle_picker(app).await,
        Command::PastePlain => picker::paste_plain(app).await,
        Command::Pause => state::set_paused(app, true),
        Command::Resume => state::set_paused(app, false),
        Command::Status => {
            let status = commands::build_status(app).await;
            // One line: the local socket protocol is line based.
            return serde_json::to_string(&status).unwrap_or_default();
        }
    }
    String::new()
}

/// Applies saved settings: store limits, retention, shortcuts and start at login.
pub async fn apply_config(app: &AppHandle, config: &Config, hotkeys_changed: bool) {
    let state = app.state::<AppState>();
    let limits = config.size_limits();
    let sync_source_app = config.privacy.sync_source_app;
    let policy = config.retention_policy();
    let self_name = match config.general.device_name.as_str() {
        "" => crate::tailscale::hostname().await.unwrap_or_else(hostname),
        name => name.to_owned(),
    };
    app.set_theme(match config.general.theme {
        tailcopy_core::Theme::System => None,
        tailcopy_core::Theme::Light => Some(tauri::Theme::Light),
        tailcopy_core::Theme::Dark => Some(tauri::Theme::Dark),
    });
    let trimmed = state
        .db
        .run(move |core| {
            core.store.set_limits(limits);
            core.store.set_sync_source_app(sync_source_app);
            if let Err(e) = core.store.set_self_display_name(&self_name) {
                tracing::warn!("couldn't rename this device: {e}");
            }
            let n = core.store.enforce_retention(&policy);
            core.refresh();
            n
        })
        .await;
    if matches!(trimmed, Ok(n) if n > 0) {
        state::clips_changed(app);
    }
    if let Some(engine) = state.sync.get() {
        engine.set_config(crate::sync::engine_config(config));
    }
    if hotkeys_changed {
        hotkeys::bind(app, config).await;
    }
    if state.first_run_done() {
        use tauri_plugin_autostart::ManagerExt;
        let launcher = app.autolaunch();
        let enabled = launcher.is_enabled().unwrap_or(false);
        let result = match (config.general.start_at_login, enabled) {
            (true, false) => launcher.enable(),
            (false, true) => launcher.disable(),
            _ => Ok(()),
        };
        if let Err(e) = result {
            tracing::warn!("start at login: {e}");
        }
    }
}

mod hotkeys {
    use super::*;

    /// The parsed global shortcuts, for dispatching plugin events.
    #[derive(Default)]
    pub struct Bound {
        pub picker: Option<String>,
        pub paste_plain: Option<String>,
        pub pause: Option<String>,
    }

    pub static BOUND: std::sync::Mutex<Bound> = std::sync::Mutex::new(Bound {
        picker: None,
        paste_plain: None,
        pause: None,
    });

    pub async fn bind(app: &AppHandle, config: &Config) {
        #[cfg(target_os = "linux")]
        match app.state::<AppState>().session {
            Session::Gnome => return bind_gnome(app, config).await,
            Session::X11 => {}
            _ => return,
        }
        bind_global(app, config);
    }

    /// Windows, macOS and X11: Tauri's global shortcut plugin.
    fn bind_global(app: &AppHandle, config: &Config) {
        use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};
        let gs = app.global_shortcut();
        let _ = gs.unregister_all();
        let mut bound = Bound::default();
        for (slot, value) in [
            (&mut bound.picker, &config.hotkeys.picker),
            (&mut bound.paste_plain, &config.hotkeys.paste_plain),
            (&mut bound.pause, &config.hotkeys.pause),
        ] {
            if value.is_empty() {
                continue;
            }
            match value.parse::<Shortcut>() {
                Ok(s) => match gs.register(s) {
                    Ok(()) => *slot = Some(s.into_string()),
                    Err(e) => {
                        tracing::warn!("couldn't register {value}: {e}");
                        picker::notify(
                            app,
                            &format!(
                                "TailCopy couldn't use {value}; another app may already have it. Choose another shortcut in Settings."
                            ),
                        );
                    }
                },
                Err(e) => tracing::warn!("invalid shortcut {value}: {e}"),
            }
        }
        *BOUND.lock().unwrap() = bound;
    }

    #[cfg(target_os = "linux")]
    async fn bind_gnome(app: &AppHandle, config: &Config) {
        let Some(gnome) = app.state::<AppState>().gnome() else {
            return;
        };
        for (action, value) in [
            ("picker", &config.hotkeys.picker),
            ("paste-plain", &config.hotkeys.paste_plain),
        ] {
            if let Err(e) = gnome.set_shortcut(action, value).await {
                tracing::warn!("couldn't set the {action} shortcut: {e}");
            }
        }
    }

    pub fn dispatch(app: &AppHandle, shortcut: &tauri_plugin_global_shortcut::Shortcut) {
        let pressed = Some(shortcut.into_string());
        let bound = BOUND.lock().unwrap();
        let app = app.clone();
        if pressed == bound.picker {
            tauri::async_runtime::spawn(async move { picker::toggle_picker(&app).await });
        } else if pressed == bound.paste_plain {
            tauri::async_runtime::spawn(async move { picker::paste_plain(&app).await });
        } else if pressed == bound.pause {
            let paused = !app.state::<AppState>().is_paused();
            state::set_paused(&app, paused);
        }
    }
}

mod thumbs {
    use tauri::http::{Request, Response, StatusCode, header::CONTENT_TYPE};
    use tauri::{Manager, UriSchemeContext, UriSchemeResponder, Wry};

    use crate::state::AppState;

    fn parse_hex(hex: &str) -> Option<tailcopy_core::Hash32> {
        if hex.len() != 64 {
            return None;
        }
        let bytes: Option<Vec<u8>> = (0..32)
            .map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok())
            .collect();
        tailcopy_core::model::hash_from_slice(&bytes?).ok()
    }

    /// `thumb://localhost/thumb/<hash>` and `/full/<hash>` (SPEC 6.2 thumbnails).
    pub fn handle(
        ctx: UriSchemeContext<'_, Wry>,
        request: Request<Vec<u8>>,
        responder: UriSchemeResponder,
    ) {
        let app = ctx.app_handle().clone();
        let path = request.uri().path().trim_start_matches('/').to_string();
        tauri::async_runtime::spawn(async move {
            let not_found = || {
                Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(Vec::new())
                    .unwrap()
            };
            let Some((kind, hex)) = path.split_once('/') else {
                return responder.respond(not_found());
            };
            let (Some(hash), full) = (parse_hex(hex), kind == "full") else {
                return responder.respond(not_found());
            };
            let bytes = app
                .state::<AppState>()
                .db
                .run(move |core| {
                    let view = core.store.view();
                    if full {
                        view.read_blob(&hash).ok().flatten()
                    } else {
                        view.thumb_path(&hash).and_then(|p| std::fs::read(p).ok())
                    }
                })
                .await;
            responder.respond(match bytes {
                Some(b) => Response::builder()
                    .header(CONTENT_TYPE, "image/png")
                    .header("Cache-Control", "max-age=31536000, immutable")
                    .body(b)
                    .unwrap(),
                None => not_found(),
            });
        });
    }
}

#[cfg(target_os = "linux")]
mod gnome {
    /// How long a GNOME clipboard change must stay quiet before it's read.
    const CAPTURE_SETTLE: Duration = Duration::from_millis(150);

    use std::sync::Arc;
    use std::sync::atomic::Ordering;

    use futures_util::StreamExt;
    use tailcopy_platform::gnome::{Event, Gnome};

    use super::*;

    pub async fn run(app: AppHandle) {
        let gnome = loop {
            match Gnome::connect().await {
                Ok(g) => break Arc::new(g),
                Err(e) => {
                    tracing::warn!("couldn't reach GNOME Shell: {e}");
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        };
        let state = app.state::<AppState>();
        let _ = state.gnome.set(gnome.clone());
        let mut events = match gnome.events().await {
            Ok(e) => e,
            Err(e) => {
                tracing::error!("couldn't subscribe to the extension: {e}");
                return;
            }
        };
        register(&app, &gnome).await;
        // One copy can arrive as several owner changes, the first without every type
        // (a password manager's hint can come in the second). Read once it settles.
        let mut pending: Option<(Vec<String>, String)> = None;
        let settle = tokio::time::sleep(Duration::from_secs(3600));
        tokio::pin!(settle);
        loop {
            let event = tokio::select! {
                event = events.next() => match event {
                    Some(e) => e,
                    None => break,
                },
                () = &mut settle, if pending.is_some() => {
                    let (mimetypes, source_app) = pending.take().expect("pending change");
                    if state.is_paused() {
                        // Not read, but still a local change (SPEC 7.6).
                        crate::sync::note_local_change(&app).await;
                    } else {
                        match gnome.read(&mimetypes, &source_app).await {
                            Ok(c) => crate::state::ingest(&app, c).await,
                            Err(e) => tracing::warn!("reading the clipboard failed: {e}"),
                        }
                    }
                    continue;
                }
            };
            match event {
                Event::ClipboardChanged {
                    mimetypes,
                    source_app,
                } => {
                    tracing::debug!(?mimetypes, %source_app, "clipboard changed");
                    // Count it now rather than after the settle, so a mirror can't
                    // land in between (SPEC 7.6).
                    crate::sync::note_local_change(&app).await;
                    pending = Some((mimetypes, source_app));
                    settle
                        .as_mut()
                        .reset(tokio::time::Instant::now() + CAPTURE_SETTLE);
                }
                Event::Shortcut {
                    action,
                    focused_app,
                } => {
                    *state.focused_app.lock().unwrap() = focused_app;
                    match action.as_str() {
                        "picker" => picker::toggle_picker(&app).await,
                        "paste-plain" => picker::paste_plain(&app).await,
                        _ => {}
                    }
                }
                Event::ExtensionActive => {
                    // The shell can report ACTIVE just before disabling (D-011); retry.
                    for _ in 0..3 {
                        if register(&app, &gnome).await {
                            break;
                        }
                        tokio::time::sleep(Duration::from_millis(400)).await;
                    }
                }
                Event::ExtensionInactive => {
                    state.capture_ready.store(false, Ordering::Relaxed);
                    crate::state::status_changed(&app);
                }
            }
        }
        tracing::warn!("lost the connection to GNOME Shell");
    }

    async fn register(app: &AppHandle, gnome: &Gnome) -> bool {
        let state = app.state::<AppState>();
        let ok = match gnome.register().await {
            Ok(api) if Gnome::api_supported(api) => {
                tracing::info!(api, "registered with the GNOME extension");
                true
            }
            Ok(api) => {
                tracing::warn!(api, "unsupported extension version; log out and back in");
                false
            }
            Err(e) => {
                tracing::info!("extension not ready: {e}");
                false
            }
        };
        state.capture_ready.store(ok, Ordering::Relaxed);
        if ok {
            hotkeys::bind(app, &state.config()).await;
        }
        crate::state::status_changed(app);
        ok
    }
}
