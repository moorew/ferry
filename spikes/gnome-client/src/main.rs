//! M0 GNOME client spike (SPEC 9.4, 17 item 3).
//!
//! Registers with the Ferry Shell extension over zbus, captures every clipboard
//! change it reports, shows a Tauri picker on the extension's shortcut, writes the
//! chosen clip with the GTK multi-format focused write, and pastes through the
//! extension. Prints timestamps for hotkey, show, focus and paste.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use futures_util::StreamExt;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use zbus::{Connection, OwnedVariant, proxy};

const PICKER_TITLE: &str = "Ferry Picker";
const EXTENSION_UUID: &str = "ferry@moorew.github.io";
const CLIENT_API: u32 = 1;
const MAX_CLIPS: usize = 20;

const TEXT_MIMES: &[&str] = &[
    "text/plain;charset=utf-8",
    "UTF8_STRING",
    "text/plain",
    "STRING",
];
const HTML_MIMES: &[&str] = &["text/html"];
const RTF_MIMES: &[&str] = &["text/rtf", "application/rtf"];
const PNG_MIMES: &[&str] = &["image/png"];
const PASSWORD_HINT: &str = "x-kde-passwordManagerHint";

#[proxy(
    interface = "org.gnome.Shell.Extensions.Ferry",
    default_service = "org.gnome.Shell",
    default_path = "/org/gnome/Shell/Extensions/Ferry"
)]
trait Ferry {
    fn register(&self, client_api: u32) -> zbus::Result<u32>;
    fn get_mime_types(&self) -> zbus::Result<Vec<String>>;
    fn get_content(&self, mimetype: &str) -> zbus::Result<Vec<u8>>;
    fn set_content(&self, mimetype: &str, data: &[u8]) -> zbus::Result<()>;
    fn set_shortcut(&self, action: &str, accelerators: &[&str]) -> zbus::Result<()>;
    fn present_picker(&self, pid: u32, title: &str) -> zbus::Result<()>;
    fn paste(&self, keystroke: &str, activate_previous: bool) -> zbus::Result<()>;

    #[zbus(signal)]
    fn clipboard_changed(&self, mimetypes: Vec<String>, source_app_id: String) -> zbus::Result<()>;
    #[zbus(signal)]
    fn shortcut_activated(&self, action: String, focused_app_id: String) -> zbus::Result<()>;
}

#[proxy(
    interface = "org.gnome.Shell.Extensions",
    default_service = "org.gnome.Shell",
    default_path = "/org/gnome/Shell"
)]
trait ShellExtensions {
    #[zbus(signal)]
    fn extension_state_changed(
        &self,
        uuid: String,
        state: HashMap<String, OwnedVariant>,
    ) -> zbus::Result<()>;
}

#[derive(Clone, Default, Serialize)]
struct Clip {
    #[serde(skip)]
    text: Option<String>,
    #[serde(skip)]
    html: Option<Vec<u8>>,
    #[serde(skip)]
    rtf: Option<Vec<u8>>,
    #[serde(skip)]
    png: Option<Vec<u8>>,
    preview: String,
    formats: Vec<&'static str>,
    source_app: String,
}

#[derive(Default)]
struct Inner {
    clips: Vec<Clip>,
    focused_app: String,
    hotkey_at: Option<Instant>,
    last_write: Option<(Instant, String)>,
}

struct Spike {
    proxy: FerryProxy<'static>,
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

fn shortcut(var: &str, default: &str) -> String {
    std::env::var(var).unwrap_or_else(|_| default.to_string())
}

/// SPEC 9.8: terminals want Ctrl+Shift+V.
fn keystroke_for(app_id: &str) -> &'static str {
    const SHIFT_APPS: &[&str] = &[
        "org.gnome.Ptyxis",
        "org.gnome.Terminal",
        "org.gnome.Console",
        "kitty",
        "Alacritty",
        "org.wezfurlong.wezterm",
        "com.mitchellh.ghostty",
        "com.gexperts.Tilix",
    ];
    if SHIFT_APPS.contains(&app_id) {
        "ctrl-shift-v"
    } else {
        "ctrl-v"
    }
}

