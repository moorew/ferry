//! The picker and manager windows, and choosing a clip (SPEC 9.1, 10.1, 10.2).

use std::time::Instant;

use tailcopy_core::{ClipId, Hash32, names::PICKER_TITLE};
use tailcopy_platform::{CopyOnlyReason, PasteResult, Write, WriteOutcome};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::state::{AppState, clips_changed};

pub const PICKER: &str = "picker";
pub const MANAGER: &str = "manager";

/// Default and smallest picker size, logical px (handoff tokens `size.picker`).
const PICKER_SIZE: (f64, f64) = (960.0, 640.0);
const PICKER_MIN: (f64, f64) = (640.0, 420.0);

fn size_file(app: &AppHandle) -> std::path::PathBuf {
    app.state::<AppState>().data_dir.join("picker-size")
}

/// The size the picker had when it was last hidden ("960x640").
fn saved_size(app: &AppHandle) -> (f64, f64) {
    std::fs::read_to_string(size_file(app))
        .ok()
        .and_then(|s| {
            let (w, h) = s.trim().split_once('x')?;
            Some((w.parse().ok()?, h.parse().ok()?))
        })
        .map_or(PICKER_SIZE, |(w, h): (f64, f64)| {
            (w.max(PICKER_MIN.0), h.max(PICKER_MIN.1))
        })
}

fn save_size(app: &AppHandle, window: &WebviewWindow) {
    let (Ok(size), Ok(scale)) = (window.inner_size(), window.scale_factor()) else {
        return;
    };
    let logical = size.to_logical::<f64>(scale);
    let text = format!("{}x{}", logical.width.round(), logical.height.round());
    if saved_size(app) != (logical.width.round(), logical.height.round()) {
        let _ = std::fs::write(size_file(app), text);
    }
}

pub fn create_picker(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let (width, height) = saved_size(app);
    WebviewWindowBuilder::new(app, PICKER, WebviewUrl::App("index.html".into()))
        .title(PICKER_TITLE)
        .inner_size(width, height)
        .min_inner_size(PICKER_MIN.0, PICKER_MIN.1)
        .resizable(true)
        .decorations(false)
        .transparent(true)
        .shadow(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible(false)
        .on_navigation(stays_in_app)
        .build()
}

/// The windows only ever show TailCopy's own pages. A link inside a copied web page
/// must not turn the picker or the manager into someone else's site.
fn stays_in_app(url: &tauri::Url) -> bool {
    let host = url.host_str().unwrap_or_default();
    let ok = url.scheme() == "tauri"
        || host == "tauri.localhost"
        || (cfg!(debug_assertions) && matches!(host, "localhost" | "127.0.0.1"));
    if !ok {
        tracing::warn!("blocked a window navigating away from TailCopy");
    }
    ok
}

/// Shows the manager, creating it the first time, optionally at a route.
pub fn show_manager(app: &AppHandle, route: Option<&str>) {
    let window = match app.get_webview_window(MANAGER) {
        Some(w) => w,
        None => {
            match WebviewWindowBuilder::new(app, MANAGER, WebviewUrl::App("index.html".into()))
                .title("TailCopy")
                .inner_size(960.0, 640.0)
                .min_inner_size(720.0, 480.0)
                .visible(false)
                .on_navigation(stays_in_app)
                .build()
            {
                Ok(w) => w,
                Err(e) => {
                    tracing::error!("could not open the manager: {e}");
                    return;
                }
            }
        }
    };
    crate::tray::set_new(app, false);
    #[cfg(target_os = "macos")]
    let _ = app.show();
    let _ = window.show();
    let _ = window.unminimize();
    let _ = window.set_focus();
    if let Some(route) = route {
        let _ = app.emit_to(MANAGER, "navigate", route);
    }
}

pub fn picker_visible(app: &AppHandle) -> bool {
    app.get_webview_window(PICKER)
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false)
}

pub fn hide_picker(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(PICKER) {
        save_size(app, &w);
        let _ = w.hide();
    }
}

/// The picker shortcut: toggles the picker over whatever is focused.
pub async fn toggle_picker(app: &AppHandle) {
    if picker_visible(app) {
        hide_picker(app);
        return;
    }
    let state = app.state::<AppState>();
    *state.hotkey_at.lock().unwrap() = Some(Instant::now());
    state
        .focus_logged
        .store(false, std::sync::atomic::Ordering::Relaxed);
    #[cfg(windows)]
    {
        *state.previous.lock().unwrap() = tailcopy_platform::windows::foreground();
    }
    #[cfg(target_os = "macos")]
    {
        *state.previous.lock().unwrap() = tailcopy_platform::macos::frontmost();
    }
    let Some(window) = app.get_webview_window(PICKER) else {
        return;
    };
    #[cfg(not(target_os = "linux"))]
    center_on_active_monitor(&window);
    let _ = app.emit_to(PICKER, "picker-shown", ());
    crate::tray::set_new(app, false);
    #[cfg(target_os = "macos")]
    let _ = app.show();
    let _ = window.show();
    let _ = window.set_focus();
    #[cfg(target_os = "linux")]
    if let Some(gnome) = state.gnome()
        && let Err(e) = gnome.present_picker().await
    {
        tracing::warn!("PresentPicker failed: {e}");
    }
    log_since_hotkey(&state, "picker shown");
}

