use serde_json::Value;

use crate::agent::{AgentCommand, AgentConnectionStatus, AgentRuntime};

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub fn get_agent_connection_status(
    runtime: tauri::State<'_, AgentRuntime>,
) -> Result<AgentConnectionStatus, String> {
    get_agent_connection_status_impl(&runtime)
}

#[cfg(not(feature = "tauri-commands"))]
pub fn get_agent_connection_status(
    runtime: &AgentRuntime,
) -> Result<AgentConnectionStatus, String> {
    get_agent_connection_status_impl(runtime)
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub fn poll_agent_commands(
    runtime: tauri::State<'_, AgentRuntime>,
) -> Result<Vec<AgentCommand>, String> {
    poll_agent_commands_impl(&runtime)
}

#[cfg(not(feature = "tauri-commands"))]
pub fn poll_agent_commands(runtime: &AgentRuntime) -> Result<Vec<AgentCommand>, String> {
    poll_agent_commands_impl(runtime)
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub fn send_agent_event(
    event: Value,
    runtime: tauri::State<'_, AgentRuntime>,
) -> Result<(), String> {
    send_agent_event_impl(&runtime, event)
}

#[cfg(not(feature = "tauri-commands"))]
pub fn send_agent_event(runtime: &AgentRuntime, event: Value) -> Result<(), String> {
    send_agent_event_impl(runtime, event)
}

fn get_agent_connection_status_impl(
    runtime: &AgentRuntime,
) -> Result<AgentConnectionStatus, String> {
    Ok(runtime.connection_status())
}

fn poll_agent_commands_impl(runtime: &AgentRuntime) -> Result<Vec<AgentCommand>, String> {
    runtime.poll_commands().map_err(|error| error.to_string())
}

fn send_agent_event_impl(runtime: &AgentRuntime, event: Value) -> Result<(), String> {
    runtime.send_event(event).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::agent::{AgentConnectionState, AgentRuntime};

    use super::{get_agent_connection_status, poll_agent_commands, send_agent_event};

    #[test]
    fn status_reports_unconfigured_without_faking_online() {
        let runtime = AgentRuntime::default();

        let status = get_agent_connection_status(&runtime).expect("status");

        assert_eq!(status.state, AgentConnectionState::Unconfigured);
    }

    #[test]
    fn polling_commands_returns_unconfigured_error() {
        let runtime = AgentRuntime::default();

        let error = poll_agent_commands(&runtime).expect_err("poll fails");

        assert!(error.contains("UNCONFIGURED"));
    }

    #[test]
    fn sending_events_returns_unconfigured_error() {
        let runtime = AgentRuntime::default();

        let error =
            send_agent_event(&runtime, json!({ "status": "completed" })).expect_err("send fails");

        assert!(error.contains("UNCONFIGURED"));
    }
}