async fn register(spike: &Spike) -> zbus::Result<()> {
    let api = spike.proxy.register(CLIENT_API).await?;
    let picker = shortcut("FERRY_PICKER", "<Control><Alt>v");
    let plain = shortcut("FERRY_PASTE_PLAIN", "<Control><Alt><Shift>v");
    spike.proxy.set_shortcut("picker", &[&picker]).await?;
    spike.proxy.set_shortcut("paste-plain", &[&plain]).await?;
    spike.log(format!(
        "registered, extension API {api}; picker {picker}, paste-plain {plain}"
    ));
    Ok(())
}

async fn first_available(spike: &Spike, offered: &[String], wanted: &[&str]) -> Option<Vec<u8>> {
    let mime = wanted.iter().find(|m| offered.iter().any(|o| o == *m))?;
    let t = Instant::now();
    match spike.proxy.get_content(mime).await {
        Ok(data) => {
            spike.log(format!(
                "  GetContent {mime}: {} bytes in {} ms",
                data.len(),
                t.elapsed().as_millis()
            ));
            Some(data)
        }
        Err(e) => {
            spike.log(format!("  GetContent {mime} failed: {e}"));
            None
        }
    }
}

async fn capture(spike: &Spike, app: &AppHandle, mimetypes: Vec<String>, source_app: String) {
    spike.log(format!(
        "ClipboardChanged from '{source_app}': {mimetypes:?}"
    ));
    if mimetypes.iter().any(|m| m == PASSWORD_HINT) {
        let hint = first_available(spike, &mimetypes, &[PASSWORD_HINT])
            .await
            .unwrap_or_default();
        if hint == b"secret" {
            spike.log("  skipped: password manager hint");
            return;
        }
    }
    let text = first_available(spike, &mimetypes, TEXT_MIMES)
        .await
        .map(|b| String::from_utf8_lossy(&b).into_owned());
    let clip = Clip {
        html: first_available(spike, &mimetypes, HTML_MIMES).await,
        rtf: first_available(spike, &mimetypes, RTF_MIMES).await,
        png: first_available(spike, &mimetypes, PNG_MIMES).await,
        preview: text.as_deref().unwrap_or("").chars().take(80).collect(),
        text,
        formats: vec![],
        source_app,
    };
    let mut clip = clip;
    for (present, name) in [
        (clip.text.is_some(), "text"),
        (clip.html.is_some(), "html"),
        (clip.rtf.is_some(), "rtf"),
        (clip.png.is_some(), "png"),
    ] {
        if present {
            clip.formats.push(name);
        }
    }
    if clip.formats.is_empty() {
        spike.log("  nothing stored");
        return;
    }
    {
        let mut inner = spike.inner.lock().unwrap();
        if let Some((at, text)) = &inner.last_write
            && at.elapsed() < Duration::from_secs(2)
            && clip.text.as_deref() == Some(text.as_str())
        {
            spike.log("  loop guard: our own write, not stored");
            return;
        }
        if clip.png.is_some() && clip.preview.is_empty() {
            clip.preview = "image".into();
        }
        inner.clips.insert(0, clip);
        inner.clips.truncate(MAX_CLIPS);
        let _ = app.emit("clips", &inner.clips);
    }
}

async fn show_picker(spike: &Spike, app: &AppHandle) {
    let window = app.get_webview_window("picker").expect("picker window");
    let clips = spike.inner.lock().unwrap().clips.clone();
    let _ = app.emit("clips", &clips);
    let _ = window.show();
    spike.log("picker shown");
    if let Err(e) = spike
        .proxy
        .present_picker(std::process::id(), PICKER_TITLE)
        .await
    {
        spike.log(format!("PresentPicker failed: {e}"));
    }
}