pub fn log_since_hotkey(state: &AppState, what: &str) {
    if let Some(at) = *state.hotkey_at.lock().unwrap() {
        tracing::info!(ms = at.elapsed().as_secs_f64() * 1000.0, "{what}");
    }
}

#[cfg(not(target_os = "linux"))]
fn center_on_active_monitor(window: &WebviewWindow) {
    let (Ok(cursor), Ok(size)) = (window.cursor_position(), window.outer_size()) else {
        return;
    };
    let Ok(Some(monitor)) = window.monitor_from_point(cursor.x, cursor.y) else {
        return;
    };
    let (mp, ms) = (monitor.position(), monitor.size());
    // A little above centre reads as centred.
    let x = mp.x + (ms.width as i32 - size.width as i32) / 2;
    let y = mp.y + (ms.height as i32 - size.height as i32) * 2 / 5;
    let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
}

/// A clip's stored formats, owned so they can cross to the GTK or blocking thread.
#[derive(Clone, Default)]
pub struct Payload {
    pub text: Option<String>,
    pub html: Option<String>,
    pub rtf: Option<String>,
    pub png: Option<Vec<u8>>,
    pub group_key: Option<Hash32>,
}

impl Payload {
    pub fn write(&self, plain: bool) -> Write<'_> {
        let all = Write {
            text: self.text.as_deref(),
            html: self.html.as_deref(),
            rtf: self.rtf.as_deref(),
            png: self.png.as_deref(),
        };
        // Plain paste of an image still pastes the image.
        if plain && all.text.is_some() {
            all.plain()
        } else {
            all
        }
    }
}

pub async fn load(state: &AppState, id: &str) -> Option<Payload> {
    let id = ClipId::from(id);
    state
        .db
        .run(move |core| {
            let view = core.store.view();
            let detail = view.get_clip(&id).ok().flatten()?;
            let read = |mime: &str| view.read_format(&id, mime).ok().flatten();
            let text = |mime: &str| read(mime).map(|b| String::from_utf8_lossy(&b).into_owned());
            Some(Payload {
                text: text("text/plain"),
                html: text("text/html"),
                rtf: text("text/rtf"),
                png: read("image/png"),
                group_key: Some(detail.clip.group_key),
            })
        })
        .await
}

/// Puts `payload` on the clipboard. On GNOME this is the focused write, so call it
/// while a TailCopy window has focus.
pub async fn write(app: &AppHandle, payload: Payload, plain: bool) -> WriteOutcome {
    if let Some(key) = payload.group_key {
        app.state::<AppState>().guard_write(key);
    }
    #[cfg(target_os = "linux")]
    {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let ran = app.run_on_main_thread(move || {
            let _ = tx.send(tailcopy_platform::gnome::gtk_write(&payload.write(plain)));
        });
        if ran.is_err() {
            return WriteOutcome::default();
        }
        rx.await.unwrap_or_default()
    }
    #[cfg(not(target_os = "linux"))]
    {
        #[cfg(target_os = "macos")]
        use tailcopy_platform::macos as backend;
        #[cfg(windows)]
        use tailcopy_platform::windows as backend;
        tauri::async_runtime::spawn_blocking(move || backend::write(&payload.write(plain)))
            .await
            .ok()
            .and_then(|r| {
                r.map_err(|e| tracing::warn!("clipboard write failed: {e}"))
                    .ok()
            })
            .unwrap_or_default()
    }
}

/// Enter or Shift+Enter in the picker (SPEC 10.2).
pub async fn choose(app: &AppHandle, id: &str, plain: bool) {
    let state = app.state::<AppState>();
    if !crate::sync::ensure_present(app, id, None).await {
        hide_picker(app);
        notify(app, "Not available yet (source offline)");
        return;
    }
    let Some(payload) = load(&state, id).await else {
        hide_picker(app);
        return;
    };
    let outcome = write(app, payload, plain).await;
    hide_picker(app);
    record_use(app, id).await;
    if outcome.landed.is_empty() {
        notify(app, "Couldn't put that on the clipboard. Try again.");
        return;
    }
    if !state.config().general.auto_paste {
        return;
    }
    let result = inject(app, true).await;
    log_since_hotkey(&state, "paste sent");
    if let PasteResult::CopiedOnly(reason) = result {
        notify(app, &reason.message(&keystroke_label(app)));
    }
}

