//! Tailnet sync in the app (SPEC 7, 8): starts the engine on the store actor, keeps
//! the windows and tray up to date as peers change things, and mirrors live clips to
//! the clipboard (SPEC 7.6).

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use serde::Serialize;
use tailcopy_core::model::hash_hex;
use tailcopy_core::{ClipId, Config, DeviceId, Hash32, Op, OpBody, Role, SyncFlags};
use tailcopy_sync::{
    Engine, EngineConfig, StoreHandle, StoreJob, StoreRunner, SyncEvent, TailnetState,
};
use tailcopy_tailnet::{Path, TailnetTransport};
use tauri::{AppHandle, Emitter, Manager};

use crate::db::Db;
use crate::state::{AppState, clips_changed};

/// A live clip whose content isn't here within this long isn't mirrored (SPEC 7.6).
const MIRROR_FETCH: Duration = Duration::from_secs(10);
/// Pasting a clip whose content hasn't arrived waits this long (SPEC 7.5).
const PASTE_FETCH: Duration = Duration::from_secs(3);

struct DbRunner(Db);

impl StoreRunner for DbRunner {
    fn submit(&self, job: StoreJob) {
        self.0.submit(Box::new(move |core| job(&mut core.store)));
    }
}

pub fn engine_config(c: &Config) -> EngineConfig {
    EngineConfig {
        role: Role::Device,
        port: c.sync.port,
        strict_same_login: c.sync.strict_same_login,
        inline_blob_bytes: c.limits.inline_blob_bytes,
        max_fetch_bytes: c.limits.max_image_bytes.max(c.limits.max_text_bytes),
        app_version: env!("CARGO_PKG_VERSION").into(),
    }
}

/// Starts sync. Call once, from inside the async runtime.
pub fn start(app: &AppHandle) {
    let state = app.state::<AppState>();
    let config = state.config();
    let tailnet = crate::tailscale::init(&config);
    let transport = Arc::new(
        TailnetTransport::new(tailnet, config.sync.port)
            .with_relay(&config.sync.relay)
            .with_login_guard(),
    );
    let _ = state.transport.set(transport.clone());
    let engine = Engine::start(
        StoreHandle::new(DbRunner(state.db.clone())),
        transport,
        engine_config(&config),
    );
    let pusher = engine.clone();
    state.db.on_change(move || pusher.local_changed());
    let events = engine.subscribe();
    let _ = state.sync.set(engine);
    let handle = app.clone();
    tauri::async_runtime::spawn(async move { watch(handle, events).await });
    let handle = app.clone();
    tauri::async_runtime::spawn(async move { bind_node(handle).await });
}

/// Follows this machine's Tailscale identity. Records the node the data directory
/// belongs to once Tailscale answers (a different one means it was copied, SPEC
/// 6.1), and keeps checking the login: history belongs to the login it was
/// captured under, so after a switch sync stops until the user approves (D-027).
async fn bind_node(app: AppHandle) {
    let mut node_bound = false;
    loop {
        if let Ok(status) = crate::tailscale::status().await {
            if !node_bound
                && let Some(id) = status.self_node.as_ref().map(|n| n.stable_id.clone())
                && !id.is_empty()
            {
                let regenerated = app
                    .state::<AppState>()
                    .db
                    .run(move |core| core.store.bind_tailscale_node(&id))
                    .await;
                node_bound = regenerated.is_ok();
                if matches!(regenerated, Ok(true)) {
                    crate::state::status_changed(&app);
                }
            }
            if let Some(login) = status.self_login() {
                check_login(&app, login.to_owned()).await;
            }
        }
        tokio::time::sleep(Duration::from_secs(15)).await;
    }
}