async fn paste_plain_shortcut(spike: &Spike) {
    let (text, app_id) = {
        let inner = spike.inner.lock().unwrap();
        (
            inner.clips.first().and_then(|c| c.text.clone()),
            inner.focused_app.clone(),
        )
    };
    let Some(text) = text else {
        spike.log("paste-plain: no text clip");
        return;
    };
    spike.inner.lock().unwrap().last_write = Some((Instant::now(), text.clone()));
    if let Err(e) = spike
        .proxy
        .set_content("text/plain;charset=utf-8", text.as_bytes())
        .await
    {
        spike.log(format!("SetContent failed: {e}"));
        return;
    }
    send_paste(spike, keystroke_for(&app_id), false).await;
}

async fn send_paste(spike: &Spike, keystroke: &str, activate_previous: bool) {
    let t = Instant::now();
    match spike.proxy.paste(keystroke, activate_previous).await {
        Ok(()) => spike.log(format!(
            "Paste {keystroke} sent after {} ms",
            t.elapsed().as_millis()
        )),
        Err(e) => spike.log(format!("Paste failed ({e}). Copied. Press Ctrl+V.")),
    }
}

/// GTK focused write: every format in one offer, made while the picker has focus.
fn gtk_write(clip: &Clip, plain_only: bool) {
    use gtk::{Clipboard, TargetEntry, TargetFlags, gdk};

    let mut payloads: Vec<(&str, Vec<u8>)> = Vec::new();
    if let Some(t) = &clip.text {
        for mime in TEXT_MIMES {
            payloads.push((mime, t.as_bytes().to_vec()));
        }
    }
    if !plain_only {
        if let Some(h) = &clip.html {
            payloads.push(("text/html", h.clone()));
        }
        if let Some(r) = &clip.rtf {
            payloads.push(("text/rtf", r.clone()));
            payloads.push(("application/rtf", r.clone()));
        }
        if let Some(p) = &clip.png {
            payloads.push(("image/png", p.clone()));
        }
    }
    let targets: Vec<TargetEntry> = payloads
        .iter()
        .enumerate()
        .map(|(i, (mime, _))| TargetEntry::new(mime, TargetFlags::empty(), i as u32))
        .collect();
    let data: Vec<Vec<u8>> = payloads.into_iter().map(|(_, d)| d).collect();
    let clipboard = Clipboard::get(&gdk::SELECTION_CLIPBOARD);
    let ok = clipboard.set_with_data(&targets, move |_, selection, info| {
        if let Some(bytes) = data.get(info as usize) {
            selection.set(&selection.target(), 8, bytes);
        }
    });
    println!("GTK set_with_data with {} targets: {ok}", targets.len());
}

