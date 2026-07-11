use std::sync::Mutex;

use serde::{Deserialize, Serialize};

pub const OVERLAY_WINDOW_LABEL: &str = "character-overlay";
pub const CHARACTER_EVENT_NAME: &str = "character-event";

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
#[serde(rename_all = "snake_case")]
pub enum CharacterEventKind {
    Idle,
    Analyzing,
    WaitingForApproval,
    Working,
    Success,
    Error,
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
}
