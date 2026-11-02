//! Win32 pieces of the spike: secret markers, source app, foreground window, elevation,
//! modifier state, the one-session multi-format write and the Ctrl+V keystroke.

use std::time::{Duration, Instant};

use clipboard_rs::{
    Clipboard, ClipboardContent, ClipboardContext, ContentFormat, RustImageData, common::RustImage,
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

pub enum Marker {
    None,
    DontSave(&'static str),
    DontSync(&'static str),
}

/// SPEC 9.2 secret markers.
pub fn secret_marker(ctx: &ClipboardContext) -> Marker {
    let other = |name: &str| ContentFormat::Other(name.to_string());
    let dword_is_zero = |name: &str| {
        ctx.has(other(name))
            && ctx
                .get_buffer(name)
                .is_ok_and(|b| b.len() >= 4 && b[..4] == [0, 0, 0, 0])
    };
    if ctx.has(other("ExcludeClipboardContentFromMonitorProcessing")) {
        Marker::DontSave("ExcludeClipboardContentFromMonitorProcessing")
    } else if dword_is_zero("CanIncludeInClipboardHistory") {
        Marker::DontSave("CanIncludeInClipboardHistory = 0")
    } else if dword_is_zero("CanUploadToCloudClipboard") {
        Marker::DontSync("CanUploadToCloudClipboard = 0")
    } else {
        Marker::None
    }
}

#[derive(Clone, Copy)]
pub struct Target {
    hwnd: isize,
    pid: u32,
}

impl Target {
    fn hwnd(self) -> HWND {
        HWND(self.hwnd as *mut _)
    }

    pub fn describe(self) -> String {
        format!(
            "{} (pid {})",
            process_name(self.pid).unwrap_or_default(),
            self.pid
        )
    }

    pub fn is_elevated(self) -> bool {
        // A process we cannot query from a normal integrity level is treated as elevated.
        process_elevated(self.pid).unwrap_or(true)
    }

    pub fn activate(self) -> bool {
        unsafe {
            IsWindow(Some(self.hwnd())).as_bool() && SetForegroundWindow(self.hwnd()).as_bool()
        }
    }
}

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

/// Clipboard owner's process image name, falling back to the foreground window's.
pub fn clipboard_source_app() -> Option<String> {
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

/// SPEC 9.1 modifier wait.
pub fn wait_for_modifiers_released(timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        let held = [VK_SHIFT, VK_CONTROL, VK_MENU, VK_LWIN, VK_RWIN]
            .iter()
            .any(|vk| unsafe { GetAsyncKeyState(i32::from(vk.0)) } as u16 & 0x8000 != 0);
        if !held {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

pub fn send_ctrl_v() -> Result<(), String> {
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
    enigo
        .key(Key::Control, Direction::Press)
        .map_err(|e| e.to_string())?;
    let v = enigo
        .key(Key::V, Direction::Click)
        .map_err(|e| e.to_string());
    enigo
        .key(Key::Control, Direction::Release)
        .map_err(|e| e.to_string())?;
    v
}

/// Every format in one open/empty/set/close session, through clipboard-win.
pub fn write_one_session(
    text: Option<&str>,
    html: Option<&str>,
    rtf: Option<&str>,
    png: Option<&[u8]>,
    plain: bool,
) -> Result<Vec<&'static str>, String> {
    use clipboard_win::{Clipboard as Win, options::NoClear, raw, register_format};

    let bmp = match (plain, png) {
        (false, Some(png)) => Some(png_to_bmp(png)?),
        _ => None,
    };
    let _open = Win::new_attempts(10).map_err(|e| format!("open clipboard: {e}"))?;
    raw::empty().map_err(|e| format!("empty: {e}"))?;
    let mut landed = Vec::new();
    if let Some(t) = text
        && raw::set_string_with(t, NoClear).is_ok()
    {
        landed.push("text");
    }
    if plain {
        return Ok(landed);
    }
    if let (Some(h), Some(f)) = (html, register_format("HTML Format"))
        && raw::set_html_with(f.get(), h, NoClear).is_ok()
    {
        landed.push("html");
    }
    if let (Some(r), Some(f)) = (rtf, register_format("Rich Text Format"))
        && raw::set_without_clear(f.get(), r.as_bytes()).is_ok()
    {
        landed.push("rtf");
    }
    if let (Some(p), Some(f)) = (png, register_format("PNG"))
        && raw::set_without_clear(f.get(), p).is_ok()
    {
        landed.push("png");
    }
    if let Some(bmp) = bmp
        && raw::set_bitmap_with(&bmp, NoClear).is_ok()
    {
        landed.push("bitmap");
    }
    Ok(landed)
}

fn png_to_bmp(png: &[u8]) -> Result<Vec<u8>, String> {
    let img = RustImageData::from_bytes(png).map_err(|e| e.to_string())?;
    img.to_bitmap()
        .map(|b| b.get_bytes().to_vec())
        .map_err(|e| e.to_string())
}

/// clipboard-rs `set`, image first, kept to compare against the one-session write.
pub fn write_with_clipboard_rs(
    text: Option<String>,
    html: Option<String>,
    rtf: Option<String>,
    png: Option<Vec<u8>>,
    plain: bool,
) -> Result<Vec<&'static str>, String> {
    let ctx = ClipboardContext::new().map_err(|e| e.to_string())?;
    let mut contents = Vec::new();
    if !plain && let Some(p) = png {
        contents.push(ClipboardContent::Image(
            RustImageData::from_bytes(&p).map_err(|e| e.to_string())?,
        ));
    }
    if let Some(t) = text {
        contents.push(ClipboardContent::Text(t));
    }
    if !plain {
        if let Some(h) = html {
            contents.push(ClipboardContent::Html(h));
        }
        if let Some(r) = rtf {
            contents.push(ClipboardContent::Rtf(r));
        }
    }
    ctx.set(contents).map_err(|e| e.to_string())?;
    Ok(vec!["(clipboard-rs set; check formats with Ctrl+T)"])
}
