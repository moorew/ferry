//! Tray and menu bar icon (SPEC 10.1).

use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, Wry};

use crate::picker;
use crate::state::AppState;

const TRAY: &str = "main";

struct Items {
    pause: CheckMenuItem<Wry>,
}

fn icon(paused: bool) -> Image<'static> {
    let bytes: &'static [u8] = match (cfg!(target_os = "macos"), cfg!(windows), paused) {
        (true, _, false) => include_bytes!("../icons/tray/tray-template@2x.png"),
        (true, _, true) => include_bytes!("../icons/tray/tray-template-paused@2x.png"),
        (_, true, false) => include_bytes!("../icons/tray/tray-tile.png"),
        (_, true, true) => include_bytes!("../icons/tray/tray-tile-paused.png"),
        (_, _, false) => include_bytes!("../icons/tray/tray-light@2x.png"),
        (_, _, true) => include_bytes!("../icons/tray/tray-light-paused@2x.png"),
    };
    Image::from_bytes(bytes).expect("bundled tray icon")
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
    TrayIconBuilder::with_id(TRAY)
        .icon(icon(false))
        .icon_as_template(cfg!(target_os = "macos"))
        .tooltip("TailCopy")
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
    Ok(())
}

/// Brings the icon and the Pause item in line with the current state.
pub fn refresh(app: &AppHandle) {
    let paused = app.state::<AppState>().is_paused();
    if let Some(tray) = app.tray_by_id(TRAY) {
        let _ = tray.set_icon(Some(icon(paused)));
        let _ = tray.set_icon_as_template(cfg!(target_os = "macos"));
        let _ = tray.set_tooltip(Some(if paused {
            "TailCopy (paused)"
        } else {
            "TailCopy"
        }));
    }
    if let Some(items) = app.try_state::<Items>() {
        let _ = items.pause.set_checked(paused);
    }
}
