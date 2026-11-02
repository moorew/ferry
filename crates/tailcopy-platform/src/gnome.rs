//! GNOME on Wayland (SPEC 9.4): the TailCopy Shell extension over D-Bus for capture,
//! shortcuts, presenting the picker and pasting; GTK's clipboard for the focused
//! multi-format write.
//!
//! Every D-Bus call here is async and must run off the GTK main thread: while TailCopy
//! owns the clipboard, the shell's `GetContent` asks our own main loop for the data
//! (D-007).

use std::collections::HashMap;
use std::path::Path;

use futures_util::{Stream, StreamExt, stream};
use zbus::{Connection, OwnedVariant, proxy};

use tailcopy_core::names::{GNOME_EXTENSION_UUID, PICKER_TITLE};

use crate::{
    Captured, CopyOnlyReason, Keystroke, Marker, PasteResult, Result, Write, WriteOutcome,
};

/// API version this client speaks, and the extension versions it accepts (SPEC 9.4.7).
pub const CLIENT_API: u32 = 1;
const SUPPORTED_EXTENSION_API: std::ops::RangeInclusive<u32> = 1..=1;

const TEXT_MIMES: &[&str] = &[
    "text/plain;charset=utf-8",
    "UTF8_STRING",
    "text/plain",
    "STRING",
];
const RTF_MIMES: &[&str] = &["text/rtf", "application/rtf"];
const PASSWORD_HINT: &str = "x-kde-passwordManagerHint";
const SYSTEM_EXTENSION_DIR: &str = "/usr/share/gnome-shell/extensions";

#[proxy(
    interface = "org.gnome.Shell.Extensions.TailCopy",
    default_service = "org.gnome.Shell",
    default_path = "/org/gnome/Shell/Extensions/TailCopy"
)]
trait Extension {
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
    fn get_extension_info(&self, uuid: &str) -> zbus::Result<HashMap<String, OwnedVariant>>;
    fn enable_extension(&self, uuid: &str) -> zbus::Result<bool>;

    #[zbus(signal)]
    fn extension_state_changed(
        &self,
        uuid: String,
        state: HashMap<String, OwnedVariant>,
    ) -> zbus::Result<()>;
}

#[proxy(
    interface = "org.gnome.SessionManager",
    default_service = "org.gnome.SessionManager",
    default_path = "/org/gnome/SessionManager"
)]
trait SessionManager {
    fn logout(&self, mode: u32) -> zbus::Result<()>;
}

/// Where the extension stands, for first run (SPEC 9.4.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtensionStatus {
    /// Running and ready for `Register`.
    Active,
    /// Installed and known to the shell, but turned off.
    Disabled,
    /// Installed on disk, but the shell started before it was: log out and back in.
    NeedsRelogin,
    /// Not installed.
    Missing,
    /// The shell reports an error or an unsupported GNOME version.
    Broken(String),
}

/// Something the extension or the shell told us.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    ClipboardChanged {
        mimetypes: Vec<String>,
        source_app: String,
    },
    Shortcut {
        action: String,
        focused_app: String,
    },
    /// The extension came back (after unlocking, for example): register again.
    ExtensionActive,
    /// The extension went away (the screen locked, for example).
    ExtensionInactive,
}

pub struct Gnome {
    conn: Connection,
    ext: ExtensionProxy<'static>,
    shell: ShellExtensionsProxy<'static>,
}

impl Gnome {
    pub async fn connect() -> Result<Self> {
        let conn = Connection::session().await?;
        let ext = ExtensionProxy::new(&conn).await?;
        let shell = ShellExtensionsProxy::new(&conn).await?;
        Ok(Gnome { conn, ext, shell })
    }

    pub async fn status(&self) -> ExtensionStatus {
        let info = match self.shell.get_extension_info(GNOME_EXTENSION_UUID).await {
            Ok(info) => info,
            Err(e) => return ExtensionStatus::Broken(e.to_string()),
        };
        let state = info
            .get("state")
            .and_then(|v| f64::try_from(v).ok())
            .map(|s| s as i64);
        match state {
            Some(1) => ExtensionStatus::Active,
            Some(2 | 6) => ExtensionStatus::Disabled,
            Some(4) => ExtensionStatus::Broken("this version of GNOME isn't supported yet".into()),
            Some(3) => ExtensionStatus::Broken(
                info.get("error")
                    .and_then(|v| String::try_from(v.clone()).ok())
                    .unwrap_or_else(|| "the extension failed to start".into()),
            ),
            Some(_) => ExtensionStatus::Disabled,
            None if installed_on_disk() => ExtensionStatus::NeedsRelogin,
            None => ExtensionStatus::Missing,
        }
    }

    pub async fn enable(&self) -> Result<bool> {
        Ok(self.shell.enable_extension(GNOME_EXTENSION_UUID).await?)
    }

