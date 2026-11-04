//! Tray and menu bar icon (SPEC 10.1, design handoff section 3).
//!
//! States here are idle, paused and offline (Tailscale not running). The syncing
//! frames and the new-item dot arrive with sync in M2.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, Wry};

use crate::picker;
use crate::state::AppState;

const TRAY: &str = "main";
const TAILSCALE_CHECK: Duration = Duration::from_secs(30);

/// Last known Tailscale state, so the icon can show offline without a fresh query.
static OFFLINE: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum TrayState {
    Idle,
    Paused,
    Offline,
}

impl TrayState {
    fn tooltip(self) -> &'static str {
        match self {
            TrayState::Idle => "TailCopy",
            TrayState::Paused => "TailCopy (paused)",
            TrayState::Offline => "TailCopy (Tailscale isn't running)",
        }
    }
}

macro_rules! tray_png {
    ($state:expr, $variant:literal) => {
        match $state {
            TrayState::Idle => {
                include_bytes!(concat!("../icons/tray/tray-idle-", $variant, ".png")) as &[u8]
            }
            TrayState::Paused => {
                include_bytes!(concat!("../icons/tray/tray-paused-", $variant, ".png"))
            }
            TrayState::Offline => {
                include_bytes!(concat!("../icons/tray/tray-offline-", $variant, ".png"))
            }
        }
    };
}

/// Black template images; the menu bar tints them for light, dark and highlighted.
#[cfg(target_os = "macos")]
fn icon_bytes(_app: &AppHandle, state: TrayState) -> &'static [u8] {
    tray_png!(state, "black-48")
}

/// GNOME's top bar is always dark, so the white glyph (16 px design at 2x).
#[cfg(target_os = "linux")]
fn icon_bytes(_app: &AppHandle, state: TrayState) -> &'static [u8] {
    tray_png!(state, "16-white-32")
}

/// White on a dark taskbar, ink on a light one, sized for the display scale:
/// 16 px at 100%, then 20, 24 and 32.
#[cfg(windows)]
fn icon_bytes(app: &AppHandle, state: TrayState) -> &'static [u8] {
    let scale = app
        .primary_monitor()
        .ok()
        .flatten()
        .map_or(1.0, |m| m.scale_factor());
    match (taskbar::uses_light_theme(), scale) {
        (false, s) if s < 1.2 => tray_png!(state, "16-white-16"),
        (false, s) if s < 1.4 => tray_png!(state, "white-20"),
        (false, s) if s < 1.75 => tray_png!(state, "white-24"),
        (false, _) => tray_png!(state, "16-white-32"),
        (true, s) if s < 1.2 => tray_png!(state, "16-ink-16"),
        (true, s) if s < 1.4 => tray_png!(state, "ink-20"),
        (true, s) if s < 1.75 => tray_png!(state, "ink-24"),
        (true, _) => tray_png!(state, "16-ink-32"),
    }
}

fn current(app: &AppHandle) -> TrayState {
    if app.state::<AppState>().is_paused() {
        TrayState::Paused
    } else if OFFLINE.load(Ordering::Relaxed) {
        TrayState::Offline
    } else {
        TrayState::Idle
    }
}

fn icon(app: &AppHandle, state: TrayState) -> Image<'static> {
    Image::from_bytes(icon_bytes(app, state)).expect("bundled tray icon")
}

struct Items {
    pause: CheckMenuItem<Wry>,
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open TailCopy", true, None::<&str>)?;
    let history = MenuItem::with_id(app, "history", "History…", true, None::<&str>)?;
    let pause = CheckMenuItem::with_id(app, "pause", "Pause capture", true, false, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit TailCopy", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &open,
            &history,
            &PredefinedMenuItem::separator(app)?,
            &pause,
            &PredefinedMenuItem::separator(app)?,
            &settings,
            &quit,
        ],
    )?;
    app.manage(Items {
        pause: pause.clone(),
    });
    let state = current(app);
    TrayIconBuilder::with_id(TRAY)
        .icon(icon(app, state))
        .icon_as_template(cfg!(target_os = "macos"))
        .tooltip(state.tooltip())
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move { picker::toggle_picker(&app).await });
            }
            "history" => picker::show_manager(app, Some("history")),
            "settings" => picker::show_manager(app, Some("settings")),
            "pause" => {
                let paused = !app.state::<AppState>().is_paused();
                crate::state::set_paused(app, paused);
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;

    #[cfg(windows)]
    {
        let app = app.clone();
        taskbar::watch(move || refresh(&app));
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move { watch_tailscale(app).await });
    Ok(())
}