#[tauri::command]
fn picker_focused(spike: State<'_, Arc<Spike>>) {
    if let Some(at) = spike.inner.lock().unwrap().hotkey_at.take() {
        spike.log(format!(
            "picker focused (webview) {} ms after the hotkey signal",
            at.elapsed().as_millis()
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
    let (clip, app_id) = {
        let inner = spike.inner.lock().unwrap();
        let clip = inner.clips.get(index).cloned().ok_or("no such clip")?;
        (clip, inner.focused_app.clone())
    };
    spike.log(format!("choose #{index} plain={plain}"));
    if let Some(t) = &clip.text {
        spike.inner.lock().unwrap().last_write = Some((Instant::now(), t.clone()));
    }
    let window = app.get_webview_window("picker").ok_or("no picker")?;
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.run_on_main_thread(move || {
        gtk_write(&clip, plain);
        let _ = window.hide();
        let _ = tx.send(());
    })
    .map_err(|e| e.to_string())?;
    rx.await.map_err(|e| e.to_string())?;
    spike.log("written and hidden");
    send_paste(&spike, keystroke_for(&app_id), true).await;

    // Check that the offer outlived the hide by reading it back through the shell.
    tokio::time::sleep(Duration::from_millis(300)).await;
    match spike.proxy.get_mime_types().await {
        Ok(m) => spike.log(format!("offer after hide: {m:?}")),
        Err(e) => spike.log(format!("GetMimeTypes failed: {e}")),
    }
    Ok(())
}

#[tauri::command]
fn verify_offer(app: AppHandle, spike: State<'_, Arc<Spike>>) {
    let spike = spike.inner().clone();
    tauri::async_runtime::spawn(async move {
        let mimes = spike.proxy.get_mime_types().await.unwrap_or_default();
        spike.log(format!("current offer: {mimes:?}"));
        let clip = spike.inner.lock().unwrap().clips.first().cloned();
        let Some(clip) = clip else { return };
        for (mime, expected) in [
            (
                "text/plain;charset=utf-8",
                clip.text.as_ref().map(|t| t.as_bytes().to_vec()),
            ),
            ("text/html", clip.html.clone()),
            ("text/rtf", clip.rtf.clone()),
            ("image/png", clip.png.clone()),
        ] {
            let Some(expected) = expected else { continue };
            let got = spike.proxy.get_content(mime).await.unwrap_or_default();
            spike.log(format!(
                "  {mime}: {}",
                if got == expected {
                    "matches"
                } else {
                    "DIFFERS"
                }
            ));
        }
        let _ = app.emit("verified", ());
    });
}

async fn run_dbus(app: AppHandle, spike: Arc<Spike>, conn: Connection) -> zbus::Result<()> {
    register(&spike).await?;
    let mut changed = spike.proxy.receive_clipboard_changed().await?;
    let mut shortcuts = spike.proxy.receive_shortcut_activated().await?;
    let shell = ShellExtensionsProxy::new(&conn).await?;
    let mut states = shell.receive_extension_state_changed().await?;
    loop {
        tokio::select! {
            Some(sig) = changed.next() => {
                let args = sig.args()?;
                capture(&spike, &app, args.mimetypes, args.source_app_id).await;
            }
            Some(sig) = shortcuts.next() => {
                let args = sig.args()?;
                spike.log(format!("ShortcutActivated {} (focused '{}')", args.action, args.focused_app_id));
                {
                    let mut inner = spike.inner.lock().unwrap();
                    inner.focused_app = args.focused_app_id.clone();
                    inner.hotkey_at = Some(Instant::now());
                }
                match args.action.as_str() {
                    "picker" => show_picker(&spike, &app).await,
                    "paste-plain" => paste_plain_shortcut(&spike).await,
                    other => spike.log(format!("unknown action {other}")),
                }
            }
            Some(sig) = states.next() => {
                let args = sig.args()?;
                if args.uuid != EXTENSION_UUID {
                    continue;
                }
                let state = args.state.get("state").and_then(|v| f64::try_from(v).ok());
                spike.log(format!("ExtensionStateChanged: state {state:?}"));
                if state == Some(1.0) {
                    match register(&spike).await {
                        Ok(()) => spike.log("re-registered after the extension came back"),
                        Err(e) => spike.log(format!("re-register failed: {e}")),
                    }
                }
            }
            else => break,
        }
    }
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            picker_focused,
            hide_picker,
            choose,
            verify_offer
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            let (conn, proxy) = tauri::async_runtime::block_on(async {
                let conn = Connection::session().await?;
                let proxy = FerryProxy::new(&conn).await?;
                Ok::<_, zbus::Error>((conn, proxy))
            })?;
            let spike = Arc::new(Spike {
                proxy,
                inner: Mutex::default(),
                started: Instant::now(),
            });
            println!(
                "exe: {} (must be listed in the extension's allowed-clients)",
                std::env::current_exe()?.display()
            );
            app.manage(spike.clone());
            tauri::async_runtime::spawn(async move {
                if let Err(e) = run_dbus(handle, spike.clone(), conn).await {
                    spike.log(format!("D-Bus loop ended: {e}"));
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running the spike");
}
