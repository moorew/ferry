//! Windows backend (SPEC 9.2): clipboard-rs for watching and reading, clipboard-win for
//! writing every format in one session (D-012), Win32 for focus, elevation and the
//! source app, enigo for Ctrl+V.

use std::{sync::mpsc, time::Duration};

use clipboard_rs::{
    Clipboard, ClipboardContext, ClipboardHandler, ClipboardWatcher, ClipboardWatcherContext,
    ContentFormat, RustImageData, common::RustImage,
};
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use windows::Win32::{
    Foundation::{CloseHandle, HANDLE, HWND},
    Security::{GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation},
    System::{
        DataExchange::GetClipboardOwner,
        Threading::{
            GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_NAME_WIN32,
            PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
        },
    },
    UI::{
        Input::KeyboardAndMouse::{
            GetAsyncKeyState, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
        },
        WindowsAndMessaging::{
            GetForegroundWindow, GetWindowThreadProcessId, IsWindow, SetForegroundWindow,
        },
    },
};
use windows::core::PWSTR;

use crate::{
    Captured, CopyOnlyReason, Error, Marker, PasteResult, Result, Write, WriteOutcome,
    wait_until_released,
};

const FOCUS_SETTLE: Duration = Duration::from_millis(30);
const MODIFIER_WAIT: Duration = Duration::from_millis(500);

struct Notify(mpsc::Sender<()>);

impl ClipboardHandler for Notify {
    fn on_clipboard_change(&mut self) {
        let _ = self.0.send(());
    }
}

/// Starts the clipboard format listener on its own thread; one message per change.
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

pub fn read() -> Result<Captured> {
    let ctx = ClipboardContext::new().map_err(|e| Error::Clipboard(e.to_string()))?;
    let raw_types = ctx.available_formats().unwrap_or_default();
    let source_app = clipboard_source_app();
    let marker = secret_marker(&ctx);
    if matches!(marker, Marker::DontSave(_)) {
        return Ok(Captured {
            raw_types,
            source_app,
            marker,
            ..Captured::default()
        });
    }
    let text = ctx
        .has(ContentFormat::Text)
        .then(|| ctx.get_text().ok())
        .flatten();
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
    Ok(Captured {
        text,
        html,
        rtf,
        png,
        raw_types,
        source_app,
        marker,
    })
}

fn secret_marker(ctx: &ClipboardContext) -> Marker {
    let other = |name: &str| ContentFormat::Other(name.to_string());
    let dword_is_zero = |name: &str| {
        ctx.has(other(name))
            && ctx
                .get_buffer(name)
                .is_ok_and(|b| b.len() >= 4 && b[..4] == [0, 0, 0, 0])
    };
    if ctx.has(other("ExcludeClipboardContentFromMonitorProcessing")) {
        Marker::DontSave("ExcludeClipboardContentFromMonitorProcessing".into())
    } else if dword_is_zero("CanIncludeInClipboardHistory") {
        Marker::DontSave("CanIncludeInClipboardHistory".into())
    } else if dword_is_zero("CanUploadToCloudClipboard") {
        Marker::DontSync("CanUploadToCloudClipboard".into())
    } else {
        Marker::None
    }
}

/// Writes every supplied format in one open/empty/set/close session.
pub fn write(clip: &Write<'_>) -> Result<WriteOutcome> {
    use clipboard_win::{Clipboard as Session, options::NoClear, raw, register_format};

    let bmp = match clip.png {
        Some(png) => Some(png_to_bmp(png)?),
        None => None,
    };
    let _open = Session::new_attempts(10).map_err(|e| Error::Clipboard(e.to_string()))?;
    raw::empty().map_err(|e| Error::Clipboard(e.to_string()))?;
    let mut landed = Vec::new();
    if let Some(t) = clip.text
        && raw::set_string_with(t, NoClear).is_ok()
    {
        landed.push("text/plain");
    }
    if let (Some(h), Some(f)) = (clip.html, register_format("HTML Format"))
        && raw::set_html_with(f.get(), h, NoClear).is_ok()
    {
        landed.push("text/html");
    }
    if let (Some(r), Some(f)) = (clip.rtf, register_format("Rich Text Format"))
        && raw::set_without_clear(f.get(), r.as_bytes()).is_ok()
    {
        landed.push("text/rtf");
    }
    if let (Some(p), Some(f)) = (clip.png, register_format("PNG"))
        && raw::set_without_clear(f.get(), p).is_ok()
    {
        landed.push("image/png");
    }
    if let Some(bmp) = bmp
        && raw::set_bitmap_with(&bmp, NoClear).is_ok()
        && !landed.contains(&"image/png")
    {
        landed.push("image/png");
    }
    Ok(WriteOutcome { landed })
}

