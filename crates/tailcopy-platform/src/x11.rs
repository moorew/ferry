//! X11 sessions (SPEC 9.5, tier 2): clipboard-rs to watch and read, the shared GTK
//! write, EWMH window properties for focus and the source app, and XTest for the paste
//! keystroke. Shortcuts come from Tauri's global shortcut plugin.

use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

use clipboard_rs::{
    Clipboard, ClipboardContext, ClipboardHandler, ClipboardWatcher, ClipboardWatcherContext,
};
use x11rb::{
    COPY_DEPTH_FROM_PARENT, CURRENT_TIME,
    connection::Connection,
    protocol::{
        Event,
        xproto::{
            AtomEnum, ChangeWindowAttributesAux, ClientMessageEvent, ConnectionExt as _,
            CreateWindowAux, EventMask, GetPropertyReply, KEY_PRESS_EVENT, KEY_RELEASE_EVENT,
            KeyButMask, PropMode, Window, WindowClass,
        },
        xtest::ConnectionExt as _,
    },
    rust_connection::RustConnection,
    wrapper::ConnectionExt as _,
};

use crate::linux::{PASSWORD_HINT, RTF_MIMES, TEXT_MIMES, decode_html};
use crate::{
    Captured, CopyOnlyReason, Error, Keystroke, Marker, PasteResult, Result, wait_until_released,
};

const XK_CONTROL_L: u32 = 0xffe3;
const XK_SHIFT_L: u32 = 0xffe1;
const XK_INSERT: u32 = 0xff63;
const XK_V: u32 = 0x0076;

const FOCUS_SETTLE: Duration = Duration::from_millis(30);
const MODIFIER_WAIT: Duration = Duration::from_millis(500);
/// How long the picker window may take to appear in the window manager's list.
const PRESENT_TIMEOUT: Duration = Duration::from_millis(500);

struct Notify(mpsc::Sender<()>);

impl ClipboardHandler for Notify {
    fn on_clipboard_change(&mut self) {
        let _ = self.0.send(());
    }
}

/// Watches the CLIPBOARD selection (XFixes) on its own thread; one message per change.
pub fn watch() -> Result<mpsc::Receiver<()>> {
    let (tx, rx) = mpsc::channel();
    let mut watcher =
        ClipboardWatcherContext::new().map_err(|e| Error::Clipboard(e.to_string()))?;
    std::thread::Builder::new()
        .name("clipboard-watch".into())
        .spawn(move || {
            watcher.add_handler(Notify(tx)).start_watch();
        })
        .map_err(|e| Error::Clipboard(e.to_string()))?;
    Ok(rx)
}

/// Reads the formats TailCopy stores from the CLIPBOARD selection. Runs off the GTK
/// main thread: when TailCopy owns the selection, GTK serves it from there.
pub fn read() -> Result<Captured> {
    let ctx = ClipboardContext::new().map_err(|e| Error::Clipboard(e.to_string()))?;
    let raw_types = ctx.available_formats().unwrap_or_default();
    let offered = |mime: &str| raw_types.iter().any(|t| t == mime);
    let get = |mime: &str| ctx.get_buffer(mime).ok().filter(|b| !b.is_empty());
    let source_app = X::connect().ok().and_then(|x| x.clipboard_source_app());
    if offered(PASSWORD_HINT) {
        let hint = ctx.get_buffer(PASSWORD_HINT).unwrap_or_default();
        // Fail closed, as on GNOME (D-020).
        if hint.is_empty() || hint.trim_ascii() == b"secret" {
            return Ok(Captured {
                raw_types,
                source_app,
                marker: Marker::DontSave(PASSWORD_HINT.into()),
                ..Captured::default()
            });
        }
    }
    let text = TEXT_MIMES
        .iter()
        .find(|m| offered(m))
        .and_then(|m| get(m))
        .map(|b| String::from_utf8_lossy(&b).into_owned());
    let html = offered("text/html")
        .then(|| get("text/html"))
        .flatten()
        .map(|b| decode_html(&b));
    let rtf = RTF_MIMES
        .iter()
        .find(|m| offered(m))
        .and_then(|m| get(m))
        .map(|b| String::from_utf8_lossy(&b).into_owned());
    let png = offered("image/png").then(|| get("image/png")).flatten();
    Ok(Captured {
        text,
        html,
        rtf,
        png,
        raw_types,
        source_app,
        marker: Marker::None,
    })
}

/// The window that had focus when the picker opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    window: Window,
    /// GTK application id, else the `WM_CLASS` class, else the process name.
    pub app_id: String,
}

/// The active window, unless it belongs to TailCopy itself.
pub fn foreground() -> Option<Target> {
    let x = X::connect().ok()?;
    let window = x.active_window()?;
    if x.pid(window) == Some(std::process::id()) {
        return None;
    }
    Some(Target {
        window,
        app_id: x.app_id(window).unwrap_or_default(),
    })
}