async fn check_login(app: &AppHandle, current: String) {
    let state = app.state::<AppState>();
    let Some(transport) = state.transport.get() else {
        return;
    };
    let recorded = state
        .db
        .run(|core| core.store.sync_login())
        .await
        .ok()
        .flatten();
    match recorded {
        None => {
            // The first login seen owns this history.
            let login = current.clone();
            let _ = state
                .db
                .run(move |core| core.store.set_sync_login(&login))
                .await;
            transport.approve_login(&current);
        }
        Some(owner) if owner == current => {
            transport.approve_login(&current);
            if state.login_change.lock().unwrap().take().is_some() {
                tracing::info!("back on the Tailscale login this history belongs to");
                if let Some(engine) = state.sync.get() {
                    engine.retry_now();
                }
                crate::state::status_changed(app);
            }
        }
        Some(owner) => {
            let fresh = {
                let mut change = state.login_change.lock().unwrap();
                let fresh = change.as_ref().is_none_or(|(_, c)| *c != current);
                *change = Some((owner.clone(), current.clone()));
                fresh
            };
            if fresh {
                tracing::warn!(
                    "this machine's Tailscale login changed; sync stopped until approved"
                );
                // whois now refuses the new login's devices; end what's open.
                transport.approve_login(&owner);
                if let Some(engine) = state.sync.get() {
                    engine.reconnect_all();
                }
                crate::picker::notify(
                    app,
                    &format!(
                        "This computer is now signed in to Tailscale as {current}. Sync is stopped so your history doesn't go to that account's devices; allow it in Settings > Tailnet."
                    ),
                );
                crate::state::status_changed(app);
            }
        }
    }
}

/// The user allows the current Tailscale login to sync this history.
pub async fn approve_current_login(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let Some((_, current)) = state.login_change.lock().unwrap().clone() else {
        return Ok(());
    };
    let login = current.clone();
    state
        .db
        .run(move |core| core.store.set_sync_login(&login))
        .await
        .map_err(|e| e.to_string())?;
    if let Some(transport) = state.transport.get() {
        transport.approve_login(&current);
    }
    *state.login_change.lock().unwrap() = None;
    tracing::info!("the user allowed sync for the new Tailscale login");
    if let Some(engine) = state.sync.get() {
        engine.retry_now();
    }
    crate::state::status_changed(app);
    Ok(())
}

async fn watch(app: AppHandle, mut events: tokio::sync::broadcast::Receiver<SyncEvent>) {
    use tokio::sync::broadcast::error::RecvError;
    loop {
        match events.recv().await {
            Ok(SyncEvent::Applied { live, .. }) => {
                refresh(&app).await;
                if live
                    .iter()
                    .any(|op| matches!(op.body, OpBody::ClipAdded { .. }))
                    && !crate::picker::picker_visible(&app)
                {
                    crate::tray::set_new(&app, true);
                }
                if !live.is_empty() {
                    let app = app.clone();
                    tauri::async_runtime::spawn(async move { mirror(&app, live).await });
                }
            }
            Ok(SyncEvent::BlobStored(hash)) => {
                refresh(&app).await;
                let _ = app.emit("blob-ready", hash_hex(&hash));
            }
            Ok(SyncEvent::PeersChanged | SyncEvent::DeviceIdChanged) => {
                let _ = app.emit("devices-changed", ());
                crate::state::status_changed(&app);
            }
            Err(RecvError::Lagged(_)) => {
                refresh(&app).await;
                let _ = app.emit("devices-changed", ());
            }
            Err(RecvError::Closed) => return,
        }
    }
}

async fn refresh(app: &AppHandle) {
    app.state::<AppState>()
        .db
        .run(|core| {
            core.refresh();
        })
        .await;
    clips_changed(app);
}

/// A local clipboard change happened (even one TailCopy skips): nothing older than
/// it may be mirrored over it (SPEC 7.6).
pub async fn note_local_change(app: &AppHandle) {
    let state = app.state::<AppState>();
    if let Ok(hlc) = state.db.run(|core| core.store.tick_hlc()).await {
        state.local_clip_hlc.fetch_max(hlc, Ordering::SeqCst);
    }
}

async fn missing_blobs(state: &AppState, id: &ClipId) -> Option<Vec<Hash32>> {
    let id = id.clone();
    state
        .db
        .run(move |core| {
            let detail = core.store.view().get_clip(&id).ok().flatten()?;
            Some(
                detail
                    .formats
                    .iter()
                    .filter(|f| !f.present)
                    .map(|f| f.blob_hash)
                    .collect(),
            )
        })
        .await
}

