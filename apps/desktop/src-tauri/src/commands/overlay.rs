//! Overlay shell commands.
//!
//! This module owns only the native overlay *window* (create/show/hide) and the CharacterEvent
//! delivery bridge. It never calls the file engine: overlay code stays out of every direct
//! file-operation path (`trash_file`, `rename_file`, `create_file`) and out of proposal execution.
//! The overlay can surface state and hand chat input to the draft/proposal flow, but it can never
//! bypass approval, precheck, journal, or execution boundaries.

use crate::overlay::{CharacterEvent, OverlayRuntime, OverlayStatus};

#[cfg(feature = "tauri-commands")]
use crate::overlay::{
    CHARACTER_EVENT_NAME, CHAT_OVERLAY_WINDOW_LABEL, HOUSE_OVERLAY_WINDOW_LABEL,
    OVERLAY_WINDOW_LABEL,
};

#[cfg(feature = "tauri-commands")]
use tauri::{Emitter, Manager};

#[cfg(feature = "tauri-commands")]
const CHAT_OVERLAY_WIDTH: i32 = 450;
#[cfg(feature = "tauri-commands")]
const CHAT_OVERLAY_HEIGHT: i32 = 360;

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
    let window = match app.get_webview_window(OVERLAY_WINDOW_LABEL) {
        Some(window) => window,
        None => {
            build_overlay_window(app).map_err(|error| runtime.mark_not_ready(error).to_string())?
        }
    };
    window.show().map_err(|error| {
        runtime
            .mark_not_ready(format!("cannot show overlay: {error}"))
            .to_string()
    })?;
    let _ = window.set_focus();
    let status = runtime.mark_visible().map_err(|error| error.to_string())?;

    // The house/quick-view surface is secondary. It should never prevent the mouse character from
    // appearing, especially on startup or immediately after first pairing.
    match app
        .get_webview_window(HOUSE_OVERLAY_WINDOW_LABEL)
        .map(Ok)
        .unwrap_or_else(|| build_house_overlay_window(app))
    {
        Ok(house) => {
            if let Err(error) = window_show_without_focus(&house, "house overlay") {
                eprintln!("failed to show secondary house overlay: {error}");
            }
        }
        Err(error) => eprintln!("failed to create secondary house overlay: {error}"),
    }

    // Keep chat ready for the character tap without allowing it to steal focus at startup.
    match app.get_webview_window(CHAT_OVERLAY_WINDOW_LABEL) {
        Some(chat_window) => {
            let _ = chat_window.hide();
        }
        None => {
            if let Err(error) = build_chat_overlay_window(app) {
                eprintln!("failed to preload chat overlay: {error}");
            }
        }
    }
    Ok(status)
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
    if let Some(window) = app.get_webview_window(CHAT_OVERLAY_WINDOW_LABEL) {
        window
            .hide()
            .map_err(|error| format!("WINDOW_MISSING: cannot hide chat overlay: {error}"))?;
    }
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
        if let Some(chat_window) = app.get_webview_window(CHAT_OVERLAY_WINDOW_LABEL) {
            let _ = chat_window.emit(CHARACTER_EVENT_NAME, &event);
        }
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
    if let Some(window) = app.get_webview_window(CHAT_OVERLAY_WINDOW_LABEL) {
        let _ = window.emit(CHARACTER_EVENT_NAME, event);
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
    // Keep the native transparent hit surface close to the mascot. Chat now renders in its own
    // fixed-size window, so this window never resizes at runtime.
    .inner_size(112.0, 140.0)
    .resizable(false)
    .decorations(false)
    .transparent(true)
    .shadow(false)
    .always_on_top(true)
    .focusable(true)
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

/// The chat panel lives in its own fixed-size window instead of resizing the mascot window. That
/// removes the clipping/off-screen bugs that came from growing a `resizable(false)` overlay while
/// the mascot was perched high on the house.
#[cfg(feature = "tauri-commands")]
fn build_chat_overlay_window(app: &tauri::AppHandle) -> Result<tauri::WebviewWindow, String> {
    tauri::WebviewWindowBuilder::new(
        app,
        CHAT_OVERLAY_WINDOW_LABEL,
        tauri::WebviewUrl::App("index.html".into()),
    )
    .title("MouseKeeper Chat")
    .inner_size(CHAT_OVERLAY_WIDTH as f64, CHAT_OVERLAY_HEIGHT as f64)
    .resizable(false)
    .decorations(false)
    .transparent(false)
    .shadow(false)
    .always_on_top(true)
    .focusable(true)
    .skip_taskbar(true)
    .visible(false)
    .build()
    .map_err(|error| format!("WINDOW_MISSING: cannot create chat overlay window: {error}"))
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub fn show_chat_overlay(app: tauri::AppHandle) -> Result<(), String> {
    let window = match app.get_webview_window(CHAT_OVERLAY_WINDOW_LABEL) {
        Some(window) => window,
        None => build_chat_overlay_window(&app)?,
    };
    position_chat_overlay(&app, &window);
    window
        .show()
        .map_err(|error| format!("WINDOW_MISSING: cannot show chat overlay: {error}"))?;
    let _ = window.set_focus();
    Ok(())
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub fn hide_chat_overlay(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(CHAT_OVERLAY_WINDOW_LABEL) {
        window
            .hide()
            .map_err(|error| format!("WINDOW_MISSING: cannot hide chat overlay: {error}"))?;
    }
    Ok(())
}

/// Places the chat near the mascot, then clamps it onto the current monitor and chooses the
/// candidate with the least overlap against the mascot and house windows.
#[cfg(feature = "tauri-commands")]
fn position_chat_overlay(app: &tauri::AppHandle, window: &tauri::WebviewWindow) {
    let Some(mouse) = app.get_webview_window(OVERLAY_WINDOW_LABEL) else {
        return;
    };
    let (Ok(mouse_pos), Ok(mouse_size)) = (mouse.outer_position(), mouse.outer_size()) else {
        return;
    };
    let chat_w = CHAT_OVERLAY_WIDTH;
    let chat_h = CHAT_OVERLAY_HEIGHT;
    let gap = 12;
    let mouse_rect = RectI {
        x: mouse_pos.x,
        y: mouse_pos.y,
        w: mouse_size.width as i32,
        h: mouse_size.height as i32,
    };
    let monitor_rect = mouse.current_monitor().ok().flatten().map(|monitor| RectI {
        x: monitor.position().x,
        y: monitor.position().y,
        w: monitor.size().width as i32,
        h: monitor.size().height as i32,
    });
    let house_rect = app
        .get_webview_window(HOUSE_OVERLAY_WINDOW_LABEL)
        .and_then(|house| {
            let position = house.outer_position().ok()?;
            let size = house.outer_size().ok()?;
            Some(RectI {
                x: position.x,
                y: position.y,
                w: size.width as i32,
                h: size.height as i32,
            })
        });
    let align_y = mouse_pos.y + mouse_size.height as i32 - chat_h;
    let align_x = mouse_pos.x + (mouse_size.width as i32 - chat_w) / 2;
    let candidates = [
        RectI {
            x: mouse_pos.x - chat_w - gap,
            y: align_y,
            w: chat_w,
            h: chat_h,
        },
        RectI {
            x: mouse_pos.x + mouse_size.width as i32 + gap,
            y: align_y,
            w: chat_w,
            h: chat_h,
        },
        RectI {
            x: align_x,
            y: mouse_pos.y - chat_h - gap,
            w: chat_w,
            h: chat_h,
        },
        RectI {
            x: align_x,
            y: mouse_pos.y + mouse_size.height as i32 + gap,
            w: chat_w,
            h: chat_h,
        },
    ];
    let best = candidates
        .into_iter()
        .map(|candidate| clamp_rect(candidate, monitor_rect))
        .min_by_key(|candidate| {
            (
                overlap_area(*candidate, mouse_rect)
                    + house_rect
                        .map(|house| overlap_area(*candidate, house) * 2)
                        .unwrap_or(0),
                distance_squared(*candidate, mouse_rect),
            )
        })
        .unwrap_or(RectI {
            x: mouse_pos.x - chat_w - gap,
            y: align_y,
            w: chat_w,
            h: chat_h,
        });
    let x = best.x;
    let y = best.y;
    let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
}

#[cfg(feature = "tauri-commands")]
#[derive(Clone, Copy)]
struct RectI {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

#[cfg(feature = "tauri-commands")]
fn clamp_rect(rect: RectI, bounds: Option<RectI>) -> RectI {
    let Some(bounds) = bounds else {
        return rect;
    };
    let max_x = bounds.x + (bounds.w - rect.w).max(0);
    let max_y = bounds.y + (bounds.h - rect.h).max(0);
    RectI {
        x: rect.x.clamp(bounds.x, max_x),
        y: rect.y.clamp(bounds.y, max_y),
        ..rect
    }
}

#[cfg(feature = "tauri-commands")]
fn overlap_area(a: RectI, b: RectI) -> i32 {
    let x = (a.x + a.w).min(b.x + b.w) - a.x.max(b.x);
    let y = (a.y + a.h).min(b.y + b.h) - a.y.max(b.y);
    x.max(0) * y.max(0)
}

#[cfg(feature = "tauri-commands")]
fn distance_squared(a: RectI, b: RectI) -> i32 {
    let ax = a.x + a.w / 2;
    let ay = a.y + a.h / 2;
    let bx = b.x + b.w / 2;
    let by = b.y + b.h / 2;
    (ax - bx).pow(2) + (ay - by).pow(2)
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