fn png_to_bmp(png: &[u8]) -> Result<Vec<u8>> {
    let img = RustImageData::from_bytes(png).map_err(|e| Error::Clipboard(e.to_string()))?;
    img.to_bitmap()
        .map(|b| b.get_bytes().to_vec())
        .map_err(|e| Error::Clipboard(e.to_string()))
}

/// The window that had focus when the picker opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Target {
    hwnd: isize,
    pid: u32,
}

impl Target {
    fn hwnd(self) -> HWND {
        HWND(self.hwnd as *mut _)
    }

    pub fn process_name(self) -> Option<String> {
        process_name(self.pid)
    }

    fn is_elevated(self) -> bool {
        // A process a normal integrity level can't query is treated as elevated.
        process_elevated(self.pid).unwrap_or(true)
    }

    fn activate(self) -> bool {
        unsafe {
            IsWindow(Some(self.hwnd())).as_bool() && SetForegroundWindow(self.hwnd()).as_bool()
        }
    }
}

/// The foreground window, unless it belongs to TailCopy itself.
pub fn foreground() -> Option<Target> {
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.is_invalid() {
        return None;
    }
    let mut pid = 0;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    (pid != std::process::id()).then_some(Target {
        hwnd: hwnd.0 as isize,
        pid,
    })
}

/// Restores focus to `target` (when given), waits for modifiers, then sends Ctrl+V.
pub fn paste(target: Option<Target>) -> PasteResult {
    if let Some(target) = target {
        if target.is_elevated() && !self_is_elevated() {
            return PasteResult::CopiedOnly(CopyOnlyReason::ElevatedTarget);
        }
        if !target.activate() {
            tracing::warn!("could not restore focus to the previous window");
        }
        std::thread::sleep(FOCUS_SETTLE);
    }
    if !wait_until_released(MODIFIER_WAIT, modifiers_held) {
        return PasteResult::CopiedOnly(CopyOnlyReason::ModifiersHeld);
    }
    match send_ctrl_v() {
        Ok(()) => PasteResult::Pasted,
        Err(e) => {
            tracing::warn!("Ctrl+V failed: {e}");
            PasteResult::CopiedOnly(CopyOnlyReason::Failed)
        }
    }
}

fn modifiers_held() -> bool {
    [VK_SHIFT, VK_CONTROL, VK_MENU, VK_LWIN, VK_RWIN]
        .iter()
        .any(|vk| unsafe { GetAsyncKeyState(i32::from(vk.0)) } as u16 & 0x8000 != 0)
}

fn send_ctrl_v() -> Result<()> {
    let input = |e: enigo::InputError| Error::Input(e.to_string());
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| Error::Input(e.to_string()))?;
    enigo.key(Key::Control, Direction::Press).map_err(input)?;
    let v = enigo.key(Key::V, Direction::Click).map_err(input);
    enigo.key(Key::Control, Direction::Release).map_err(input)?;
    v
}

/// Clipboard owner's process image name, falling back to the foreground window's.
fn clipboard_source_app() -> Option<String> {
    let owner = unsafe { GetClipboardOwner() }.ok();
    let hwnd = owner.unwrap_or_else(|| unsafe { GetForegroundWindow() });
    let mut pid = 0;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    process_name(pid)
}

fn with_process<T>(pid: u32, f: impl FnOnce(HANDLE) -> Option<T>) -> Option<T> {
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
    let result = f(handle);
    let _ = unsafe { CloseHandle(handle) };
    result
}

fn process_name(pid: u32) -> Option<String> {
    with_process(pid, |handle| {
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        unsafe {
            QueryFullProcessImageNameW(
                handle,
                PROCESS_NAME_WIN32,
                PWSTR(buf.as_mut_ptr()),
                &mut len,
            )
        }
        .ok()?;
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        path.rsplit('\\').next().map(str::to_string)
    })
}

fn token_elevated(process: HANDLE) -> Option<bool> {
    let mut token = HANDLE::default();
    unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) }.ok()?;
    let mut elevation = TOKEN_ELEVATION::default();
    let mut len = 0;
    let result = unsafe {
        GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut _),
            size_of::<TOKEN_ELEVATION>() as u32,
            &mut len,
        )
    };
    let _ = unsafe { CloseHandle(token) };
    result.ok()?;
    Some(elevation.TokenIsElevated != 0)
}

fn process_elevated(pid: u32) -> Option<bool> {
    with_process(pid, token_elevated)
}

pub fn self_is_elevated() -> bool {
    token_elevated(unsafe { GetCurrentProcess() }).unwrap_or(false)
}
