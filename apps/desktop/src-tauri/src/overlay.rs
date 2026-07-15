use std::sync::Mutex;

use serde::{Deserialize, Serialize};

pub const OVERLAY_WINDOW_LABEL: &str = "character-overlay";
pub const HOUSE_OVERLAY_WINDOW_LABEL: &str = "house-overlay";
pub const CHAT_OVERLAY_WINDOW_LABEL: &str = "chat-overlay";
pub const SPEECH_BUBBLE_OVERLAY_WINDOW_LABEL: &str = "speech-bubble-overlay";
pub const CHARACTER_EVENT_NAME: &str = "character-event";
pub const SPEECH_BUBBLE_TEXT_EVENT: &str = "speech-bubble:show";
pub const SPEECH_BUBBLE_CLOSED_EVENT: &str = "speech-bubble:closed";

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct OverlayStatus {
    pub state: OverlayState,
    pub window_label: String,
    pub last_event_kind: Option<CharacterEventKind>,
    pub last_error_code: Option<OverlayErrorCode>,
    pub last_error_message: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OverlayState {
    NotReady,
    Hidden,
    Visible,
    Error,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct CharacterEvent {
    pub kind: CharacterEventKind,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub correlation_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CharacterEventKind {
    Idle,
    Connecting,
    Analyzing,
    #[serde(rename = "WAITING_APPROVAL")]
    WaitingForApproval,
    Working,
    Success,
    Error,
    UserWorking,
    Offline,
}

/// A summary of what one background pass did, used to derive the character state to show. Kept as
/// plain counts so the overlay stays decoupled from the command/decision processor report types.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OverlayActivity {
    pub had_error: bool,
    pub executed_item_count: usize,
    pub execution_failed_count: usize,
    pub submitted_proposal_count: usize,
    pub processed_command_count: usize,
}

/// Maps a background pass to the character state to display, ordered by urgency: a failure wins,
/// then freshly executed work, then proposals waiting on a human, then analysis, otherwise idle.
pub fn character_event_for(activity: &OverlayActivity) -> CharacterEvent {
    let kind = if activity.had_error || activity.execution_failed_count > 0 {
        CharacterEventKind::Error
    } else if activity.executed_item_count > 0 {
        CharacterEventKind::Success
    } else if activity.submitted_proposal_count > 0 {
        CharacterEventKind::WaitingForApproval
    } else if activity.processed_command_count > 0 {
        CharacterEventKind::Analyzing
    } else {
        CharacterEventKind::Idle
    };
    CharacterEvent {
        kind,
        message: None,
        correlation_id: None,
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OverlayErrorCode {
    NotReady,
    WindowMissing,
    EmitFailed,
}

#[derive(Debug, PartialEq, Eq)]
pub struct OverlayError {
    pub code: OverlayErrorCode,
    pub message: String,
}

#[derive(Debug)]
pub struct OverlayRuntime {
    inner: Mutex<OverlayRuntimeState>,
}

#[derive(Debug)]
struct OverlayRuntimeState {
    state: OverlayState,
    last_event_kind: Option<CharacterEventKind>,
    last_error: Option<OverlayError>,
}

impl Default for OverlayRuntime {
    fn default() -> Self {
        Self {
            inner: Mutex::new(OverlayRuntimeState {
                state: OverlayState::NotReady,
                last_event_kind: None,
                last_error: Some(OverlayError {
                    code: OverlayErrorCode::NotReady,
                    message: "overlay window is not configured yet".to_string(),
                }),
            }),
        }
    }
}

impl OverlayRuntime {
    pub fn status(&self) -> Result<OverlayStatus, OverlayError> {
        let inner = self.inner.lock().map_err(|_| OverlayError {
            code: OverlayErrorCode::EmitFailed,
            message: "overlay runtime lock poisoned".to_string(),
        })?;

        Ok(OverlayStatus {
            state: inner.state.clone(),
            window_label: OVERLAY_WINDOW_LABEL.to_string(),
            last_event_kind: inner.last_event_kind.clone(),
            last_error_code: inner.last_error.as_ref().map(|error| error.code.clone()),
            last_error_message: inner.last_error.as_ref().map(|error| error.message.clone()),
        })
    }

    pub fn mark_event_accepted(&self, event: &CharacterEvent) -> Result<(), OverlayError> {
        let mut inner = self.inner.lock().map_err(|_| OverlayError {
            code: OverlayErrorCode::EmitFailed,
            message: "overlay runtime lock poisoned".to_string(),
        })?;

        inner.last_event_kind = Some(event.kind.clone());
        inner.last_error = None;
        Ok(())
    }

    /// Records that the overlay window is created and shown. Called by the shell after the actual
    /// Tauri window is built/shown; the overlay runtime itself never touches the file engine.
    pub fn mark_visible(&self) -> Result<OverlayStatus, OverlayError> {
        self.set_visibility(OverlayState::Visible)
    }

    /// Records that the overlay window is hidden (but still alive). The character UI keeps its
    /// state; only visibility changes.
    pub fn mark_hidden(&self) -> Result<OverlayStatus, OverlayError> {
        self.set_visibility(OverlayState::Hidden)
    }

    fn set_visibility(&self, state: OverlayState) -> Result<OverlayStatus, OverlayError> {
        {
            let mut inner = self.inner.lock().map_err(|_| OverlayError {
                code: OverlayErrorCode::EmitFailed,
                message: "overlay runtime lock poisoned".to_string(),
            })?;
            inner.state = state;
            inner.last_error = None;
        }
        self.status()
    }

    pub fn mark_not_ready(&self, message: impl Into<String>) -> OverlayError {
        let error = OverlayError {
            code: OverlayErrorCode::NotReady,
            message: message.into(),
        };

        if let Ok(mut inner) = self.inner.lock() {
            inner.state = OverlayState::NotReady;
            inner.last_error = Some(OverlayError {
                code: error.code.clone(),
                message: error.message.clone(),
            });
        }

        error
    }
}

impl std::fmt::Display for OverlayError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code.as_str(), self.message)
    }
}

impl OverlayErrorCode {
    fn as_str(&self) -> &'static str {
        match self {
            OverlayErrorCode::NotReady => "NOT_READY",
            OverlayErrorCode::WindowMissing => "WINDOW_MISSING",
            OverlayErrorCode::EmitFailed => "EMIT_FAILED",
        }
    }
}

