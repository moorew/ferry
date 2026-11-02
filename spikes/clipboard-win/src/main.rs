//! M0 Windows clipboard spike (SPEC 9.2, 17 items 1 and 5).
//!
//! Watches the clipboard with clipboard-rs, reads text, HTML, RTF and PNG, honours
//! the Windows secret markers, shows a Tauri picker on Ctrl+Alt+V, writes the chosen
//! clip with every format in one clipboard session, restores focus to the previous
//! window and sends Ctrl+V with enigo after the modifier wait. Ctrl+Alt+Shift+V
//! pastes the current clipboard as plain text. Prints timestamps throughout.

use std::{
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};

use clipboard_rs::{
    Clipboard, ClipboardContext, ClipboardHandler, ClipboardWatcher, ClipboardWatcherContext,
    ContentFormat, common::RustImage,
};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_global_shortcut::{Shortcut, ShortcutState};

mod win;

const PICKER: &str = "ctrl+alt+v";
const PASTE_PLAIN: &str = "ctrl+alt+shift+v";
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
    previous: Option<win::Target>,
    hotkey_at: Option<Instant>,
    last_write: Option<(Instant, Option<String>)>,
}

struct Spike {
    inner: Mutex<Inner>,
    started: Instant,
}

impl Spike {
    fn log(&self, msg: impl AsRef<str>) {
        println!(
            "[{:>9.1} ms] {}",
            self.started.elapsed().as_secs_f64() * 1000.0,
            msg.as_ref()
        );
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
    let source_app = win::clipboard_source_app().unwrap_or_default();
    spike.log(format!("clipboard changed ({source_app}): {types:?}"));

    match win::secret_marker(&ctx) {
        win::Marker::DontSave(why) => {
            spike.log(format!("  skipped: {why}"));
            return;
        }
        win::Marker::DontSync(why) => spike.log(format!("  would save but not sync: {why}")),
        win::Marker::None => {}
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
    let previous = win::foreground();
    {
        let mut inner = spike.inner.lock().unwrap();
        inner.hotkey_at = Some(Instant::now());
        if previous.is_some() {
            inner.previous = previous;
        }
        let _ = app.emit("clips", &inner.clips);
    }
    spike.log(format!(
        "picker hotkey; previous window {:?}",
        previous.map(|p| p.describe())
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
    let result = if std::env::var_os("TAILCOPY_WRITER_CLIPBOARD_RS").is_some() {
        win::write_with_clipboard_rs(
            clip.text.clone(),
            clip.html.clone(),
            clip.rtf.clone(),
            clip.png.clone(),
            plain,
        )
    } else {
        win::write_one_session(
            clip.text.as_deref(),
            clip.html.as_deref(),
            clip.rtf.as_deref(),
            clip.png.as_deref(),
            plain,
        )
    };
    match result {
        Ok(landed) => spike.log(format!(
            "wrote {landed:?} in {} ms",
            t.elapsed().as_millis()
        )),
        Err(e) => spike.log(format!("write failed: {e}")),
    }
}

/// Restore focus (when `target` is set), wait for modifiers, then send Ctrl+V.
fn paste(spike: &Spike, target: Option<win::Target>) {
    if let Some(target) = target {
        if target.is_elevated() && !win::self_is_elevated() {
            spike.log("Copied. Press Ctrl+V to paste (admin window).");
            return;
        }
        let ok = target.activate();
        std::thread::sleep(Duration::from_millis(30));
        spike.log(format!(
            "focus restored: {ok}; foreground is now {:?}",
            win::foreground().map(|f| f.describe())
        ));
    }
    let t = Instant::now();
    if !win::wait_for_modifiers_released(Duration::from_millis(500)) {
        spike.log("modifiers still held after 500 ms. Copied. Press Ctrl+V.");
        return;
    }
    let waited = t.elapsed().as_millis();
    match win::send_ctrl_v() {
        Ok(()) => spike.log(format!("Ctrl+V sent (waited {waited} ms for modifiers)")),
        Err(e) => spike.log(format!("Ctrl+V failed: {e}")),
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
        (clip, inner.previous)
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
            spike.log(format!(
                "ready: {PICKER} opens the picker, {PASTE_PLAIN} pastes plain text; elevated={}",
                win::self_is_elevated()
            ));
            let (tx, rx) = mpsc::channel();
            std::thread::spawn(move || {
                let mut watcher = ClipboardWatcherContext::new().expect("clipboard watcher");
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
