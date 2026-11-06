//! Clipboard backends, paste injection, focus tracking and source app lookup (SPEC 9).
//!
//! Each platform module exposes the same small surface: read what is on the clipboard,
//! write a clip (every format in one offer where the platform allows), remember the
//! window that was focused when the picker opened, and paste into it after the
//! modifier wait. Hotkeys live in the app: Windows, macOS and X11 use Tauri's global
//! shortcut plugin, GNOME uses the shell extension's `ShortcutActivated` signal.

use std::time::{Duration, Instant};

#[cfg(any(windows, test))]
mod bmp;
#[cfg(target_os = "linux")]
pub mod gnome;
#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(windows)]
pub mod windows;
#[cfg(target_os = "linux")]
pub mod x11;

/// Every format a backend could read for one clipboard change.
#[derive(Debug, Default, Clone)]
pub struct Captured {
    pub text: Option<String>,
    pub html: Option<String>,
    pub rtf: Option<String>,
    pub png: Option<Vec<u8>>,
    /// Every type the source offered, for privacy checks and diagnostics.
    pub raw_types: Vec<String>,
    /// Process name, bundle id or desktop app id.
    pub source_app: Option<String>,
    pub marker: Marker,
}

impl Captured {
    pub fn is_empty(&self) -> bool {
        self.text.as_deref().is_none_or(|t| t.trim().is_empty())
            && self.html.is_none()
            && self.rtf.is_none()
            && self.png.is_none()
    }

    /// MIME types present, for logs (never contents).
    pub fn formats(&self) -> Vec<&'static str> {
        let mut out = Vec::new();
        if self.text.is_some() {
            out.push("text/plain");
        }
        if self.html.is_some() {
            out.push("text/html");
        }
        if self.rtf.is_some() {
            out.push("text/rtf");
        }
        if self.png.is_some() {
            out.push("image/png");
        }
        out
    }
}

/// OS-level secret markers (SPEC 9.2, 9.3, 9.4.3).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub enum Marker {
    #[default]
    None,
    /// Never store this clip.
    DontSave(String),
    /// Store locally but never sync.
    DontSync(String),
}

/// What to put on the clipboard.
#[derive(Debug, Default, Clone)]
pub struct Write<'a> {
    pub text: Option<&'a str>,
    pub html: Option<&'a str>,
    pub rtf: Option<&'a str>,
    pub png: Option<&'a [u8]>,
}

impl Write<'_> {
    /// The same clip with only its plain text.
    pub fn plain(&self) -> Self {
        Write {
            text: self.text,
            ..Write::default()
        }
    }
}

/// Which formats actually landed, so the UI can say "pasted as plain text".
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct WriteOutcome {
    pub landed: Vec<&'static str>,
}

/// The paste keystroke to send (SPEC 9.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keystroke {
    CtrlV,
    CtrlShiftV,
    ShiftInsert,
    CmdV,
}

impl Keystroke {
    /// The extension's name for this keystroke (SPEC 9.4.2).
    pub fn gnome_name(self) -> &'static str {
        match self {
            Keystroke::CtrlV | Keystroke::CmdV => "ctrl-v",
            Keystroke::CtrlShiftV => "ctrl-shift-v",
            Keystroke::ShiftInsert => "shift-insert",
        }
    }

    pub fn from_config(name: &str) -> Option<Self> {
        match name {
            "ctrl-v" => Some(Keystroke::CtrlV),
            "ctrl-shift-v" => Some(Keystroke::CtrlShiftV),
            "shift-insert" => Some(Keystroke::ShiftInsert),
            _ => None,
        }
    }
}

/// Result of a paste attempt. When pasting isn't possible the clip stays on the
/// clipboard and the UI tells the user to paste it themselves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PasteResult {
    Pasted,
    CopiedOnly(CopyOnlyReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CopyOnlyReason {
    /// Modifier keys were still held after the wait (SPEC 9.1).
    ModifiersHeld,
    /// The target window runs elevated (Windows).
    ElevatedTarget,
    /// macOS Accessibility permission is missing.
    NoAccessibility,
    /// The platform can't inject keystrokes here.
    Unsupported,
    /// The keystroke could not be sent.
    Failed,
}

impl CopyOnlyReason {
    /// The message the picker shows (SPEC 9.1, 9.2, 9.3).
    pub fn message(self, keystroke: &str) -> String {
        match self {
            CopyOnlyReason::ElevatedTarget => {
                format!("Copied. Press {keystroke} to paste (admin window).")
            }
            CopyOnlyReason::NoAccessibility => format!(
                "Copied. Press {keystroke} to paste. Allow TailCopy in Accessibility settings to paste for you."
            ),
            _ => format!("Copied. Press {keystroke}."),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("clipboard: {0}")]
    Clipboard(String),
    #[error("input: {0}")]
    Input(String),
    #[cfg(target_os = "linux")]
    #[error("GNOME Shell: {0}")]
    Gnome(#[from] zbus::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

/// The paste keystroke for an app id, from the configured overrides (SPEC 9.8).
pub fn keystroke_for(
    app_id: &str,
    overrides: &[(String, String)],
    default: Keystroke,
) -> Keystroke {
    overrides
        .iter()
        .find(|(id, _)| id == app_id)
        .and_then(|(_, k)| Keystroke::from_config(k))
        .unwrap_or(default)
}

/// Polls `held` until it reports no modifiers or `timeout` passes (SPEC 9.1).
pub fn wait_until_released(timeout: Duration, mut held: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if !held() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Which backend fits this session (SPEC 9.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Session {
    Windows,
    MacOs,
    Gnome,
    /// An X11 session (SPEC 9.5, tier 2).
    X11,
    /// No supported clipboard backend: sync and the manager still work.
    Degraded,
}

pub fn detect_session() -> Session {
    if cfg!(windows) {
        Session::Windows
    } else if cfg!(target_os = "macos") {
        Session::MacOs
    } else {
        let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
        let gnome = std::env::var("XDG_CURRENT_DESKTOP")
            .is_ok_and(|d| d.split(':').any(|part| part.eq_ignore_ascii_case("GNOME")));
        let x11 = std::env::var_os("DISPLAY").is_some();
        match (wayland, gnome, x11) {
            (true, true, _) => Session::Gnome,
            // On other Wayland desktops DISPLAY is XWayland, which only sees X apps.
            (false, _, true) => Session::X11,
            _ => Session::Degraded,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_wins_for_terminals() {
        let overrides = vec![("org.gnome.Ptyxis".to_string(), "ctrl-shift-v".to_string())];
        assert_eq!(
            keystroke_for("org.gnome.Ptyxis", &overrides, Keystroke::CtrlV),
            Keystroke::CtrlShiftV
        );
        assert_eq!(
            keystroke_for("org.gnome.TextEditor", &overrides, Keystroke::CtrlV),
            Keystroke::CtrlV
        );
    }

    #[test]
    fn wait_gives_up_after_timeout() {
        let t = Instant::now();
        assert!(!wait_until_released(Duration::from_millis(50), || true));
        assert!(t.elapsed() >= Duration::from_millis(50));
        assert!(wait_until_released(Duration::from_millis(50), || false));
    }

    #[test]
    fn empty_capture() {
        let c = Captured {
            text: Some("  \n".into()),
            ..Captured::default()
        };
        assert!(c.is_empty());
    }
}
