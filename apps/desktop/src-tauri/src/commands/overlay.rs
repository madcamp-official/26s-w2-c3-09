//! Overlay shell commands.
//!
//! This module owns only the native overlay *window* (create/show/hide) and the CharacterEvent
//! delivery bridge. It never calls the file engine: overlay code stays out of every direct
//! file-operation path (`trash_file`, `rename_file`, `create_file`) and out of proposal execution.
//! The overlay can surface state and hand chat input to the draft/proposal flow, but it can never
//! bypass approval, precheck, journal, or execution boundaries.

use crate::overlay::{CharacterEvent, OverlayRuntime, OverlayStatus};

#[cfg(feature = "tauri-commands")]
use crate::overlay::{CHARACTER_EVENT_NAME, HOUSE_OVERLAY_WINDOW_LABEL, OVERLAY_WINDOW_LABEL};

#[cfg(feature = "tauri-commands")]
use tauri::{Emitter, Manager};

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub fn get_overlay_status(
    runtime: tauri::State<'_, OverlayRuntime>,
) -> Result<OverlayStatus, String> {
    get_overlay_status_impl(&runtime)
}

#[cfg(not(feature = "tauri-commands"))]
pub fn get_overlay_status(runtime: &OverlayRuntime) -> Result<OverlayStatus, String> {
    get_overlay_status_impl(runtime)
}

/// Creates the overlay window if it does not exist yet, then shows it. B's character UI renders
/// inside this window (routed by window label in the frontend).
#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub fn show_overlay(
    app: tauri::AppHandle,
    runtime: tauri::State<'_, OverlayRuntime>,
) -> Result<OverlayStatus, String> {
    show_overlay_window(&app, &runtime)
}

#[cfg(feature = "tauri-commands")]
pub fn show_overlay_window(
    app: &tauri::AppHandle,
    runtime: &OverlayRuntime,
) -> Result<OverlayStatus, String> {
    let house = match app.get_webview_window(HOUSE_OVERLAY_WINDOW_LABEL) {
        Some(window) => window,
        None => build_house_overlay_window(app)?,
    };
    window_show_without_focus(&house, "house overlay")?;

    let window = match app.get_webview_window(OVERLAY_WINDOW_LABEL) {
        Some(window) => window,
        None => build_overlay_window(app)?,
    };
    window
        .show()
        .map_err(|error| format!("WINDOW_MISSING: cannot show overlay: {error}"))?;
    let _ = window.set_focus();
    runtime.mark_visible().map_err(|error| error.to_string())
}

#[cfg(not(feature = "tauri-commands"))]
pub fn show_overlay(runtime: &OverlayRuntime) -> Result<OverlayStatus, String> {
    runtime.mark_visible().map_err(|error| error.to_string())
}

