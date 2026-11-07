//! The phone apps' core (SPEC 21.2): the store, search and the dial-only sync
//! engine behind one uniffi object. The React Native app reaches it through a thin
//! Expo Module; Swift and Kotlin extensions call the same bindings.
//!
//! Every method returns quickly or is async, so no UI thread ever waits on disk or
//! the network. Nothing here logs clip content.

mod actor;
mod inbox;

use std::collections::HashMap;
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tailcopy_core::hlc::wall_ms;
use tailcopy_core::model::mime;
use tailcopy_core::{
    Capture, CaptureOutcome, CapturedFormats, ClipId, DeviceId, DisplayRow, OpBody, PeerKey, Role,
    SearchContext, SyncFlags,
};
use tailcopy_sync::{
    Engine, EngineConfig, PairLink, PairedTransport, StoreHandle, SyncEvent, TailnetState,
    is_tailscale_ip,
};

use crate::actor::Actor;

uniffi::setup_scaffolding!();

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum CoreError {
    #[error("{message}")]
    Failed { message: String },
    #[error("{message}")]
    BadPairingCode { message: String },
}

fn failed(e: impl std::fmt::Display) -> CoreError {
    CoreError::Failed {
        message: e.to_string(),
    }
}

/// What the platform hears back. Called on a background thread.
#[uniffi::export(with_foreign)]
pub trait CoreListener: Send + Sync {
    /// The list, devices or sync state changed; re-read what's on screen.
    fn changed(&self);
    /// A clip from another device arrived live (SPEC 7.6). The platform writes it
    /// to the clipboard only if live mirror is on and the app is in the foreground.
    fn live_clip(&self, clip_id: String);
}

/// Formats read from the clipboard, a share or a drop.
#[derive(Clone, Debug, Default, uniffi::Record)]
pub struct ClipInput {
    pub text: Option<String>,
    pub html: Option<String>,
    pub rtf: Option<Vec<u8>>,
    /// PNG, JPEG, HEIC is not supported: the platform converts it to PNG first.
    pub image: Option<Vec<u8>>,
}

/// One row of History or Pinned.
#[derive(Clone, Debug, uniffi::Record)]
pub struct ClipItem {
    pub id: String,
    /// text, rich or image.
    pub kind: String,
    /// text, rich, image, code, link or colour (how it's drawn).
    pub look: String,
    pub preview: String,
    pub label: Option<String>,
    pub pinned: bool,
    pub mono: bool,
    pub device_id: String,
    pub device_name: String,
    pub color_index: u32,
    pub from_this_device: bool,
    pub created_ms: u64,
    pub byte_size: u64,
    /// Kept here only (over a cap or a privacy rule).
    pub local_only: bool,
    /// Path of a PNG thumbnail on disk, for images.
    pub thumbnail: Option<String>,
}

