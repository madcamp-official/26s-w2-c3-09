#[cfg(feature = "tauri-commands")]
use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    App, Manager,
};

#[cfg(feature = "tauri-commands")]
const MAIN_WINDOW_LABEL: &str = "main";
#[cfg(feature = "tauri-commands")]
const TRAY_ID: &str = "mousekeeper-main-tray";
#[cfg(feature = "tauri-commands")]
const MENU_OPEN_MANAGER: &str = "open_manager";
#[cfg(feature = "tauri-commands")]
const MENU_HIDE_MANAGER: &str = "hide_manager";
#[cfg(feature = "tauri-commands")]
const MENU_SHOW_OVERLAY: &str = "show_overlay";
#[cfg(feature = "tauri-commands")]
const MENU_PAUSE_AGENT: &str = "pause_agent";
#[cfg(feature = "tauri-commands")]
const MENU_RESUME_AGENT: &str = "resume_agent";
#[cfg(feature = "tauri-commands")]
const MENU_QUIT: &str = "quit";

#[cfg(feature = "tauri-commands")]
pub fn install_tray(app: &App) -> Result<(), String> {
    let open_manager =
        MenuItem::with_id(app, MENU_OPEN_MANAGER, "Open manager", true, None::<&str>)
            .map_err(|error| format!("cannot create tray menu item: {error}"))?;
    let hide_manager =
        MenuItem::with_id(app, MENU_HIDE_MANAGER, "Hide manager", true, None::<&str>)
            .map_err(|error| format!("cannot create tray menu item: {error}"))?;
    let show_overlay =
        MenuItem::with_id(app, MENU_SHOW_OVERLAY, "Show overlay", true, None::<&str>)
            .map_err(|error| format!("cannot create tray menu item: {error}"))?;
    let pause_agent = MenuItem::with_id(app, MENU_PAUSE_AGENT, "Pause agent", true, None::<&str>)
        .map_err(|error| format!("cannot create tray menu item: {error}"))?;
    let resume_agent =
        MenuItem::with_id(app, MENU_RESUME_AGENT, "Resume agent", true, None::<&str>)
            .map_err(|error| format!("cannot create tray menu item: {error}"))?;
    let quit = MenuItem::with_id(app, MENU_QUIT, "Quit", true, None::<&str>)
        .map_err(|error| format!("cannot create tray menu item: {error}"))?;
    let menu = Menu::with_items(
        app,
        &[
            &open_manager,
            &hide_manager,
            &show_overlay,
            &pause_agent,
            &resume_agent,
            &quit,
        ],
    )
    .map_err(|error| format!("cannot create tray menu: {error}"))?;

    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("MouseKeeper")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            MENU_OPEN_MANAGER => show_main_window(app),
            MENU_HIDE_MANAGER => hide_main_window(app),
            MENU_SHOW_OVERLAY => show_overlay_from_tray(app),
            MENU_PAUSE_AGENT => pause_background_runtime(app),
            MENU_RESUME_AGENT => resume_background_runtime(app),
            MENU_QUIT => app.exit(0),
            _ => {}
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    } else {
        builder = builder.icon(fallback_tray_icon());
    }

    builder
        .build(app)
        .map(|_| ())
        .map_err(|error| format!("cannot install tray icon: {error}"))
}

#[cfg(feature = "tauri-commands")]
pub fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[cfg(feature = "tauri-commands")]
pub fn hide_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        let _ = window.hide();
    }
}

#[cfg(feature = "tauri-commands")]
pub fn show_overlay_from_tray(app: &tauri::AppHandle) {
    let runtime = app.state::<crate::overlay::OverlayRuntime>();
    if let Err(error) = crate::commands::overlay::show_overlay_window(app, runtime.inner()) {
        eprintln!("failed to show overlay from tray: {error}");
    }
}

#[cfg(feature = "tauri-commands")]
pub fn pause_background_runtime(app: &tauri::AppHandle) {
    let runtime = app.state::<crate::background::BackgroundRuntime>();
    let _ = runtime.pause("paused from system tray");
}

#[cfg(feature = "tauri-commands")]
pub fn resume_background_runtime(app: &tauri::AppHandle) {
    let runtime = app.state::<crate::background::BackgroundRuntime>();
    let _ = runtime.start(app.clone());
}

#[cfg(feature = "tauri-commands")]
pub fn fallback_tray_icon() -> Image<'static> {
    let rgba = vec![
        0x20, 0x21, 0x24, 0xff, 0xff, 0xff, 0xff, 0xff, 0x20, 0x21, 0x24, 0xff, 0x20, 0x21, 0x24,
        0xff,
    ];
    Image::new_owned(rgba, 2, 2)
}
