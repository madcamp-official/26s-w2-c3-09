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
    OVERLAY_WINDOW_LABEL, SPEECH_BUBBLE_CLOSED_EVENT, SPEECH_BUBBLE_OVERLAY_WINDOW_LABEL,
    SPEECH_BUBBLE_TEXT_EVENT,
};

#[cfg(feature = "tauri-commands")]
use tauri::{Emitter, Manager};

#[cfg(feature = "tauri-commands")]
const CHAT_OVERLAY_WIDTH: i32 = 450;
#[cfg(feature = "tauri-commands")]
const CHAT_OVERLAY_HEIGHT: i32 = 360;
// Wider/taller than the visible bubble itself so the tail (which pokes 16px past the bubble edge)
// always has room to render before hitting the window's own edge and getting clipped.
#[cfg(feature = "tauri-commands")]
const SPEECH_BUBBLE_WIDTH: i32 = 300;
#[cfg(feature = "tauri-commands")]
const SPEECH_BUBBLE_HEIGHT: i32 = 130;
// Negative: measured against the actual mouse artwork, not just its bounding boxes. The mascot's
// 112x140 window has ~8px of transparent padding around its 96x118 drag-surface box, and inside
// that box `object-fit: contain` plus the source sprite's own margin (mouse_idle_preview.gif is
// 1400x1800 but the drawn mouse only spans roughly x:[229,1211]) adds ~17px more empty space
// before the drawn pixels start — about 25px of total invisible padding on the mascot side.
// This only works together with two things in styles.css: the *visible* bubble box is
// edge-anchored to the side of its window facing the mascot (`.speech-bubble-overlay--tail-*`,
// not centered — otherwise moving the window closer wouldn't move the visible bubble any closer,
// it would just shrink dead space around it), and the bubble's tail (which points at the mascot)
// pokes 16px past that box edge through the window's 20px padding, landing ~4px shy of the
// window's own edge. Safe to sit this deep in the mascot's padding either way: the bubble window
// is click-through (`set_ignore_cursor_events`), so it can never block dragging the mascot.
#[cfg(feature = "tauri-commands")]
const SPEECH_BUBBLE_GAP: i32 = 12;

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
    // Same reasoning as chat: build the speech bubble webview now (minutes before the idle timer
    // first fires) so its listener for SPEECH_BUBBLE_TEXT_EVENT is already registered by the time
    // `show_speech_bubble` actually emits — otherwise the very first bubble's text is emitted
    // before the fresh webview finishes booting and is silently dropped.
    if app
        .get_webview_window(SPEECH_BUBBLE_OVERLAY_WINDOW_LABEL)
        .is_none()
    {
        if let Err(error) = build_speech_bubble_window(app) {
            eprintln!("failed to preload speech bubble overlay: {error}");
        }
    }
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

/// The chat panel lives in its own window instead of resizing the mascot window. It starts at a
/// comfortable default size, then stays user-resizable without affecting the mascot overlay.
#[cfg(feature = "tauri-commands")]
fn build_chat_overlay_window(app: &tauri::AppHandle) -> Result<tauri::WebviewWindow, String> {
    tauri::WebviewWindowBuilder::new(
        app,
        CHAT_OVERLAY_WINDOW_LABEL,
        tauri::WebviewUrl::App("index.html".into()),
    )
    .title("MouseKeeper Chat")
    .inner_size(CHAT_OVERLAY_WIDTH as f64, CHAT_OVERLAY_HEIGHT as f64)
    .min_inner_size(360.0, 280.0)
    .resizable(true)
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
    let left_candidate = (
        RectI {
            x: mouse_pos.x - bubble_w - gap,
            y: align_y,
            w: bubble_w,
            h: bubble_h,
        },
        TailSide::Right,
    );
    /*
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
    ]; */
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

/// Which edge of the bubble carries the tail, i.e. the side facing the mascot. `Bottom` means the
/// bubble sits above the mascot with its tail pointing down at it.
#[cfg(feature = "tauri-commands")]
#[derive(Clone, Copy, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum TailSide {
    Bottom,
    Top,
    Left,
    Right,
}

#[cfg(feature = "tauri-commands")]
#[derive(Clone, serde::Serialize)]
struct SpeechBubblePayload {
    text: String,
    tail_side: TailSide,
}

/// The idle speech bubble lives in its own window instead of resizing the tightly-fit mascot
/// window (its 112x140 size, plus the drag/house-snap foot-ratio math, assumes the sprite fills
/// it). It is created lazily, positioned fresh each time it is shown — the mascot never wanders
/// while a bubble is up — and hidden again once the bubble finishes streaming/fading.
#[cfg(feature = "tauri-commands")]
fn build_speech_bubble_window(app: &tauri::AppHandle) -> Result<tauri::WebviewWindow, String> {
    let window = tauri::WebviewWindowBuilder::new(
        app,
        SPEECH_BUBBLE_OVERLAY_WINDOW_LABEL,
        tauri::WebviewUrl::App("index.html".into()),
    )
    .title("MouseKeeper Speech Bubble")
    .inner_size(SPEECH_BUBBLE_WIDTH as f64, SPEECH_BUBBLE_HEIGHT as f64)
    .resizable(false)
    .decorations(false)
    .transparent(true)
    .shadow(false)
    .always_on_top(true)
    .focusable(false)
    .skip_taskbar(true)
    .visible(false)
    .build()
    .map_err(|error| format!("WINDOW_MISSING: cannot create speech bubble window: {error}"))?;
    // Purely decorative and `always_on_top`: without this, whenever its (best-effort,
    // monitor-clamped) position ends up overlapping the mascot window, it silently steals the
    // pointer events meant for the mascot's drag surface. Ignoring cursor events makes it truly
    // click-through no matter where it lands, so it can never block dragging the mascot.
    let _ = window.set_ignore_cursor_events(true);
    Ok(window)
}

