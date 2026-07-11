use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct AgentConnectionStatus {
    pub state: AgentConnectionState,
    pub server_base_url: Option<String>,
    pub device_id: Option<String>,
    pub last_error_code: Option<AgentErrorCode>,
    pub last_error_message: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentConnectionState {
    Unconfigured,
    Offline,
    Connecting,
    Online,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct AgentCommand {
    pub command_id: String,
    pub command_type: String,
    pub room_id: String,
    pub idempotency_key: String,
    pub payload: Value,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AgentErrorCode {
    Unconfigured,
    TransportUnavailable,
}

#[derive(Debug, PartialEq, Eq)]
pub struct AgentError {
    pub code: AgentErrorCode,
    pub message: String,
}

pub trait AgentClient: Send + Sync {
    fn connection_status(&self) -> AgentConnectionStatus;
    fn poll_commands(&self) -> Result<Vec<AgentCommand>, AgentError>;
    fn send_event(&self, event: Value) -> Result<(), AgentError>;
}

#[derive(Debug, Default)]
pub struct UnconfiguredAgentClient;

impl AgentClient for UnconfiguredAgentClient {
    fn connection_status(&self) -> AgentConnectionStatus {
        AgentConnectionStatus {
            state: AgentConnectionState::Unconfigured,
            server_base_url: None,
            device_id: None,
            last_error_code: Some(AgentErrorCode::Unconfigured),
            last_error_message: Some(
                "desktop agent transport is not configured; set up server auth before polling"
                    .to_string(),
            ),
        }
    }

    fn poll_commands(&self) -> Result<Vec<AgentCommand>, AgentError> {
        Err(unconfigured_error(
            "cannot poll commands without configured server transport",
        ))
    }

    fn send_event(&self, _event: Value) -> Result<(), AgentError> {
        Err(unconfigured_error(
            "cannot send desktop agent event without configured server transport",
        ))
    }
}

fn unconfigured_error(message: &str) -> AgentError {
    AgentError {
        code: AgentErrorCode::Unconfigured,
        message: message.to_string(),
    }
}

pub struct AgentRuntime {
    client: Box<dyn AgentClient>,
}

impl Default for AgentRuntime {
    fn default() -> Self {
        Self {
            client: Box::<UnconfiguredAgentClient>::default(),
        }
    }
}

impl AgentRuntime {
    pub fn connection_status(&self) -> AgentConnectionStatus {
        self.client.connection_status()
    }

    pub fn poll_commands(&self) -> Result<Vec<AgentCommand>, AgentError> {
        self.client.poll_commands()
    }

    pub fn send_event(&self, event: Value) -> Result<(), AgentError> {
        self.client.send_event(event)
    }
}

impl std::fmt::Display for AgentError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code.as_str(), self.message)
    }
}

impl AgentErrorCode {
    fn as_str(&self) -> &'static str {
        match self {
            AgentErrorCode::Unconfigured => "UNCONFIGURED",
            AgentErrorCode::TransportUnavailable => "TRANSPORT_UNAVAILABLE",
        }
    }
}

impl std::error::Error for AgentError {}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{AgentClient, AgentConnectionState, AgentErrorCode, UnconfiguredAgentClient};

    #[test]
    fn unconfigured_client_reports_unconfigured_status() {
        let client = UnconfiguredAgentClient;

        let status = client.connection_status();

        assert_eq!(status.state, AgentConnectionState::Unconfigured);
        assert_eq!(status.last_error_code, Some(AgentErrorCode::Unconfigured));
    }

    #[test]
    fn unconfigured_client_refuses_to_poll_commands() {
        let client = UnconfiguredAgentClient;

        let error = client.poll_commands().expect_err("poll must fail");

        assert_eq!(error.code, AgentErrorCode::Unconfigured);
    }

    #[test]
    fn unconfigured_client_refuses_to_send_events() {
        let client = UnconfiguredAgentClient;

        let error = client
            .send_event(json!({ "status": "completed" }))
            .expect_err("send must fail");

        assert_eq!(error.code, AgentErrorCode::Unconfigured);
    }
}