/// A clip's content, for copying out.
#[derive(Clone, Debug, uniffi::Record)]
pub struct ClipContent {
    pub text: Option<String>,
    pub html: Option<String>,
    pub rtf: Option<Vec<u8>>,
    pub png: Option<Vec<u8>>,
    /// Some format is still on another device that didn't answer in time.
    pub incomplete: bool,
    /// Copied on another device (its pasteboard write stays local, SPEC 21.4).
    pub remote: bool,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct DeviceItem {
    pub id: String,
    pub name: String,
    /// device, relay or mobile.
    pub role: String,
    pub color_index: u32,
    pub is_self: bool,
    /// Paired, or vouched for by a paired device (SPEC 21.3.5).
    pub trusted: bool,
    pub connected: bool,
    pub caught_up: bool,
    pub last_sync_ms: Option<u64>,
    pub error: Option<String>,
    pub send: bool,
    pub receive: bool,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct SyncStatus {
    /// stopped, starting, ready (on the tailnet) or offline (Tailscale isn't connected).
    pub state: String,
    pub connected: u32,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct Paired {
    pub device_id: String,
    pub name: String,
}

/// The core. One per process; the app owns it.
#[derive(uniffi::Object)]
pub struct TailCopy {
    rt: Runtime,
    actor: Actor,
    engine: Mutex<Option<Engine>>,
    listener: Arc<dyn CoreListener>,
    inbox: Option<PathBuf>,
}

/// The core's tokio runtime. It may be dropped from any thread, including one
/// that is itself running async code, so it shuts down without waiting.
struct Runtime(Option<tokio::runtime::Runtime>);

impl std::ops::Deref for Runtime {
    type Target = tokio::runtime::Runtime;
    fn deref(&self) -> &Self::Target {
        self.0.as_ref().expect("runtime lives as long as the core")
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        if let Some(rt) = self.0.take() {
            rt.shutdown_background();
        }
    }
}

fn tailnet_addrs() -> Option<Vec<IpAddr>> {
    let ips: Vec<IpAddr> = if_addrs::get_if_addrs()
        .ok()?
        .into_iter()
        .map(|i| i.ip())
        .filter(|ip| is_tailscale_ip(*ip))
        .collect();
    (!ips.is_empty()).then_some(ips)
}

fn engine_config() -> EngineConfig {
    EngineConfig {
        role: Role::Mobile,
        app_version: env!("CARGO_PKG_VERSION").into(),
        ..EngineConfig::default()
    }
}

#[uniffi::export]
impl TailCopy {
    /// Opens (or creates) the store in `data_dir`. `inbox_dir` is where iOS
    /// extensions leave clips; Android passes `None`.
    #[uniffi::constructor]
    pub fn open(
        data_dir: String,
        inbox_dir: Option<String>,
        device_name: String,
        listener: Arc<dyn CoreListener>,
    ) -> Result<Arc<TailCopy>, CoreError> {
        let dir = PathBuf::from(&data_dir);
        std::fs::create_dir_all(dir.join("logs")).map_err(failed)?;
        init_logging(&dir);
        let rt = Runtime(Some(
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .thread_name("tailcopy-sync")
                .enable_all()
                .build()
                .map_err(failed)?,
        ));
        let actor = Actor::spawn(dir, device_name).map_err(failed)?;
        let core = Arc::new(TailCopy {
            rt,
            actor,
            engine: Mutex::new(None),
            listener,
            inbox: inbox_dir.map(PathBuf::from),
        });
        let weak = Arc::downgrade(&core);
        core.actor.on_change(move || {
            if let Some(core) = weak.upgrade() {
                if let Some(e) = core.engine.lock().unwrap().as_ref() {
                    e.local_changed();
                }
                core.listener.changed();
            }
        });
        tracing::info!(version = env!("CARGO_PKG_VERSION"), "core opened");
        Ok(core)
    }

    /// Starts dialling paired devices (the app came to the foreground, or a
    /// background refresh began).
    pub fn start_sync(self: Arc<Self>) {
        let mut slot = self.engine.lock().unwrap();
        if slot.is_some() {
            return;
        }
        let _guard = self.rt.enter();
        let store = StoreHandle::new(self.actor.clone());
        let transport = Arc::new(PairedTransport::new(store.clone(), Box::new(tailnet_addrs)));
        let engine = Engine::start(store, transport, engine_config());
        let mut events = engine.subscribe();
        let listener = self.listener.clone();
        self.rt.spawn(async move {
            while let Ok(event) = events.recv().await {
                match event {
                    SyncEvent::Applied { live, .. } => {
                        for op in live {
                            if let OpBody::ClipAdded { clip_id, .. }
                            | OpBody::ClipUsed { clip_id } = &op.body
                            {
                                listener.live_clip(clip_id.0.clone());
                            }
                        }
                    }
                    SyncEvent::PeersChanged => listener.changed(),
                    _ => {}
                }
            }
        });
        *slot = Some(engine);
        tracing::info!("sync started");
    }

    /// Stops every session (the app is leaving the foreground).
    pub fn stop_sync(&self) {
        if let Some(engine) = self.engine.lock().unwrap().take() {
            engine.stop();
            tracing::info!("sync stopped");
        }
    }

    /// Stops sync and closes the database until the next call (iOS, before the app
    /// is suspended; SPEC 21.2).
    pub async fn suspend(&self) {
        self.stop_sync();
        let actor = self.actor.clone();
        let _ = self.rt.spawn(async move { actor.close().await }).await;
    }

    /// Retries every paired device now (Tailscale reconnected, or the user pulled
    /// to refresh).
    pub fn retry(&self) {
        if let Some(engine) = self.engine.lock().unwrap().as_ref() {
            engine.retry_now();
        }
    }

    pub fn status(&self) -> SyncStatus {
        let snapshot = self.engine.lock().unwrap().as_ref().map(Engine::snapshot);
        let Some(s) = snapshot else {
            return SyncStatus {
                state: "stopped".into(),
                connected: 0,
            };
        };
        SyncStatus {
            state: match s.tailnet {
                TailnetState::Starting => "starting",
                TailnetState::Unavailable(_) => "offline",
                TailnetState::Listening | TailnetState::DialOnly => "ready",
            }
            .into(),
            connected: s.peers.iter().filter(|p| p.connected).count() as u32,
        }
    }

    /// Saves a clip. Returns whether anything was stored. `sensitive` clips (marked
    /// by a password manager) are never stored; `local_only` ones are kept here and
    /// not synced (Android's remote-clipboard exclusion, SPEC 21.4).
    pub async fn capture(
        &self,
        clip: ClipInput,
        source_app: Option<String>,
        sensitive: bool,
        local_only: bool,
    ) -> Result<bool, CoreError> {
        if sensitive {
            return Ok(false);
        }
        let capture = Capture {
            formats: CapturedFormats {
                text: clip.text,
                html: clip.html,
                rtf: clip.rtf,
                image: clip.image,
            },
            source_app,
            local_only,
        };
        self.call(move |core| {
            Ok(matches!(
                core.store()?.capture(&capture)?,
                CaptureOutcome::Added { .. }
            ))
        })
        .await
    }

    /// Adds every clip the extensions left in the inbox. Returns how many.
    pub async fn ingest_inbox(&self) -> Result<u32, CoreError> {
        let Some(dir) = self.inbox.clone() else {
            return Ok(0);
        };
        self.call(move |core| {
            let mut added = 0;
            for p in inbox::read(&dir) {
                if matches!(
                    core.store()?.capture(&p.capture)?,
                    CaptureOutcome::Added { .. }
                ) {
                    added += 1;
                }
                inbox::remove(&p.files);
            }
            Ok(added)
        })
        .await
    }

    /// History rows matching `query` (the desktop picker's syntax: `from:`, `is:`,
    /// free text), newest first. An empty query lists everything.
    pub async fn list(
        &self,
        query: String,
        offset: u32,
        limit: u32,
        today_start_ms: u64,
        yesterday_start_ms: u64,
    ) -> Result<Vec<ClipItem>, CoreError> {
        let (offset, limit) = (offset as usize, limit.clamp(1, 500) as usize);
        self.call(move |core| {
            let (store, index) = core.store_and_index()?;
            let me = store.device_id().clone();
            let view = store.view();
            let devices: HashMap<_, _> = view
                .devices()?
                .into_iter()
                .map(|d| (d.device_id.clone(), (d.display_name, d.color_index)))
                .collect();
            let rows: Vec<DisplayRow> = if query.trim().is_empty() {
                index.rows().skip(offset).take(limit).cloned().collect()
            } else {
                let ctx = SearchContext {
                    today_start_ms,
                    yesterday_start_ms,
                };
                index
                    .search(Some(&view), &query, &ctx, offset + limit)?
                    .into_iter()
                    .skip(offset)
                    .map(|h| h.row)
                    .collect()
            };
            Ok(rows
                .into_iter()
                .map(|r| {
                    let (name, colour) = devices.get(&r.origin).cloned().unwrap_or_default();
                    ClipItem {
                        thumbnail: r
                            .image_hash
                            .and_then(|h| view.thumb_path(&h))
                            .map(|p| p.to_string_lossy().into_owned()),
                        id: r.clip_id.0,
                        kind: r.kind.as_str().into(),
                        look: r.look.as_str().into(),
                        preview: r.preview,
                        label: r.label,
                        pinned: r.pinned,
                        mono: r.mono,
                        from_this_device: r.origin == me,
                        device_id: r.origin.0,
                        device_name: name,
                        color_index: colour,
                        created_ms: wall_ms(r.created_hlc),
                        byte_size: r.byte_size,
                        local_only: r.local_only,
                    }
                })
                .collect())
        })
        .await
    }

    /// A clip's formats, fetching any that are still on another device for up to
    /// `wait_ms` (SPEC 7.5).
    pub async fn content(&self, clip_id: String, wait_ms: u64) -> Result<ClipContent, CoreError> {
        let id = ClipId(clip_id);
        let lookup = id.clone();
        let detail = self
            .call(move |core| Ok(core.store()?.view().get_clip(&lookup)?))
            .await?
            .ok_or_else(|| failed("That clip was deleted"))?;
        let missing: Vec<_> = detail
            .formats
            .iter()
            .filter(|f| !f.present)
            .map(|f| f.blob_hash)
            .collect();
        let engine = self.engine.lock().unwrap().clone();
        if let Some(engine) = engine
            && !missing.is_empty()
        {
            let wait = Duration::from_millis(wait_ms);
            let fetches = missing.into_iter().map(|h| {
                let e = engine.clone();
                self.rt.spawn(async move { e.fetch_blob(h, wait).await })
            });
            for f in fetches.collect::<Vec<_>>() {
                let _ = f.await;
            }
        }
        let remote = detail.clip.origin;
        self.call(move |core| {
            let store = core.store()?;
            let remote = remote != *store.device_id();
            let view = store.view();
            let read = |m: &str| view.read_format(&id, m);
            let text = read(mime::TEXT)?.and_then(|b| String::from_utf8(b).ok());
            let html = read(mime::HTML)?.and_then(|b| String::from_utf8(b).ok());
            let rtf = read(mime::RTF)?;
            let png = read(mime::PNG)?;
            let wanted = detail.formats.len();
            let got = [text.is_some(), html.is_some(), rtf.is_some(), png.is_some()]
                .iter()
                .filter(|b| **b)
                .count();
            Ok(ClipContent {
                text,
                html,
                rtf,
                png,
                incomplete: got < wanted,
                remote,
            })
        })
        .await
    }

    /// The clip was copied out of TailCopy (moves it to the top everywhere).
    pub async fn used(&self, clip_id: String) -> Result<(), CoreError> {
        self.call(move |core| {
            core.store()?.use_clip(&ClipId(clip_id))?;
            Ok(())
        })
        .await
    }

    pub async fn set_pinned(&self, clip_id: String, pinned: bool) -> Result<(), CoreError> {
        self.call(move |core| {
            core.store()?.set_pinned(&ClipId(clip_id), pinned)?;
            Ok(())
        })
        .await
    }

    pub async fn set_label(&self, clip_id: String, label: Option<String>) -> Result<(), CoreError> {
        self.call(move |core| {
            let label = label.as_deref().map(str::trim).filter(|l| !l.is_empty());
            core.store()?.set_label(&ClipId(clip_id), label)?;
            Ok(())
        })
        .await
    }

    pub async fn delete(&self, clip_id: String) -> Result<(), CoreError> {
        self.call(move |core| {
            core.store()?.delete_row(&ClipId(clip_id))?;
            Ok(())
        })
        .await
    }

    pub async fn clear_history(&self, include_pinned: bool) -> Result<(), CoreError> {
        self.call(move |core| {
            core.store()?.clear_history(include_pinned)?;
            Ok(())
        })
        .await
    }

    /// This phone and every desktop and relay it knows: paired, vouched for, or
    /// seen in a session.
    pub async fn devices(&self) -> Result<Vec<DeviceItem>, CoreError> {
        let snapshot = self.engine.lock().unwrap().as_ref().map(Engine::snapshot);
        let (rows, keys, flags) = self
            .call(|core| {
                let store = core.store()?;
                Ok((
                    store.view().devices()?,
                    store.peer_keys()?,
                    store.sync_flags()?,
                ))
            })
            .await?;
        let peer = |id: &DeviceId| {
            snapshot
                .as_ref()
                .and_then(|s| s.peers.iter().find(|p| &p.device_id == id).cloned())
        };
        let trusted: HashMap<DeviceId, &PeerKey> = keys
            .iter()
            .filter(|k| k.trusted)
            .map(|k| (k.device_id.clone(), k))
            .collect();
        let mut out: Vec<DeviceItem> = rows
            .iter()
            .filter(|d| d.is_self || trusted.contains_key(&d.device_id) || d.role == Role::Mobile)
            .map(|d| {
                let p = peer(&d.device_id);
                let f = flags.get(&d.device_id).copied().unwrap_or_default();
                DeviceItem {
                    id: d.device_id.0.clone(),
                    name: d.display_name.clone(),
                    role: d.role.as_str().into(),
                    color_index: d.color_index,
                    is_self: d.is_self,
                    trusted: trusted.contains_key(&d.device_id),
                    connected: p.as_ref().is_some_and(|p| p.connected),
                    caught_up: p.as_ref().is_some_and(|p| p.caught_up),
                    last_sync_ms: p.as_ref().and_then(|p| p.last_sync_ms).or(d.last_seen_ms),
                    error: p.and_then(|p| p.error),
                    send: f.send,
                    receive: f.receive,
                }
            })
            .collect();
        // Paired but never reached: no devices row yet.
        for k in keys.iter().filter(|k| k.trusted) {
            if !out.iter().any(|d| d.id == k.device_id.0) {
                out.push(DeviceItem {
                    id: k.device_id.0.clone(),
                    name: k.display_name.clone(),
                    role: k.role.as_str().into(),
                    color_index: 0,
                    is_self: false,
                    trusted: true,
                    connected: false,
                    caught_up: false,
                    last_sync_ms: None,
                    error: peer(&k.device_id).and_then(|p| p.error),
                    send: true,
                    receive: true,
                });
            }
        }
        Ok(out)
    }

    pub async fn set_device_sync(
        &self,
        device_id: String,
        send: bool,
        receive: bool,
    ) -> Result<(), CoreError> {
        self.call(move |core| {
            core.store()?
                .set_sync_flags(&DeviceId(device_id), SyncFlags { send, receive })?;
            Ok(())
        })
        .await
    }

    pub async fn rename_this_device(&self, name: String) -> Result<(), CoreError> {
        let name = name.trim().to_owned();
        if name.is_empty() {
            return Err(failed("A device needs a name"));
        }
        self.call(move |core| {
            core.store()?.set_self_display_name(&name)?;
            Ok(())
        })
        .await
    }

    /// Trusts the desktop or relay in a scanned pairing code (SPEC 21.3.3).
    pub async fn pair(&self, link: String) -> Result<Paired, CoreError> {
        let link = PairLink::parse(&link).map_err(|e| CoreError::BadPairingCode {
            message: e.to_string(),
        })?;
        let entry = PeerKey {
            device_id: link.device_id.clone(),
            display_name: link.display_name.clone(),
            role: Role::Device,
            key: link.key,
            addrs: link.addrs.iter().map(ToString::to_string).collect(),
            trusted: true,
            updated_ms: 0,
        };
        self.call(move |core| {
            core.store()?.record_peer_key(&entry)?;
            Ok(())
        })
        .await?;
        self.retry();
        self.listener.changed();
        Ok(Paired {
            device_id: link.device_id.0,
            name: link.display_name,
        })
    }

    /// Stops trusting one device (SPEC 21.3.5).
    pub async fn forget(&self, device_id: String) -> Result<(), CoreError> {
        let id = DeviceId(device_id);
        let drop_id = id.clone();
        self.call(move |core| {
            core.store()?.forget_peer_key(&drop_id)?;
            Ok(())
        })
        .await?;
        if let Some(e) = self.engine.lock().unwrap().as_ref() {
            e.drop_session(&id);
        }
        self.listener.changed();
        Ok(())
    }

    pub async fn forget_all(&self) -> Result<(), CoreError> {
        self.call(|core| {
            core.store()?.forget_all_peer_keys()?;
            Ok(())
        })
        .await?;
        if let Some(e) = self.engine.lock().unwrap().as_ref() {
            e.reconnect_all();
        }
        self.listener.changed();
        Ok(())
    }

    pub async fn setting(&self, key: String) -> Result<Option<String>, CoreError> {
        self.call(move |core| Ok(core.store()?.setting(&key)?))
            .await
    }

    pub async fn set_setting(&self, key: String, value: String) -> Result<(), CoreError> {
        self.call(move |core| Ok(core.store()?.set_setting(&key, &value)?))
            .await
    }

    /// Whether this device is on the tailnet now (the Tailscale app is connected).
    pub fn tailnet_connected(&self) -> bool {
        tailnet_addrs().is_some()
    }
}

impl TailCopy {
    /// Runs `f` on the store thread from the core's own runtime, so the caller's
    /// executor (Swift, Kotlin) never needs to be tokio.
    async fn call<T, F>(&self, f: F) -> Result<T, CoreError>
    where
        T: Send + 'static,
        F: FnOnce(&mut actor::Core) -> anyhow::Result<T> + Send + 'static,
    {
        let actor = self.actor.clone();
        self.rt
            .spawn(async move { actor.run(f).await })
            .await
            .map_err(failed)?
            .map_err(failed)
    }
}

fn init_logging(dir: &std::path::Path) {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let appender = tracing_appender::rolling::Builder::new()
            .rotation(tracing_appender::rolling::Rotation::DAILY)
            .filename_prefix("tailcopy")
            .filename_suffix("log")
            .max_log_files(3)
            .build(dir.join("logs"));
        if let Ok(appender) = appender {
            let _ = tracing_subscriber::fmt()
                .with_writer(appender)
                .with_ansi(false)
                .with_env_filter(
                    tracing_subscriber::EnvFilter::try_from_default_env()
                        .unwrap_or_else(|_| "info".into()),
                )
                .try_init();
        }
    });
}