impl std::error::Error for OverlayError {}

#[cfg(test)]
mod tests {
    use super::{
        CharacterEvent, CharacterEventKind, OverlayErrorCode, OverlayRuntime, OverlayState,
    };

    #[test]
    fn overlay_defaults_to_not_ready() {
        let runtime = OverlayRuntime::default();

        let status = runtime.status().expect("status");

        assert_eq!(status.state, OverlayState::NotReady);
        assert_eq!(status.last_error_code, Some(OverlayErrorCode::NotReady));
    }

    #[test]
    fn mark_not_ready_returns_explicit_error() {
        let runtime = OverlayRuntime::default();

        let error = runtime.mark_not_ready("overlay UI has not been created");

        assert_eq!(error.code, OverlayErrorCode::NotReady);
    }

    #[test]
    fn accepted_event_records_last_event_kind() {
        let runtime = OverlayRuntime::default();
        let event = CharacterEvent {
            kind: CharacterEventKind::Working,
            message: None,
            correlation_id: Some("command-1".to_string()),
        };

        runtime.mark_event_accepted(&event).expect("accept event");
        let status = runtime.status().expect("status");

        assert_eq!(status.last_event_kind, Some(CharacterEventKind::Working));
        assert_eq!(status.last_error_code, None);
    }

    #[test]
    fn character_states_match_the_shared_wire_contract() {
        for (kind, wire_value) in [
            (CharacterEventKind::Idle, "IDLE"),
            (CharacterEventKind::Connecting, "CONNECTING"),
            (CharacterEventKind::Analyzing, "ANALYZING"),
            (CharacterEventKind::WaitingForApproval, "WAITING_APPROVAL"),
            (CharacterEventKind::Working, "WORKING"),
            (CharacterEventKind::Success, "SUCCESS"),
            (CharacterEventKind::Error, "ERROR"),
            (CharacterEventKind::UserWorking, "USER_WORKING"),
            (CharacterEventKind::Offline, "OFFLINE"),
        ] {
            let event = CharacterEvent {
                kind,
                message: None,
                correlation_id: None,
            };
            let json = serde_json::to_string(&event).expect("serialize character state");
            assert!(json.contains(&format!(r#""kind":"{wire_value}""#)));
            assert_eq!(
                serde_json::from_str::<CharacterEvent>(&json).expect("deserialize character state"),
                event
            );
        }
    }

    #[test]
    fn visibility_transitions_clear_the_not_ready_error() {
        let runtime = OverlayRuntime::default();

        let shown = runtime.mark_visible().expect("visible");
        assert_eq!(shown.state, OverlayState::Visible);
        assert_eq!(shown.last_error_code, None);

        let hidden = runtime.mark_hidden().expect("hidden");
        assert_eq!(hidden.state, OverlayState::Hidden);
    }

    #[test]
    fn character_event_mapping_is_ordered_by_urgency() {
        use super::{character_event_for, OverlayActivity};

        // Failure beats everything else in the same pass.
        let failing = character_event_for(&OverlayActivity {
            execution_failed_count: 1,
            executed_item_count: 3,
            ..OverlayActivity::default()
        });
        assert_eq!(failing.kind, CharacterEventKind::Error);

        assert_eq!(
            character_event_for(&OverlayActivity {
                executed_item_count: 2,
                ..OverlayActivity::default()
            })
            .kind,
            CharacterEventKind::Success
        );
        assert_eq!(
            character_event_for(&OverlayActivity {
                submitted_proposal_count: 1,
                ..OverlayActivity::default()
            })
            .kind,
            CharacterEventKind::WaitingForApproval
        );
        assert_eq!(
            character_event_for(&OverlayActivity {
                processed_command_count: 1,
                ..OverlayActivity::default()
            })
            .kind,
            CharacterEventKind::Analyzing
        );
        assert_eq!(
            character_event_for(&OverlayActivity::default()).kind,
            CharacterEventKind::Idle
        );
    }
}
