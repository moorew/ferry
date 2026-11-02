//! M0 macOS clipboard spike (SPEC 9.3, 17 items 2 and 5).
//!
//! Checks and prompts for Accessibility, polls the pasteboard every 250 ms with
//! clipboard-rs, reads text, HTML, RTF and PNG (TIFF converted), honours the
//! nspasteboard.org markers, shows a Tauri picker on Cmd+Shift+V, writes the chosen
//! clip as one pasteboard item, reactivates the previous app and sends Cmd+V with
//! enigo after the modifier wait. Ctrl+Opt+Cmd+V (a test-only stand-in for a global
//! paste-plain shortcut) pastes the current clipboard as plain text.

use std::{
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};

use clipboard_rs::{
    Clipboard, ClipboardContent, ClipboardContext, ClipboardHandler, ClipboardWatcher,
    ClipboardWatcherContext, ContentFormat, RustImageData, common::RustImage,
};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_global_shortcut::{Shortcut, ShortcutState};

mod mac;

const PICKER: &str = "cmd+shift+v";
const PASTE_PLAIN: &str = "ctrl+alt+cmd+v";
const MAX_CLIPS: usize = 20;

#[derive(Clone, Default, Serialize)]
struct Clip {
    #[serde(skip)]
    text: Option<String>,
    #[serde(skip)]
    html: Option<String>,
    #[serde(skip)]
    rtf: Option<String>,
    #[serde(skip)]
    png: Option<Vec<u8>>,
    preview: String,
    formats: Vec<&'static str>,
    source_app: String,
}

#[derive(Default)]
struct Inner {
    clips: Vec<Clip>,
    previous: Option<mac::Target>,
    hotkey_at: Option<Instant>,
    last_write: Option<(Instant, Option<String>)>,
}

struct Spike {
    inner: Mutex<Inner>,
    started: Instant,
}

impl Spike {
    fn log(&self, msg: impl AsRef<str>) {
        let line = format!(
            "[{:>9.1} ms] {}",
            self.started.elapsed().as_secs_f64() * 1000.0,
            msg.as_ref()
        );
        println!("{line}");
        if let Some(home) = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }) {
            let path = std::path::Path::new(&home).join("tailcopy-m0.log");
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
            {
                use std::io::Write;
                let _ = writeln!(file, "{line}");
            }
        }
    }
}

struct Notify(mpsc::Sender<()>);

impl ClipboardHandler for Notify {
    fn on_clipboard_change(&mut self) {
        let _ = self.0.send(());
    }
}

fn capture(spike: &Spike, app: &AppHandle) {
    let t = Instant::now();
    let Ok(ctx) = ClipboardContext::new() else {
        spike.log("could not open the clipboard");
        return;
    };
    let types = ctx.available_formats().unwrap_or_default();
    let source_app = mac::source_app(&ctx, &types);
    spike.log(format!("clipboard changed ({source_app}): {types:?}"));

    if let mac::Marker::DontSave(why) = mac::secret_marker(&types) {
        spike.log(format!("  skipped: {why}"));
        return;
    }

    let text = ctx
        .has(ContentFormat::Text)
        .then(|| ctx.get_text().ok())
        .flatten()
        .filter(|t| !t.trim().is_empty());
    let html = ctx
        .has(ContentFormat::Html)
        .then(|| ctx.get_html().ok())
        .flatten();
    let rtf = ctx
        .has(ContentFormat::Rtf)
        .then(|| ctx.get_rich_text().ok())
        .flatten();
    let png = ctx
        .has(ContentFormat::Image)
        .then(|| ctx.get_image().ok())
        .flatten()
        .and_then(|img| img.to_png().ok())
        .map(|buf| buf.get_bytes().to_vec());

    {
        let inner = spike.inner.lock().unwrap();
        if let Some((at, written)) = &inner.last_write
            && at.elapsed() < Duration::from_secs(2)
            && written == &text
        {
            spike.log("  loop guard: our own write, not stored");
            return;
        }
    }

    let mut clip = Clip {
        preview: text
            .as_deref()
            .map(|t| t.chars().take(80).collect())
            .unwrap_or_else(|| "image".into()),
        source_app,
        ..Default::default()
    };
    for (present, name) in [
        (text.is_some(), "text"),
        (html.is_some(), "html"),
        (rtf.is_some(), "rtf"),
        (png.is_some(), "png"),
    ] {
        if present {
            clip.formats.push(name);
        }
    }
    if clip.formats.is_empty() {
        spike.log("  nothing to store");
        return;
    }
    clip.text = text;
    clip.html = html;
    clip.rtf = rtf;
    clip.png = png;
    spike.log(format!(
        "  read {:?} in {} ms (png {} bytes)",
        clip.formats,
        t.elapsed().as_millis(),
        clip.png.as_ref().map_or(0, Vec::len)
    ));

    let mut inner = spike.inner.lock().unwrap();
    inner.clips.insert(0, clip);
    inner.clips.truncate(MAX_CLIPS);
    let _ = app.emit("clips", &inner.clips);
}