/// Brings the icon, tooltip and Pause item in line with the current state.
pub fn refresh(app: &AppHandle) {
    let state = current(app);
    if let Some(tray) = app.tray_by_id(TRAY) {
        let _ = tray.set_icon(Some(icon(app, state)));
        let _ = tray.set_icon_as_template(cfg!(target_os = "macos"));
        let _ = tray.set_tooltip(Some(state.tooltip()));
    }
    if let Some(items) = app.try_state::<Items>() {
        let _ = items.pause.set_checked(state == TrayState::Paused);
    }
}

/// Records what the latest status said about Tailscale; `refresh` shows it.
pub fn set_tailscale_running(running: bool) {
    OFFLINE.store(!running, Ordering::Relaxed);
}

/// Notices Tailscale starting or stopping while TailCopy runs. M2's tailnet adapter
/// replaces this with its own watch.
async fn watch_tailscale(app: AppHandle) {
    let mut last = None;
    loop {
        let running = crate::tailscale::state().await == "running";
        if last.is_some_and(|was| was != running) {
            crate::state::status_changed(&app);
        } else if last.is_none() {
            set_tailscale_running(running);
            refresh(&app);
        }
        last = Some(running);
        tokio::time::sleep(TAILSCALE_CHECK).await;
    }
}

/// Windows taskbar colour: `SystemUsesLightTheme` under the user's Personalize key.
#[cfg(windows)]
mod taskbar {
    use windows::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, KEY_NOTIFY, REG_NOTIFY_CHANGE_LAST_SET, RRF_RT_REG_DWORD,
        RegCloseKey, RegGetValueW, RegNotifyChangeKeyValue, RegOpenKeyExW,
    };
    use windows::core::{PCWSTR, w};

    const PERSONALIZE: PCWSTR = w!(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize");

    /// False when the value is missing: Windows 10 and 11 default to a dark taskbar.
    pub fn uses_light_theme() -> bool {
        let mut value = 0u32;
        let mut size = std::mem::size_of::<u32>() as u32;
        let err = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                PERSONALIZE,
                w!("SystemUsesLightTheme"),
                RRF_RT_REG_DWORD,
                None,
                Some((&raw mut value).cast()),
                Some(&mut size),
            )
        };
        err.is_ok() && value == 1
    }

    /// Calls `changed` on its own thread whenever a Personalize value changes.
    pub fn watch(changed: impl Fn() + Send + 'static) {
        let spawned = std::thread::Builder::new()
            .name("taskbar-theme".into())
            .spawn(move || {
                let mut key = HKEY::default();
                let opened = unsafe {
                    RegOpenKeyExW(HKEY_CURRENT_USER, PERSONALIZE, None, KEY_NOTIFY, &mut key)
                };
                if opened.is_err() {
                    tracing::warn!("can't watch the taskbar theme: {opened:?}");
                    return;
                }
                let mut light = uses_light_theme();
                while unsafe {
                    RegNotifyChangeKeyValue(key, false, REG_NOTIFY_CHANGE_LAST_SET, None, false)
                }
                .is_ok()
                {
                    let now = uses_light_theme();
                    if now != light {
                        light = now;
                        changed();
                    }
                }
                let _ = unsafe { RegCloseKey(key) };
            });
        if let Err(e) = spawned {
            tracing::warn!("can't start the taskbar theme watcher: {e}");
        }
    }
}
