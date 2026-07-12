use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct BackgroundRuntimeStatus {
    pub state: BackgroundRuntimeState,
    pub last_started_unix_ms: Option<i64>,
    pub last_stopped_unix_ms: Option<i64>,
    pub last_error_message: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BackgroundRuntimeState {
    Stopped,
    Running,
    Suspended,
}

#[derive(Debug)]
pub struct BackgroundRuntime {
    status: Mutex<BackgroundRuntimeStatus>,
}

impl Default for BackgroundRuntime {
    fn default() -> Self {
        Self {
            status: Mutex::new(BackgroundRuntimeStatus {
                state: BackgroundRuntimeState::Stopped,
                last_started_unix_ms: None,
                last_stopped_unix_ms: None,
                last_error_message: None,
            }),
        }
    }
}

impl BackgroundRuntime {
    pub fn status(&self) -> Result<BackgroundRuntimeStatus, String> {
        self.status
            .lock()
            .map(|status| status.clone())
            .map_err(|_| "background runtime lock poisoned".to_string())
    }

    pub fn start_suspended(
        &self,
        reason: impl Into<String>,
    ) -> Result<BackgroundRuntimeStatus, String> {
        let mut status = self
            .status
            .lock()
            .map_err(|_| "background runtime lock poisoned".to_string())?;
        status.state = BackgroundRuntimeState::Suspended;
        status.last_started_unix_ms = Some(unix_ms());
        status.last_error_message = Some(reason.into());
        Ok(status.clone())
    }

    pub fn pause(&self, reason: impl Into<String>) -> Result<BackgroundRuntimeStatus, String> {
        let mut status = self
            .status
            .lock()
            .map_err(|_| "background runtime lock poisoned".to_string())?;
        status.state = BackgroundRuntimeState::Suspended;
        status.last_stopped_unix_ms = Some(unix_ms());
        status.last_error_message = Some(reason.into());
        Ok(status.clone())
    }
}

fn unix_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::{BackgroundRuntime, BackgroundRuntimeState};

    #[test]
    fn starts_suspended_until_transport_exists() {
        let runtime = BackgroundRuntime::default();

        let status = runtime
            .start_suspended("agent transport is not configured")
            .expect("start");

        assert_eq!(status.state, BackgroundRuntimeState::Suspended);
        assert!(status.last_started_unix_ms.is_some());
        assert_eq!(
            status.last_error_message,
            Some("agent transport is not configured".to_string())
        );
    }

    #[test]
    fn pause_records_reason_and_stop_time() {
        let runtime = BackgroundRuntime::default();

        let status = runtime.pause("user paused agent").expect("pause");

        assert_eq!(status.state, BackgroundRuntimeState::Suspended);
        assert!(status.last_stopped_unix_ms.is_some());
    }
}