/// Hides the overlay window (keeps it alive). The main window, tray, close-to-tray, and autostart
/// lifecycles are unaffected — the overlay is an independent window.
#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub fn hide_overlay(
    app: tauri::AppHandle,
    runtime: tauri::State<'_, OverlayRuntime>,
) -> Result<OverlayStatus, String> {
    if let Some(window) = app.get_webview_window(OVERLAY_WINDOW_LABEL) {
        window
            .hide()
            .map_err(|error| format!("WINDOW_MISSING: cannot hide overlay: {error}"))?;
    }
    if let Some(window) = app.get_webview_window(HOUSE_OVERLAY_WINDOW_LABEL) {
        window
            .hide()
            .map_err(|error| format!("WINDOW_MISSING: cannot hide house overlay: {error}"))?;
    }
    runtime.mark_hidden().map_err(|error| error.to_string())
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub fn set_house_overlay_locked(app: tauri::AppHandle, locked: bool) -> Result<(), String> {
    let window = match app.get_webview_window(HOUSE_OVERLAY_WINDOW_LABEL) {
        Some(window) => window,
        None => build_house_overlay_window(&app)?,
    };
    let _ = window.set_always_on_bottom(true);
    if !locked {
        window_show_without_focus(&window, "house overlay")?;
    }
    Ok(())
}

#[cfg(not(feature = "tauri-commands"))]
pub fn hide_overlay(runtime: &OverlayRuntime) -> Result<OverlayStatus, String> {
    runtime.mark_hidden().map_err(|error| error.to_string())
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub fn emit_character_event(
    event: CharacterEvent,
    app: tauri::AppHandle,
    runtime: tauri::State<'_, OverlayRuntime>,
) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(OVERLAY_WINDOW_LABEL) {
        window
            .emit(CHARACTER_EVENT_NAME, &event)
            .map_err(|error| format!("EMIT_FAILED: {error}"))?;
        runtime
            .mark_event_accepted(&event)
            .map_err(|error| error.to_string())?;
        Ok(())
    } else {
        Err(runtime
            .mark_not_ready("overlay window is not created yet")
            .to_string())
    }
}

#[cfg(not(feature = "tauri-commands"))]
pub fn emit_character_event(
    runtime: &OverlayRuntime,
    _event: CharacterEvent,
) -> Result<(), String> {
    Err(runtime
        .mark_not_ready("overlay window is not created yet")
        .to_string())
}

/// Best-effort character event delivery used by the background bridge. Unlike the explicit
/// `emit_character_event` command, this silently does nothing when the overlay is not open — a
/// closed overlay is a normal state, not an error to surface in the runtime.
#[cfg(feature = "tauri-commands")]
pub fn emit_character_event_if_open(
    app: &tauri::AppHandle,
    runtime: &OverlayRuntime,
    event: &CharacterEvent,
) {
    if let Some(window) = app.get_webview_window(OVERLAY_WINDOW_LABEL) {
        if window.emit(CHARACTER_EVENT_NAME, event).is_ok() {
            let _ = runtime.mark_event_accepted(event);
        }
    }
}

#[cfg(feature = "tauri-commands")]
fn build_overlay_window(app: &tauri::AppHandle) -> Result<tauri::WebviewWindow, String> {
    tauri::WebviewWindowBuilder::new(
        app,
        OVERLAY_WINDOW_LABEL,
        tauri::WebviewUrl::App("index.html".into()),
    )
    .title("MouseKeeper")
    // Keep the native transparent hit surface close to the mascot while the bubble is closed.
    // The frontend expands the same window only while the prompt/chat panel is visible.
    .inner_size(112.0, 140.0)
    .resizable(false)
    .decorations(false)
    .transparent(true)
    .shadow(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .build()
    .map_err(|error| format!("WINDOW_MISSING: cannot create overlay window: {error}"))
}

#[cfg(feature = "tauri-commands")]
fn build_house_overlay_window(app: &tauri::AppHandle) -> Result<tauri::WebviewWindow, String> {
    let window = tauri::WebviewWindowBuilder::new(
        app,
        HOUSE_OVERLAY_WINDOW_LABEL,
        tauri::WebviewUrl::App("index.html".into()),
    )
    .title("MouseKeeper House")
    .inner_size(820.0, 590.0)
    .resizable(false)
    .decorations(false)
    .transparent(true)
    .shadow(false)
    .always_on_bottom(true)
    .focusable(true)
    .skip_taskbar(true)
    .build()
    .map_err(|error| format!("WINDOW_MISSING: cannot create house overlay window: {error}"))?;

    position_house_overlay(app, &window);
    Ok(window)
}

#[cfg(feature = "tauri-commands")]
fn window_show_without_focus(window: &tauri::WebviewWindow, name: &str) -> Result<(), String> {
    window
        .show()
        .map_err(|error| format!("WINDOW_MISSING: cannot show {name}: {error}"))?;
    let _ = window.set_always_on_bottom(true);
    Ok(())
}

#[cfg(feature = "tauri-commands")]
fn position_house_overlay(app: &tauri::AppHandle, window: &tauri::WebviewWindow) {
    let Ok(Some(monitor)) = app.primary_monitor() else {
        return;
    };
    let monitor_position = monitor.position();
    let monitor_size = monitor.size();
    let x = monitor_position.x + 32;
    let y = monitor_position.y + monitor_size.height as i32 - 590 - 48;
    let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
}

fn get_overlay_status_impl(runtime: &OverlayRuntime) -> Result<OverlayStatus, String> {
    runtime.status().map_err(|error| error.to_string())
}

// These tests exercise the `not(feature = "tauri-commands")` variants above, which take a plain
// `&OverlayRuntime` so they are callable without a live Tauri `AppHandle`/`State`. The
// `tauri-commands` variants need a running app to construct their `State` argument and are not
// unit-testable this way, so this module only compiles for the CLI/no-tauri-commands build.
#[cfg(all(test, not(feature = "tauri-commands")))]
mod tests {
    use crate::overlay::{CharacterEvent, CharacterEventKind, OverlayRuntime, OverlayState};

    use super::{emit_character_event, get_overlay_status, hide_overlay, show_overlay};

    #[test]
    fn status_reports_overlay_not_ready() {
        let runtime = OverlayRuntime::default();

        let status = get_overlay_status(&runtime).expect("status");

        assert_eq!(status.state, OverlayState::NotReady);
    }

    #[test]
    fn show_then_hide_updates_visibility_state() {
        let runtime = OverlayRuntime::default();

        let shown = show_overlay(&runtime).expect("show");
        assert_eq!(shown.state, OverlayState::Visible);

        let hidden = hide_overlay(&runtime).expect("hide");
        assert_eq!(hidden.state, OverlayState::Hidden);
    }

    #[test]
    fn emit_character_event_refuses_to_fake_delivery_without_window() {
        let runtime = OverlayRuntime::default();
        let event = CharacterEvent {
            kind: CharacterEventKind::Analyzing,
            message: Some("Analyzing managed root".to_string()),
            correlation_id: Some("command-1".to_string()),
        };

        let error = emit_character_event(&runtime, event).expect_err("emit fails");

        assert!(error.contains("NOT_READY"));
    }
}