    /// Registers this process as the extension's client and returns its API version.
    /// May fail once just after an `ExtensionActive` event (D-011); retry on the next.
    pub async fn register(&self) -> Result<u32> {
        let api = self.ext.register(CLIENT_API).await?;
        if !SUPPORTED_EXTENSION_API.contains(&api) {
            tracing::warn!("extension API {api} is not supported; log out and back in");
        }
        Ok(api)
    }

    pub fn api_supported(api: u32) -> bool {
        SUPPORTED_EXTENSION_API.contains(&api)
    }

    /// `shortcut` is in TailCopy's config form (`Ctrl+Alt+V`); empty unbinds it.
    pub async fn set_shortcut(&self, action: &str, shortcut: &str) -> Result<()> {
        let accel = accelerator(shortcut);
        let accels: Vec<&str> = accel.iter().map(String::as_str).collect();
        Ok(self.ext.set_shortcut(action, &accels).await?)
    }

    /// One stream of everything the extension and the shell report.
    pub async fn events(&self) -> Result<impl Stream<Item = Event> + use<>> {
        let changed = self
            .ext
            .receive_clipboard_changed()
            .await?
            .filter_map(|s| async move {
                let a = s.args().ok()?;
                Some(Event::ClipboardChanged {
                    mimetypes: a.mimetypes,
                    source_app: a.source_app_id,
                })
            });
        let shortcuts = self
            .ext
            .receive_shortcut_activated()
            .await?
            .filter_map(|s| async move {
                let a = s.args().ok()?;
                Some(Event::Shortcut {
                    action: a.action,
                    focused_app: a.focused_app_id,
                })
            });
        let states = self
            .shell
            .receive_extension_state_changed()
            .await?
            .filter_map(|s| async move {
                let a = s.args().ok()?;
                if a.uuid != GNOME_EXTENSION_UUID {
                    return None;
                }
                let state = a.state.get("state").and_then(|v| f64::try_from(v).ok())?;
                Some(if state == 1.0 {
                    Event::ExtensionActive
                } else {
                    Event::ExtensionInactive
                })
            });
        Ok(stream::select(
            stream::select(changed.boxed(), shortcuts.boxed()),
            states.boxed(),
        ))
    }

    /// Reads the formats TailCopy stores from what the clipboard offers (SPEC 9.4.3).
    pub async fn read(&self, mimetypes: &[String], source_app: &str) -> Result<Captured> {
        let offered = |m: &&str| mimetypes.iter().any(|o| o == m);
        let mut captured = Captured {
            raw_types: mimetypes.to_vec(),
            source_app: (!source_app.is_empty()).then(|| source_app.to_string()),
            ..Captured::default()
        };
        if offered(&PASSWORD_HINT) {
            let hint = self.ext.get_content(PASSWORD_HINT).await;
            tracing::debug!(?hint, "password manager hint");
            let hint = hint.unwrap_or_default();
            // Fail closed: an app that offers the hint but can't serve it is treated
            // as marking the copy secret (D-020).
            if hint.is_empty() || hint.trim_ascii() == b"secret" {
                captured.marker = Marker::DontSave(PASSWORD_HINT.into());
                return Ok(captured);
            }
        }
        let get = |mime: &'static str| async move {
            match self.ext.get_content(mime).await {
                Ok(data) if !data.is_empty() => Some(data),
                Ok(_) => None,
                Err(e) => {
                    tracing::debug!("GetContent {mime} failed: {e}");
                    None
                }
            }
        };
        if let Some(mime) = TEXT_MIMES.iter().find(|m| offered(m)) {
            captured.text = get(mime)
                .await
                .map(|b| String::from_utf8_lossy(&b).into_owned());
        }
        if offered(&"text/html") {
            captured.html = get("text/html").await.map(|b| decode_html(&b));
        }
        if let Some(mime) = RTF_MIMES.iter().find(|m| offered(m)) {
            captured.rtf = get(mime)
                .await
                .map(|b| String::from_utf8_lossy(&b).into_owned());
        }
        if offered(&"image/png") {
            captured.png = get("image/png").await;
        }
        Ok(captured)
    }

    /// What the clipboard offers right now.
    pub async fn mimetypes(&self) -> Result<Vec<String>> {
        Ok(self.ext.get_mime_types().await?)
    }

    /// Background write of one format (SPEC 9.4.3): plain text for text and rich clips.
    pub async fn set_text(&self, text: &str) -> Result<WriteOutcome> {
        self.ext
            .set_content("text/plain;charset=utf-8", text.as_bytes())
            .await?;
        Ok(WriteOutcome {
            landed: vec!["text/plain"],
        })
    }

    /// Background write of an image.
    pub async fn set_png(&self, png: &[u8]) -> Result<WriteOutcome> {
        self.ext.set_content("image/png", png).await?;
        Ok(WriteOutcome {
            landed: vec!["image/png"],
        })
    }

    pub async fn present_picker(&self) -> Result<()> {
        Ok(self
            .ext
            .present_picker(std::process::id(), PICKER_TITLE)
            .await?)
    }

    pub async fn paste(&self, keystroke: Keystroke, activate_previous: bool) -> PasteResult {
        match self
            .ext
            .paste(keystroke.gnome_name(), activate_previous)
            .await
        {
            Ok(()) => PasteResult::Pasted,
            Err(zbus::Error::MethodError(name, _, _))
                if name.as_str().ends_with("ModifiersHeld") =>
            {
                PasteResult::CopiedOnly(CopyOnlyReason::ModifiersHeld)
            }
            Err(e) => {
                tracing::warn!("Paste failed: {e}");
                PasteResult::CopiedOnly(CopyOnlyReason::Failed)
            }
        }
    }

    /// Asks GNOME to log out (with its own confirmation), for first run.
    pub async fn log_out(&self) -> Result<()> {
        let session = SessionManagerProxy::new(&self.conn).await?;
        Ok(session.logout(0).await?)
    }
}