/// Fetches whatever content of `id` hasn't arrived yet. False if some of it can't be
/// had within `timeout` (the device that copied it is offline).
pub async fn ensure_present(app: &AppHandle, id: &str, timeout: Option<Duration>) -> bool {
    let state = app.state::<AppState>();
    let Some(missing) = missing_blobs(&state, &ClipId::from(id)).await else {
        return false;
    };
    if missing.is_empty() {
        return true;
    }
    let Some(engine) = state.sync.get() else {
        return false;
    };
    let deadline = Instant::now() + timeout.unwrap_or(PASTE_FETCH);
    for hash in missing {
        let left = deadline.saturating_duration_since(Instant::now());
        if !engine.fetch_blob(hash, left).await {
            return false;
        }
    }
    true
}

/// Puts the newest live clip from another device on the clipboard (SPEC 7.6).
async fn mirror(app: &AppHandle, live: Vec<Op>) {
    let started = Instant::now();
    let state = app.state::<AppState>();
    if !state.config().general.live_mirror || state.is_paused() {
        return;
    }
    let Some(op) = live.into_iter().max_by_key(|op| op.hlc) else {
        return;
    };
    let Some(id) = op.body.clip_id().cloned() else {
        return;
    };
    if op.hlc <= state.local_clip_hlc.load(Ordering::SeqCst) {
        return;
    }
    let left = MIRROR_FETCH.saturating_sub(started.elapsed());
    if !ensure_present(app, id.as_str(), Some(left)).await {
        tracing::info!(clip = %id, "not mirrored: its content didn't arrive in time");
        return;
    }
    let Some(payload) = crate::picker::load(&state, id.as_str()).await else {
        return;
    };
    // Something newer happened here while the content was on its way, or capture
    // was paused meanwhile.
    if state.is_paused() || state.local_clip_hlc.fetch_max(op.hlc, Ordering::SeqCst) >= op.hlc {
        return;
    }
    if write_background(app, payload).await {
        tracing::info!(clip = %id, origin = %op.origin, "mirrored to the clipboard");
    }
}

#[cfg(target_os = "linux")]
async fn write_background(app: &AppHandle, payload: crate::picker::Payload) -> bool {
    // GNOME background writes carry one format (SPEC 9.4.3).
    let state = app.state::<AppState>();
    let Some(gnome) = state.gnome() else {
        return false;
    };
    if let Some(key) = payload.group_key {
        state.guard_write(key);
    }
    let result = match (&payload.png, &payload.text) {
        (Some(png), _) => gnome.set_png(png).await,
        (None, Some(text)) => gnome.set_text(text).await,
        _ => return false,
    };
    result
        .map_err(|e| tracing::warn!("mirror write failed: {e}"))
        .is_ok()
}

#[cfg(not(target_os = "linux"))]
async fn write_background(app: &AppHandle, payload: crate::picker::Payload) -> bool {
    !crate::picker::write(app, payload, false)
        .await
        .landed
        .is_empty()
}

/// A device in the Tailnet settings (design handoff section 5).
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DeviceRow {
    pub id: String,
    pub name: String,
    pub color_index: u32,
    #[serde(rename = "type")]
    pub device_type: &'static str,
    pub role: &'static str,
    pub os: Option<String>,
    pub tailscale_name: Option<String>,
    pub online: bool,
    pub connected: bool,
    pub caught_up: bool,
    /// direct, peer-relay, derp or idle, while online.
    pub path: Option<&'static str>,
    pub path_detail: Option<String>,
    pub last_seen_ms: Option<u64>,
    pub last_sync_ms: Option<u64>,
    pub send: bool,
    pub receive: bool,
    pub error: Option<String>,
}

/// A same-login tailnet peer that hasn't answered as TailCopy.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct OtherPeer {
    pub name: String,
    pub os: String,
    pub online: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Devices {
    /// starting, listening or unavailable.
    pub sync: &'static str,
    pub sync_detail: Option<String>,
    pub devices: Vec<DeviceRow>,
    pub others: Vec<OtherPeer>,
    pub rejected: Vec<String>,
}

