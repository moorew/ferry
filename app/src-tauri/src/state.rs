//! Shared app state, the capture pipeline and the loop guard.

use std::path::PathBuf;
#[cfg(target_os = "linux")]
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::{Duration, Instant};

use tailcopy_core::{
    Capture, CaptureOutcome, CapturedFormats, ConfigFile, Hash32, PrivacyDecision,
    capture::normalize,
};
use tailcopy_platform::{Captured, Marker, Session};
use tauri::{AppHandle, Emitter, Manager};

use crate::db::Db;

/// Write events with the same group key within this long are our own (SPEC 7.6).
const LOOP_GUARD: Duration = Duration::from_secs(2);
/// One copy in another app can fire several change events; the same content again
/// within this long is the same copy.
const REPEAT_EVENT: Duration = Duration::from_millis(500);
/// Retention runs after this many captures, and on a timer.
const RETENTION_EVERY: u32 = 50;

#[cfg(windows)]
pub type Target = tailcopy_platform::windows::Target;
#[cfg(target_os = "macos")]
pub type Target = tailcopy_platform::macos::Target;

pub struct AppState {
    pub db: Db,
    pub config: Mutex<ConfigFile>,
    pub config_path: PathBuf,
    pub data_dir: PathBuf,
    pub session: Session,
    pub paused: AtomicBool,
    pub capture_ready: AtomicBool,
    /// The window or app focused when the picker opened.
    #[cfg(not(target_os = "linux"))]
    pub previous: Mutex<Option<Target>>,
    /// App id of the focused window when the shortcut fired (GNOME keystroke overrides).
    #[cfg(target_os = "linux")]
    pub focused_app: Mutex<String>,
    pub hotkey_at: Mutex<Option<Instant>>,
    guard: Mutex<Option<(Instant, Hash32)>>,
    last_capture: Mutex<Option<(Instant, Hash32)>>,
    pub focus_logged: AtomicBool,
    captures_since_retention: AtomicU32,
    #[cfg(target_os = "linux")]
    pub gnome: tokio::sync::OnceCell<Arc<tailcopy_platform::gnome::Gnome>>,
}

impl AppState {
    pub fn new(
        db: Db,
        config: ConfigFile,
        config_path: PathBuf,
        data_dir: PathBuf,
        session: Session,
    ) -> Self {
        AppState {
            db,
            config: Mutex::new(config),
            config_path,
            data_dir,
            session,
            paused: AtomicBool::new(false),
            capture_ready: AtomicBool::new(matches!(session, Session::Windows | Session::MacOs)),
            #[cfg(not(target_os = "linux"))]
            previous: Mutex::new(None),
            #[cfg(target_os = "linux")]
            focused_app: Mutex::new(String::new()),
            hotkey_at: Mutex::new(None),
            guard: Mutex::new(None),
            last_capture: Mutex::new(None),
            focus_logged: AtomicBool::new(false),
            captures_since_retention: AtomicU32::new(0),
            #[cfg(target_os = "linux")]
            gnome: tokio::sync::OnceCell::new(),
        }
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }

    pub fn first_run_done(&self) -> bool {
        self.data_dir.join("setup-complete").exists()
    }

    pub fn config(&self) -> tailcopy_core::Config {
        self.config.lock().unwrap().config.clone()
    }

    /// Remember that TailCopy is about to put `group_key` on the clipboard.
    pub fn guard_write(&self, group_key: Hash32) {
        *self.guard.lock().unwrap() = Some((Instant::now(), group_key));
    }

    fn is_own_write(&self, group_key: &Hash32) -> bool {
        matches!(&*self.guard.lock().unwrap(), Some((at, key)) if at.elapsed() < LOOP_GUARD && key == group_key)
    }

    /// True if this is another event for the copy we just stored.
    fn is_repeat(&self, group_key: &Hash32) -> bool {
        let mut last = self.last_capture.lock().unwrap();
        let repeat =
            matches!(&*last, Some((at, key)) if at.elapsed() < REPEAT_EVENT && key == group_key);
        *last = Some((Instant::now(), *group_key));
        repeat
    }

    #[cfg(target_os = "linux")]
    pub fn gnome(&self) -> Option<Arc<tailcopy_platform::gnome::Gnome>> {
        self.gnome.get().cloned()
    }
}