fn installed_on_disk() -> bool {
    Path::new(SYSTEM_EXTENSION_DIR)
        .join(GNOME_EXTENSION_UUID)
        .join("metadata.json")
        .exists()
        || std::env::var_os("HOME").is_some_and(|home| {
            Path::new(&home)
                .join(".local/share/gnome-shell/extensions")
                .join(GNOME_EXTENSION_UUID)
                .join("metadata.json")
                .exists()
        })
}

/// Firefox and Chromium offer `text/html` as UTF-16 with a BOM.
fn decode_html(bytes: &[u8]) -> String {
    match bytes {
        [0xFF, 0xFE, rest @ ..] => {
            let units: Vec<u16> = rest
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| u16::from_le_bytes(*c))
                .collect();
            String::from_utf16_lossy(&units)
        }
        [0xFE, 0xFF, rest @ ..] => {
            let units: Vec<u16> = rest
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| u16::from_be_bytes(*c))
                .collect();
            String::from_utf16_lossy(&units)
        }
        _ => String::from_utf8_lossy(bytes).into_owned(),
    }
}

/// `Ctrl+Alt+V` to GTK's `<Control><Alt>v`; empty input means unbound.
pub fn accelerator(shortcut: &str) -> Vec<String> {
    let parts: Vec<&str> = shortcut
        .split('+')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    let Some((key, mods)) = parts.split_last() else {
        return Vec::new();
    };
    let mut out = String::new();
    for m in mods {
        out.push_str(match m.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => "<Control>",
            "alt" | "option" => "<Alt>",
            "shift" => "<Shift>",
            "super" | "meta" | "win" | "cmd" => "<Super>",
            _ => continue,
        });
    }
    let key = if key.chars().count() == 1 {
        key.to_lowercase()
    } else {
        (*key).to_string()
    };
    out.push_str(&key);
    vec![out]
}

/// GTK focused write (SPEC 9.4.3): every format in one offer. Call on the GTK main
/// thread while the picker has focus; TailCopy keeps serving the offer after the
/// picker hides (verified in M0).
pub fn gtk_write(clip: &Write<'_>) -> WriteOutcome {
    use gtk::{Clipboard, TargetEntry, TargetFlags, gdk};

    let mut payloads: Vec<(&str, Vec<u8>)> = Vec::new();
    let mut landed = Vec::new();
    if let Some(t) = clip.text {
        for mime in TEXT_MIMES {
            payloads.push((mime, t.as_bytes().to_vec()));
        }
        landed.push("text/plain");
    }
    if let Some(h) = clip.html {
        payloads.push(("text/html", h.as_bytes().to_vec()));
        landed.push("text/html");
    }
    if let Some(r) = clip.rtf {
        for mime in RTF_MIMES {
            payloads.push((mime, r.as_bytes().to_vec()));
        }
        landed.push("text/rtf");
    }
    if let Some(p) = clip.png {
        payloads.push(("image/png", p.to_vec()));
        landed.push("image/png");
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
    if !ok {
        tracing::warn!("GTK refused the clipboard offer");
        landed.clear();
    }
    WriteOutcome { landed }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accelerators() {
        assert_eq!(accelerator("Ctrl+Alt+V"), vec!["<Control><Alt>v"]);
        assert_eq!(
            accelerator("Ctrl+Alt+Shift+V"),
            vec!["<Control><Alt><Shift>v"]
        );
        assert_eq!(accelerator("Super+Insert"), vec!["<Super>Insert"]);
        assert!(accelerator("").is_empty());
    }

    #[test]
    fn utf16_html() {
        let mut bytes = vec![0xFF, 0xFE];
        for u in "<b>hi</b>".encode_utf16() {
            bytes.extend(u.to_le_bytes());
        }
        assert_eq!(decode_html(&bytes), "<b>hi</b>");
        assert_eq!(decode_html(b"<i>x</i>"), "<i>x</i>");
    }
}