fn on_picker_hotkey(app: &AppHandle, spike: &Spike) {
    let previous = mac::frontmost();
    {
        let mut inner = spike.inner.lock().unwrap();
        inner.hotkey_at = Some(Instant::now());
        if previous.is_some() {
            inner.previous = previous.clone();
        }
        let _ = app.emit("clips", &inner.clips);
    }
    spike.log(format!(
        "picker hotkey; previous window {:?}",
        previous.as_ref().map(|p| p.describe())
    ));
    let window = app.get_webview_window("picker").expect("picker window");
    let _ = window.center();
    let _ = window.show();
    let _ = window.set_focus();
    spike.log("picker shown");
}

fn on_paste_plain_hotkey(spike: &Spike) {
    let text = ClipboardContext::new().ok().and_then(|c| c.get_text().ok());
    let Some(text) = text else {
        spike.log("paste-plain: no text on the clipboard");
        return;
    };
    spike.log("paste-plain hotkey");
    let clip = Clip {
        text: Some(text),
        ..Default::default()
    };
    write_clip(spike, &clip, true);
    paste(spike, None);
}

fn write_clip(spike: &Spike, clip: &Clip, plain: bool) {
    spike.inner.lock().unwrap().last_write = Some((Instant::now(), clip.text.clone()));
    let t = Instant::now();
    let result = write_pasteboard(clip, plain);
    match result {
        Ok(landed) => spike.log(format!(
            "wrote {landed:?} in {} ms",
            t.elapsed().as_millis()
        )),
        Err(e) => spike.log(format!("write failed: {e}")),
    }
}

/// One pasteboard item carrying every format (clipboard-rs `set` on macOS).
fn write_pasteboard(clip: &Clip, plain: bool) -> Result<Vec<&'static str>, String> {
    let ctx = ClipboardContext::new().map_err(|e| e.to_string())?;
    let mut contents = Vec::new();
    let mut landed = Vec::new();
    if let Some(t) = &clip.text {
        contents.push(ClipboardContent::Text(t.clone()));
        landed.push("text");
    }
    if !plain {
        if let Some(h) = &clip.html {
            contents.push(ClipboardContent::Html(h.clone()));
            landed.push("html");
        }
        if let Some(r) = &clip.rtf {
            contents.push(ClipboardContent::Rtf(r.clone()));
            landed.push("rtf");
        }
        if let Some(p) = &clip.png {
            let img = RustImageData::from_bytes(p).map_err(|e| e.to_string())?;
            contents.push(ClipboardContent::Image(img));
            landed.push("png");
        }
    }
    ctx.set(contents).map_err(|e| e.to_string())?;
    Ok(landed)
}