/// Runs one clipboard change through privacy rules and into the store.
pub async fn ingest(app: &AppHandle, captured: Captured) {
    let state = app.state::<AppState>();
    if state.is_paused() {
        return;
    }
    let config = state.config();
    let mut local_only = false;
    if config.privacy.respect_os_markers {
        match &captured.marker {
            Marker::DontSave(why) => {
                tracing::info!("skipped a clip marked secret ({why})");
                return;
            }
            Marker::DontSync(why) => {
                tracing::info!("keeping a clip on this device only ({why})");
                local_only = true;
            }
            Marker::None => {}
        }
    }
    match config.privacy_decision(captured.source_app.as_deref()) {
        PrivacyDecision::DontSave => {
            tracing::info!("skipped a clip from an ignored app");
            return;
        }
        PrivacyDecision::SaveLocalOnly => local_only = true,
        PrivacyDecision::Save => {}
    }
    if captured.is_empty() {
        return;
    }
    let formats = CapturedFormats {
        text: captured.text,
        html: captured.html,
        rtf: captured.rtf.map(String::into_bytes),
        image: captured.png,
    };
    let Some(normalized) = normalize(&formats, &config.size_limits()) else {
        return;
    };
    if state.is_own_write(&normalized.group_key) {
        tracing::debug!("ignored our own clipboard write");
        return;
    }
    if state.is_repeat(&normalized.group_key) {
        tracing::debug!("ignored a repeated change event");
        return;
    }
    let capture = Capture {
        formats,
        source_app: captured.source_app,
        local_only,
    };
    let retention = state
        .captures_since_retention
        .fetch_add(1, Ordering::Relaxed)
        + 1
        >= RETENTION_EVERY;
    let policy = config.retention_policy();
    let outcome = state
        .db
        .run(move |core| {
            let outcome = core.store.capture(&capture);
            if retention && let Err(e) = core.store.enforce_retention(&policy) {
                tracing::warn!("retention failed: {e}");
            }
            core.refresh();
            outcome
        })
        .await;
    if retention {
        state.captures_since_retention.store(0, Ordering::Relaxed);
    }
    match outcome {
        Ok(CaptureOutcome::Added {
            clip_id,
            local_only,
            ..
        }) => {
            tracing::info!(
                clip = %clip_id,
                kind = ?normalized.kind,
                bytes = normalized.formats.iter().map(|(_, b)| b.len()).sum::<usize>(),
                local_only,
                "captured"
            );
            clips_changed(app);
        }
        Ok(CaptureOutcome::Ignored) => {}
        Err(e) => tracing::warn!("capture failed: {e}"),
    }
}

/// Tells both windows the list changed (SPEC 10.7, debounced by the UI's own fetches).
pub fn clips_changed(app: &AppHandle) {
    let _ = app.emit("clips-changed", ());
}

/// Watches the clipboard on Windows and macOS; GNOME capture runs in `gnome_loop`.
#[cfg(any(windows, target_os = "macos"))]
pub fn start_capture(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    use tailcopy_platform::macos as backend;
    #[cfg(windows)]
    use tailcopy_platform::windows as backend;

    let rx = match backend::watch() {
        Ok(rx) => rx,
        Err(e) => {
            tracing::error!("clipboard watcher failed to start: {e}");
            app.state::<AppState>()
                .capture_ready
                .store(false, Ordering::Relaxed);
            return;
        }
    };
    let app = app.clone();
    std::thread::Builder::new()
        .name("capture".into())
        .spawn(move || {
            for () in rx {
                if app.state::<AppState>().is_paused() {
                    continue;
                }
                match backend::read() {
                    Ok(captured) => tauri::async_runtime::block_on(ingest(&app, captured)),
                    Err(e) => tracing::warn!("reading the clipboard failed: {e}"),
                }
            }
        })
        .expect("capture thread");
}

pub fn status_changed(app: &AppHandle) {
    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        let status = crate::commands::build_status(&app2).await;
        let _ = app2.emit("status-changed", status);
        crate::tray::refresh(&app2);
    });
}

pub fn set_paused(app: &AppHandle, paused: bool) {
    app.state::<AppState>()
        .paused
        .store(paused, Ordering::Relaxed);
    tracing::info!(paused, "capture paused state changed");
    status_changed(app);
}
