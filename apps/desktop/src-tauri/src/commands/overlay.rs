use crate::overlay::{CharacterEvent, OverlayRuntime, OverlayStatus};

#[cfg(feature = "tauri-commands")]
use crate::overlay::{CHARACTER_EVENT_NAME, OVERLAY_WINDOW_LABEL};

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

fn get_overlay_status_impl(runtime: &OverlayRuntime) -> Result<OverlayStatus, String> {
    runtime.status().map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use crate::overlay::{CharacterEvent, CharacterEventKind, OverlayRuntime, OverlayState};

    use super::{emit_character_event, get_overlay_status};

    #[test]
    fn status_reports_overlay_not_ready() {
        let runtime = OverlayRuntime::default();

        let status = get_overlay_status(&runtime).expect("status");

        assert_eq!(status.state, OverlayState::NotReady);
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