/// Copy without pasting (Ctrl+C in the picker, Copy in the manager).
pub async fn copy(app: &AppHandle, id: &str) {
    let state = app.state::<AppState>();
    if !crate::sync::ensure_present(app, id, None).await {
        notify(app, "Not available yet (source offline)");
        return;
    }
    if let Some(payload) = load(&state, id).await {
        write(app, payload, false).await;
        record_use(app, id).await;
    }
}

async fn record_use(app: &AppHandle, id: &str) {
    let id = ClipId::from(id);
    let state = app.state::<AppState>();
    let result = state
        .db
        .run(move |core| {
            let r = core.store.use_clip(&id);
            core.refresh();
            r
        })
        .await;
    if let Err(e) = result {
        tracing::warn!("recording use failed: {e}");
    }
    clips_changed(app);
}

/// Sends the paste keystroke into the previous app (or the focused one).
async fn inject(app: &AppHandle, to_previous: bool) -> PasteResult {
    let state = app.state::<AppState>();
    #[cfg(windows)]
    {
        let target = if to_previous {
            *state.previous.lock().unwrap()
        } else {
            None
        };
        tauri::async_runtime::spawn_blocking(move || tailcopy_platform::windows::paste(target))
            .await
            .unwrap_or(PasteResult::CopiedOnly(CopyOnlyReason::Failed))
    }
    #[cfg(target_os = "macos")]
    {
        let target = if to_previous {
            state.previous.lock().unwrap().clone()
        } else {
            None
        };
        tauri::async_runtime::spawn_blocking(move || {
            tailcopy_platform::macos::paste(target.as_ref())
        })
        .await
        .unwrap_or(PasteResult::CopiedOnly(CopyOnlyReason::Failed))
    }
    #[cfg(target_os = "linux")]
    {
        let Some(gnome) = state.gnome() else {
            return PasteResult::CopiedOnly(CopyOnlyReason::Unsupported);
        };
        gnome.paste(linux_keystroke(&state), to_previous).await
    }
}

#[cfg(target_os = "linux")]
fn linux_keystroke(state: &AppState) -> tailcopy_platform::Keystroke {
    let app_id = state.focused_app.lock().unwrap().clone();
    let overrides: Vec<(String, String)> =
        state.config().linux.paste_keystroke.into_iter().collect();
    tailcopy_platform::keystroke_for(&app_id, &overrides, tailcopy_platform::Keystroke::CtrlV)
}

fn keystroke_label(app: &AppHandle) -> String {
    #[cfg(target_os = "macos")]
    {
        let _ = app;
        "⌘V".into()
    }
    #[cfg(windows)]
    {
        let _ = app;
        "Ctrl+V".into()
    }
    #[cfg(target_os = "linux")]
    {
        match linux_keystroke(&app.state::<AppState>()) {
            tailcopy_platform::Keystroke::CtrlShiftV => "Ctrl+Shift+V".into(),
            tailcopy_platform::Keystroke::ShiftInsert => "Shift+Insert".into(),
            _ => "Ctrl+V".into(),
        }
    }
}

/// The global paste-as-plain-text shortcut: what's on the clipboard, without formatting.
pub async fn paste_plain(app: &AppHandle) {
    let state = app.state::<AppState>();
    #[cfg(not(target_os = "linux"))]
    let text = {
        #[cfg(target_os = "macos")]
        use tailcopy_platform::macos as backend;
        #[cfg(windows)]
        use tailcopy_platform::windows as backend;
        tauri::async_runtime::spawn_blocking(|| backend::read().ok().and_then(|c| c.text))
            .await
            .ok()
            .flatten()
    };
    #[cfg(target_os = "linux")]
    let text = match state.gnome() {
        Some(g) => match g.mimetypes().await {
            Ok(m) => g.read(&m, "").await.ok().and_then(|c| c.text),
            Err(_) => None,
        },
        None => None,
    };
    let Some(text) = text.filter(|t| !t.is_empty()) else {
        return;
    };
    let formats = tailcopy_core::CapturedFormats {
        text: Some(text.clone()),
        ..Default::default()
    };
    if let Some(n) = tailcopy_core::capture::normalize(&formats, &state.config().size_limits()) {
        state.guard_write(n.group_key);
    }
    #[cfg(target_os = "linux")]
    {
        let Some(g) = state.gnome() else { return };
        if let Err(e) = g.set_text(&text).await {
            tracing::warn!("plain write failed: {e}");
            return;
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let payload = Payload {
            text: Some(text),
            ..Payload::default()
        };
        write(app, payload, true).await;
    }
    if let PasteResult::CopiedOnly(reason) = inject(app, false).await {
        notify(app, &reason.message(&keystroke_label(app)));
    }
}

/// Only for problems (SPEC 1.1: no notifications unless something is wrong).
pub fn notify(app: &AppHandle, message: &str) {
    use tauri_plugin_notification::NotificationExt;
    if let Err(e) = app
        .notification()
        .builder()
        .title("TailCopy")
        .body(message)
        .show()
    {
        tracing::warn!("notification failed: {e}");
    }
}