/// Asks the window manager to focus TailCopy's window titled `title`, once it is
/// mapped. Showing a window from a global shortcut doesn't always focus it, because
/// the window manager's focus-stealing prevention has no user timestamp to go on.
pub fn present(title: &str) {
    let Ok(x) = X::connect() else { return };
    let deadline = Instant::now() + PRESENT_TIMEOUT;
    loop {
        if let Some(window) = x.own_window(title) {
            x.activate(window);
            return;
        }
        if Instant::now() >= deadline {
            tracing::debug!("the picker window didn't appear in time to focus it");
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Restores focus to `target` (when given), waits for modifiers, then sends the paste
/// keystroke.
pub fn paste(target: Option<&Target>, keystroke: Keystroke) -> PasteResult {
    let x = match X::connect() {
        Ok(x) => x,
        Err(e) => {
            tracing::warn!("couldn't reach the X server: {e}");
            return PasteResult::CopiedOnly(CopyOnlyReason::Failed);
        }
    };
    if let Some(target) = target {
        x.activate(target.window);
        std::thread::sleep(FOCUS_SETTLE);
    }
    if !wait_until_released(MODIFIER_WAIT, || x.modifiers_held()) {
        return PasteResult::CopiedOnly(CopyOnlyReason::ModifiersHeld);
    }
    match x.send(keystroke) {
        Some(()) => PasteResult::Pasted,
        None => {
            tracing::warn!("the paste keystroke couldn't be sent");
            PasteResult::CopiedOnly(CopyOnlyReason::Failed)
        }
    }
}

/// One X connection for a few property lookups.
struct X {
    conn: RustConnection,
    root: Window,
}

impl X {
    fn connect() -> Result<Self> {
        let (conn, screen) = x11rb::connect(None).map_err(|e| Error::Input(e.to_string()))?;
        let root = conn.setup().roots[screen].root;
        Ok(X { conn, root })
    }

    fn atom(&self, name: &str) -> Option<u32> {
        Some(
            self.conn
                .intern_atom(false, name.as_bytes())
                .ok()?
                .reply()
                .ok()?
                .atom,
        )
    }

    fn property(&self, window: Window, name: &str, ty: impl Into<u32>) -> Option<GetPropertyReply> {
        let atom = self.atom(name)?;
        let reply = self
            .conn
            .get_property(false, window, atom, ty, 0, 4096)
            .ok()?
            .reply()
            .ok()?;
        (!reply.value.is_empty()).then_some(reply)
    }

    fn cardinals(&self, window: Window, name: &str, ty: impl Into<u32>) -> Vec<u32> {
        self.property(window, name, ty)
            .and_then(|r| r.value32().map(Iterator::collect))
            .unwrap_or_default()
    }

    fn text(&self, window: Window, name: &str) -> Option<String> {
        self.property(window, name, AtomEnum::ANY)
            .map(|r| String::from_utf8_lossy(&r.value).into_owned())
    }

    fn active_window(&self) -> Option<Window> {
        self.cardinals(self.root, "_NET_ACTIVE_WINDOW", AtomEnum::WINDOW)
            .first()
            .copied()
            .filter(|&w| w != 0)
    }

    fn pid(&self, window: Window) -> Option<u32> {
        self.cardinals(window, "_NET_WM_PID", AtomEnum::CARDINAL)
            .first()
            .copied()
    }

    /// GTK apps set their application id, which matches their desktop file id the way
    /// GNOME's app ids do; other apps fall back to `WM_CLASS`, then the process name.
    fn app_id(&self, window: Window) -> Option<String> {
        self.text(window, "_GTK_APPLICATION_ID")
            .or_else(|| {
                let class = self.text(window, "WM_CLASS")?;
                class
                    .split('\0')
                    .rfind(|s| !s.is_empty())
                    .map(str::to_owned)
            })
            .or_else(|| {
                let pid = self.pid(window)?;
                let comm = std::fs::read_to_string(format!("/proc/{pid}/comm")).ok()?;
                Some(comm.trim().to_owned())
            })
            .filter(|s| !s.is_empty())
    }

    /// The selection owner is often a hidden helper window without these properties;
    /// then the copy most likely came from the active window, as on Windows.
    fn clipboard_source_app(&self) -> Option<String> {
        let clipboard = self.atom("CLIPBOARD")?;
        let owner = self
            .conn
            .get_selection_owner(clipboard)
            .ok()?
            .reply()
            .ok()?
            .owner;
        (owner != 0)
            .then(|| self.app_id(owner))
            .flatten()
            .or_else(|| self.app_id(self.active_window()?))
    }

    fn own_window(&self, title: &str) -> Option<Window> {
        let own = std::process::id();
        self.cardinals(self.root, "_NET_CLIENT_LIST", AtomEnum::WINDOW)
            .into_iter()
            .find(|&w| {
                self.pid(w) == Some(own)
                    && (self.text(w, "_NET_WM_NAME").as_deref() == Some(title)
                        || self.text(w, "WM_NAME").as_deref() == Some(title))
            })
    }

    /// EWMH activation request, as a pager would send it (source indication 2). It
    /// carries the server's current time: with none, window managers refuse it once
    /// there has been keyboard input since.
    fn activate(&self, window: Window) {
        let Some(atom) = self.atom("_NET_ACTIVE_WINDOW") else {
            return;
        };
        let time = self.server_time().unwrap_or(CURRENT_TIME);
        let event = ClientMessageEvent::new(32, window, atom, [2, time, 0, 0, 0]);
        let sent = self.conn.send_event(
            false,
            self.root,
            EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
            event,
        );
        if sent.is_err() || !self.round_trip() {
            tracing::warn!("could not ask the window manager to switch windows");
        }
    }

    /// Waits until the server has handled everything sent so far. Without it, closing
    /// the connection straight after a request can drop the request.
    fn round_trip(&self) -> bool {
        self.conn
            .get_input_focus()
            .ok()
            .and_then(|c| c.reply().ok())
            .is_some()
    }

    /// The X server's clock: append nothing to a property of a scratch window and read
    /// the time off the change notification (ICCCM 2.1).
    fn server_time(&self) -> Option<u32> {
        let window = self.conn.generate_id().ok()?;
        self.conn
            .create_window(
                COPY_DEPTH_FROM_PARENT,
                window,
                self.root,
                0,
                0,
                1,
                1,
                0,
                WindowClass::INPUT_ONLY,
                0,
                &CreateWindowAux::new(),
            )
            .ok()?;
        let time = (|| {
            self.conn
                .change_window_attributes(
                    window,
                    &ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE),
                )
                .ok()?;
            let atom = self.atom("WM_NAME")?;
            self.conn
                .change_property8(PropMode::APPEND, window, atom, AtomEnum::STRING, &[])
                .ok()?;
            self.conn.flush().ok()?;
            loop {
                match self.conn.wait_for_event().ok()? {
                    Event::PropertyNotify(e) if e.window == window => return Some(e.time),
                    _ => {}
                }
            }
        })();
        let _ = self.conn.destroy_window(window);
        time
    }

    /// Presses the keystroke with XTest, using keycodes already in the keymap. Mapping
    /// a spare keycode instead (as enigo does) makes every client reload its keymap,
    /// and some apps match shortcuts by keycode (D-030).
    fn send(&self, keystroke: Keystroke) -> Option<()> {
        let (mods, key): (&[u32], u32) = match keystroke {
            Keystroke::CtrlV | Keystroke::CmdV => (&[XK_CONTROL_L], XK_V),
            Keystroke::CtrlShiftV => (&[XK_CONTROL_L, XK_SHIFT_L], XK_V),
            Keystroke::ShiftInsert => (&[XK_SHIFT_L], XK_INSERT),
        };
        let mods: Vec<u8> = mods
            .iter()
            .map(|&m| self.keycode(m))
            .collect::<Option<_>>()?;
        let key = self.keycode(key)?;
        let fake = |kind: u8, code: u8| {
            self.conn
                .xtest_fake_input(kind, code, CURRENT_TIME, self.root, 0, 0, 0)
                .ok()
        };
        for &m in &mods {
            fake(KEY_PRESS_EVENT, m)?;
        }
        fake(KEY_PRESS_EVENT, key)?;
        fake(KEY_RELEASE_EVENT, key)?;
        for &m in mods.iter().rev() {
            fake(KEY_RELEASE_EVENT, m)?;
        }
        self.round_trip().then_some(())
    }

    /// The keycode for `keysym`, preferring the unshifted and shifted levels of the
    /// first group (what the user's layout types), then any level.
    fn keycode(&self, keysym: u32) -> Option<u8> {
        let setup = self.conn.setup();
        let (min, max) = (setup.min_keycode, setup.max_keycode);
        let map = self
            .conn
            .get_keyboard_mapping(min, max - min + 1)
            .ok()?
            .reply()
            .ok()?;
        let per = usize::from(map.keysyms_per_keycode).max(1);
        let rows = || map.keysyms.chunks(per).zip(min..=max);
        rows()
            .find(|(syms, _)| syms.iter().take(2).any(|&s| s == keysym))
            .or_else(|| rows().find(|(syms, _)| syms.contains(&keysym)))
            .map(|(_, code)| code)
    }

    fn modifiers_held(&self) -> bool {
        let held = KeyButMask::SHIFT | KeyButMask::CONTROL | KeyButMask::MOD1 | KeyButMask::MOD4;
        self.conn
            .query_pointer(self.root)
            .ok()
            .and_then(|c| c.reply().ok())
            .is_some_and(|r| r.mask.intersects(held))
    }
}