/// Reactivate the previous app (when `target` is set), wait for modifiers, then Cmd+V.
fn paste(spike: &Spike, target: Option<mac::Target>) {
    if !mac::accessibility_trusted() {
        spike.log("No Accessibility permission. Copied. Press Cmd+V.");
        return;
    }
    if let Some(target) = target {
        let ok = target.activate();
        std::thread::sleep(Duration::from_millis(50));
        spike.log(format!(
            "reactivated {}: {ok}; frontmost is now {:?}",
            target.describe(),
            mac::frontmost().map(|f| f.describe())
        ));
    }
    let t = Instant::now();
    if !mac::wait_for_modifiers_released(Duration::from_millis(500)) {
        spike.log("modifiers still held after 500 ms. Copied. Press Cmd+V.");
        return;
    }
    let waited = t.elapsed().as_millis();
    match mac::send_cmd_v() {
        Ok(()) => spike.log(format!("Cmd+V sent (waited {waited} ms for modifiers)")),
        Err(e) => spike.log(format!("Cmd+V failed: {e}")),
    }
}

#[tauri::command]
fn picker_focused(spike: State<'_, Arc<Spike>>) {
    if let Some(at) = spike.inner.lock().unwrap().hotkey_at.take() {
        spike.log(format!(
            "picker focused (webview) {:.1} ms after the hotkey",
            at.elapsed().as_secs_f64() * 1000.0
        ));
    }
}

#[tauri::command]
fn hide_picker(app: AppHandle) {
    if let Some(w) = app.get_webview_window("picker") {
        let _ = w.hide();
    }
}

#[tauri::command]
async fn choose(
    app: AppHandle,
    spike: State<'_, Arc<Spike>>,
    index: usize,
    plain: bool,
) -> Result<(), String> {
    let spike = spike.inner().clone();
    let (clip, previous) = {
        let inner = spike.inner.lock().unwrap();
        let clip = inner.clips.get(index).cloned().ok_or("no such clip")?;
        (clip, inner.previous.clone())
    };
    spike.log(format!("choose #{index} plain={plain}"));
    write_clip(&spike, &clip, plain);
    if let Some(w) = app.get_webview_window("picker") {
        let _ = w.hide();
    }
    tauri::async_runtime::spawn_blocking(move || paste(&spike, previous))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn verify_offer(app: AppHandle, spike: State<'_, Arc<Spike>>) {
    if let Ok(ctx) = ClipboardContext::new() {
        spike.log(format!(
            "current clipboard formats: {:?}",
            ctx.available_formats().unwrap_or_default()
        ));
    }
    let _ = app.emit("verified", ());
}

fn main() {
    let spike = Arc::new(Spike {
        inner: Mutex::default(),
        started: Instant::now(),
    });
    let hotkeys = spike.clone();
    let paste_plain: Shortcut = PASTE_PLAIN.parse().expect("valid shortcut");
    tauri::Builder::default()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_shortcuts([PICKER, PASTE_PLAIN])
                .expect("valid shortcuts")
                .with_handler(move |app, shortcut, event| {
                    if event.state != ShortcutState::Pressed {
                        return;
                    }
                    if *shortcut == paste_plain {
                        let spike = hotkeys.clone();
                        std::thread::spawn(move || on_paste_plain_hotkey(&spike));
                    } else {
                        on_picker_hotkey(app, &hotkeys);
                    }
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            picker_focused,
            hide_picker,
            choose,
            verify_offer
        ])
        .setup(move |app| {
            app.manage(spike.clone());
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            let trusted = mac::accessibility_trusted();
            spike.log(format!(
                "ready: {PICKER} opens the picker, {PASTE_PLAIN} pastes plain text; accessibility trusted={trusted}"
            ));
            if !trusted {
                spike.log("asking for Accessibility permission; grant it, then restart the spike");
                mac::prompt_accessibility();
            }
            let (tx, rx) = mpsc::channel();
            std::thread::spawn(move || {
                let mut watcher = ClipboardWatcherContext::new_with_interval(Duration::from_millis(250))
                    .expect("clipboard watcher");
                watcher.add_handler(Notify(tx)).start_watch();
            });
            let handle = app.handle().clone();
            let capturer = spike.clone();
            std::thread::spawn(move || {
                for () in rx {
                    capture(&capturer, &handle);
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running the spike");
}