fn path_of(node: &tailcopy_tailnet::Node) -> (Option<&'static str>, Option<String>) {
    if !node.online {
        return (None, None);
    }
    match node.path() {
        Path::Direct(a) => (Some("direct"), Some(a)),
        Path::PeerRelay(a) => (Some("peer-relay"), Some(a)),
        Path::Derp(r) => (Some("derp"), Some(r)),
        Path::Idle => (Some("idle"), None),
    }
}

pub async fn devices(app: &AppHandle) -> Devices {
    let state = app.state::<AppState>();
    let (me, rows, flags) = state
        .db
        .run(|core| {
            let view = core.store.view();
            (
                core.store.device_id().clone(),
                view.devices().unwrap_or_default(),
                core.store.sync_flags().unwrap_or_default(),
            )
        })
        .await;
    let snapshot = state.sync.get().map(Engine::snapshot);
    let status = crate::tailscale::status().await.ok();
    let (sync, sync_detail) = match snapshot.as_ref().map(|s| &s.tailnet) {
        Some(TailnetState::Listening) => ("listening", None),
        Some(TailnetState::Unavailable(why)) => ("unavailable", Some(why.clone())),
        _ => ("starting", None),
    };
    let mut known_nodes = std::collections::BTreeSet::new();
    let devices = rows
        .iter()
        .filter(|d| d.device_id != me && !d.display_name.is_empty())
        .map(|d| {
            let peer = snapshot
                .as_ref()
                .and_then(|s| s.peers.iter().find(|p| p.device_id == d.device_id));
            let node = status.as_ref().and_then(|s| {
                d.ts_node_id
                    .as_deref()
                    .and_then(|id| s.peers.iter().find(|n| n.stable_id == id))
            });
            if let Some(id) = &d.ts_node_id {
                known_nodes.insert(id.clone());
            }
            let (path, path_detail) = node.map(path_of).unwrap_or((None, None));
            let f = flags
                .get(&d.device_id)
                .copied()
                .unwrap_or(SyncFlags::default());
            let look = crate::dto::device(d);
            DeviceRow {
                id: d.device_id.0.clone(),
                name: d.display_name.clone(),
                color_index: d.color_index,
                device_type: look.device_type,
                role: d.role.as_str(),
                os: node
                    .map(|n| n.os.clone())
                    .filter(|s| !s.is_empty())
                    .or(d.os.clone()),
                tailscale_name: d.ts_hostname.clone(),
                online: node.is_some_and(|n| n.online),
                connected: peer.is_some_and(|p| p.connected),
                caught_up: peer.is_some_and(|p| p.caught_up),
                path,
                path_detail,
                last_seen_ms: d.last_seen_ms,
                last_sync_ms: peer.and_then(|p| p.last_sync_ms),
                send: f.send,
                receive: f.receive,
                error: peer.and_then(|p| p.error.clone()),
            }
        })
        .collect();
    let others = status
        .as_ref()
        .map(|s| {
            let relay_tag = &state.config().sync.relay_tag;
            s.candidates(relay_tag)
                .into_iter()
                .filter(|n| !known_nodes.contains(&n.stable_id))
                .map(|n| OtherPeer {
                    name: n.short_name().to_owned(),
                    os: n.os.clone(),
                    online: n.online,
                })
                .collect()
        })
        .unwrap_or_default();
    Devices {
        sync,
        sync_detail,
        devices,
        others,
        rejected: snapshot.map(|s| s.rejected).unwrap_or_default(),
    }
}

pub async fn set_flags(app: &AppHandle, id: &str, flags: SyncFlags) -> tailcopy_core::Result<()> {
    let id = DeviceId::from(id);
    app.state::<AppState>()
        .db
        .run(move |core| core.store.set_sync_flags(&id, flags))
        .await?;
    let _ = app.emit("devices-changed", ());
    Ok(())
}