/// Computes where to place the speech bubble so it never overlaps the mascot, moves the window
/// there, then shows it and delivers the text plus which edge the tail should point from. Tries
/// to the mascot's left (tail pointing right at it), then above, then its right, then below (in
/// that order — the mascot never moves while a bubble is up, so only a monitor-edge clamp can
/// force a fallback); whichever clamped candidate overlaps the mascot least wins.
#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub fn show_speech_bubble(app: tauri::AppHandle, text: String) -> Result<(), String> {
    let window = match app.get_webview_window(SPEECH_BUBBLE_OVERLAY_WINDOW_LABEL) {
        Some(window) => window,
        None => build_speech_bubble_window(&app)?,
    };
    let Some(mouse) = app.get_webview_window(OVERLAY_WINDOW_LABEL) else {
        return Err("WINDOW_MISSING: mascot window is not open".to_string());
    };
    let (Ok(mouse_pos), Ok(mouse_size)) = (mouse.outer_position(), mouse.outer_size()) else {
        return Err("WINDOW_MISSING: cannot read mascot window geometry".to_string());
    };

    let bubble_w = SPEECH_BUBBLE_WIDTH;
    let bubble_h = SPEECH_BUBBLE_HEIGHT;
    let gap = SPEECH_BUBBLE_GAP;
    let mouse_w = mouse_size.width as i32;
    let mouse_h = mouse_size.height as i32;
    let mouse_rect = RectI {
        x: mouse_pos.x,
        y: mouse_pos.y,
        w: mouse_w,
        h: mouse_h,
    };
    let monitor_rect = mouse.current_monitor().ok().flatten().map(|monitor| RectI {
        x: monitor.position().x,
        y: monitor.position().y,
        w: monitor.size().width as i32,
        h: monitor.size().height as i32,
    });
    let align_x = mouse_pos.x + (mouse_w - bubble_w) / 2;
    let align_y = mouse_pos.y + (mouse_h - bubble_h) / 2;
    let candidates = [
        (
            RectI {
                x: mouse_pos.x - bubble_w - gap,
                y: align_y,
                w: bubble_w,
                h: bubble_h,
            },
            TailSide::Right,
        ),
        (
            RectI {
                x: align_x,
                y: mouse_pos.y - bubble_h - gap,
                w: bubble_w,
                h: bubble_h,
            },
            TailSide::Bottom,
        ),
        (
            RectI {
                x: mouse_pos.x + mouse_w + gap,
                y: align_y,
                w: bubble_w,
                h: bubble_h,
            },
            TailSide::Left,
        ),
        (
            RectI {
                x: align_x,
                y: mouse_pos.y + mouse_h + gap,
                w: bubble_w,
                h: bubble_h,
            },
            TailSide::Top,
        ),
    ];
    let default_rect = RectI {
        x: mouse_pos.x - bubble_w - gap,
        y: align_y,
        w: bubble_w,
        h: bubble_h,
    };
    // The bubble artwork itself must remain to the mouse's left. Clamp only its vertical
    // position; clamping x could move the visible bubble to the other side near an edge.
    let (left_rect, tail_side) = left_candidate;
    let best_rect = if let Some(monitor) = monitor_rect {
        RectI { y: left_rect.y.clamp(monitor.y, monitor.y + (monitor.h - left_rect.h).max(0)), ..left_rect }
    } else {
        left_rect
    };

    let _ = window.set_position(tauri::PhysicalPosition::new(best_rect.x, best_rect.y));
    window
        .show()
        .map_err(|error| format!("WINDOW_MISSING: cannot show speech bubble: {error}"))?;
    window
        .emit(
            SPEECH_BUBBLE_TEXT_EVENT,
            SpeechBubblePayload { text, tail_side },
        )
        .map_err(|error| format!("EMIT_FAILED: {error}"))?;
    Ok(())
}

/// Hides the bubble window and tells the mascot window it closed, whether this was called by the
/// bubble itself (finished streaming/fading) or by the mascot window forcing an early close (drag
/// start, chat open, mood change).
#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub fn hide_speech_bubble(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(SPEECH_BUBBLE_OVERLAY_WINDOW_LABEL) {
        window
            .hide()
            .map_err(|error| format!("WINDOW_MISSING: cannot hide speech bubble: {error}"))?;
    }
    if let Some(mouse) = app.get_webview_window(OVERLAY_WINDOW_LABEL) {
        let _ = mouse.emit(SPEECH_BUBBLE_CLOSED_EVENT, ());
    }
    Ok(())
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
