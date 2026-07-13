use crate::agent::{
    AgentCommand, AgentConnectionStatus, AgentRoomSync, AgentRuntime, HeartbeatResult,
    PairingSession, PairingStatus, SyncEvent,
};
use crate::command_processor::{process_pending_commands, CommandProcessingReport};
use crate::execution_processor::{process_pending_decisions, DecisionProcessingReport};
use crate::file_browse_processor::{
    process_pending_file_browse_requests, FileBrowseProcessingReport,
};
use crate::file_transfer_processor::{
    process_pending_file_transfers, FileTransferProcessingReport,
};
use crate::storage::agent_sync::AgentSyncStore;
use crate::storage::managed_roots::ManagedRootStore;
use crate::storage::outbox::OutboxStore;

#[derive(Clone, Debug, serde::Serialize, PartialEq)]
pub struct SyncReplay {
    pub previous_cursor: u64,
    pub next_cursor: u64,
    pub events: Vec<SyncEvent>,
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub fn get_agent_connection_status(
    runtime: tauri::State<'_, AgentRuntime>,
) -> Result<AgentConnectionStatus, String> {
    Ok(runtime.connection_status())
}

#[cfg(not(feature = "tauri-commands"))]
pub fn get_agent_connection_status(
    runtime: &AgentRuntime,
) -> Result<AgentConnectionStatus, String> {
    Ok(runtime.connection_status())
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub async fn start_agent_pairing(
    device_name: String,
    runtime: tauri::State<'_, AgentRuntime>,
) -> Result<PairingSession, String> {
    runtime
        .start_pairing(device_name)
        .await
        .map_err(|error| error.to_string())
}

#[cfg(not(feature = "tauri-commands"))]
pub async fn start_agent_pairing(
    runtime: &AgentRuntime,
    device_name: String,
) -> Result<PairingSession, String> {
    runtime
        .start_pairing(device_name)
        .await
        .map_err(|error| error.to_string())
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub async fn poll_agent_pairing(
    session_id: String,
    desktop_nonce: String,
    runtime: tauri::State<'_, AgentRuntime>,
) -> Result<PairingStatus, String> {
    runtime
        .poll_pairing(session_id, desktop_nonce)
        .await
        .map_err(|error| error.to_string())
}

#[cfg(not(feature = "tauri-commands"))]
pub async fn poll_agent_pairing(
    runtime: &AgentRuntime,
    session_id: String,
    desktop_nonce: String,
) -> Result<PairingStatus, String> {
    runtime
        .poll_pairing(session_id, desktop_nonce)
        .await
        .map_err(|error| error.to_string())
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub async fn send_agent_heartbeat(
    presence: String,
    runtime: tauri::State<'_, AgentRuntime>,
) -> Result<HeartbeatResult, String> {
    runtime
        .heartbeat(presence)
        .await
        .map_err(|error| error.to_string())
}

#[cfg(not(feature = "tauri-commands"))]
pub async fn send_agent_heartbeat(
    runtime: &AgentRuntime,
    presence: String,
) -> Result<HeartbeatResult, String> {
    runtime
        .heartbeat(presence)
        .await
        .map_err(|error| error.to_string())
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub async fn poll_agent_commands(
    runtime: tauri::State<'_, AgentRuntime>,
) -> Result<Vec<AgentCommand>, String> {
    runtime
        .poll_commands()
        .await
        .map_err(|error| error.to_string())
}

#[cfg(not(feature = "tauri-commands"))]
pub async fn poll_agent_commands(runtime: &AgentRuntime) -> Result<Vec<AgentCommand>, String> {
    runtime
        .poll_commands()
        .await
        .map_err(|error| error.to_string())
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub async fn process_agent_commands(
    runtime: tauri::State<'_, AgentRuntime>,
    roots: tauri::State<'_, ManagedRootStore>,
    outbox: tauri::State<'_, OutboxStore>,
) -> Result<CommandProcessingReport, String> {
    process_pending_commands(&runtime, &roots, &outbox).await
}

#[cfg(not(feature = "tauri-commands"))]
pub async fn process_agent_commands(
    runtime: &AgentRuntime,
    roots: &ManagedRootStore,
    outbox: &OutboxStore,
) -> Result<CommandProcessingReport, String> {
    process_pending_commands(runtime, roots, outbox).await
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub async fn process_agent_decisions(
    runtime: tauri::State<'_, AgentRuntime>,
    roots: tauri::State<'_, ManagedRootStore>,
    outbox: tauri::State<'_, OutboxStore>,
) -> Result<DecisionProcessingReport, String> {
    process_pending_decisions(&runtime, &roots, &outbox).await
}

#[cfg(not(feature = "tauri-commands"))]
pub async fn process_agent_decisions(
    runtime: &AgentRuntime,
    roots: &ManagedRootStore,
    outbox: &OutboxStore,
) -> Result<DecisionProcessingReport, String> {
    process_pending_decisions(runtime, roots, outbox).await
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub async fn process_agent_file_browse_requests(
    runtime: tauri::State<'_, AgentRuntime>,
    roots: tauri::State<'_, ManagedRootStore>,
) -> Result<FileBrowseProcessingReport, String> {
    process_pending_file_browse_requests(&runtime, &roots).await
}

#[cfg(not(feature = "tauri-commands"))]
pub async fn process_agent_file_browse_requests(
    runtime: &AgentRuntime,
    roots: &ManagedRootStore,
) -> Result<FileBrowseProcessingReport, String> {
    process_pending_file_browse_requests(runtime, roots).await
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub async fn process_agent_file_transfers(
    runtime: tauri::State<'_, AgentRuntime>,
    roots: tauri::State<'_, ManagedRootStore>,
    outbox: tauri::State<'_, OutboxStore>,
) -> Result<FileTransferProcessingReport, String> {
    process_pending_file_transfers(&runtime, &roots, &outbox).await
}

#[cfg(not(feature = "tauri-commands"))]
pub async fn process_agent_file_transfers(
    runtime: &AgentRuntime,
    roots: &ManagedRootStore,
    outbox: &OutboxStore,
) -> Result<FileTransferProcessingReport, String> {
    process_pending_file_transfers(runtime, roots, outbox).await
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub async fn flush_agent_outbox(
    runtime: tauri::State<'_, AgentRuntime>,
    outbox: tauri::State<'_, OutboxStore>,
) -> Result<crate::outbox_processor::OutboxFlushReport, String> {
    crate::outbox_processor::flush_outbox(&runtime, &outbox).await
}

#[cfg(not(feature = "tauri-commands"))]
pub async fn flush_agent_outbox(
    runtime: &AgentRuntime,
    outbox: &OutboxStore,
) -> Result<crate::outbox_processor::OutboxFlushReport, String> {
    crate::outbox_processor::flush_outbox(runtime, outbox).await
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub async fn ensure_agent_room(
    root_id: String,
    display_name: String,
    runtime: tauri::State<'_, AgentRuntime>,
) -> Result<AgentRoomSync, String> {
    runtime
        .ensure_room_for_root(root_id, display_name)
        .await
        .map_err(|error| error.to_string())
}

#[cfg(not(feature = "tauri-commands"))]
pub async fn ensure_agent_room(
    runtime: &AgentRuntime,
    root_id: String,
    display_name: String,
) -> Result<AgentRoomSync, String> {
    runtime
        .ensure_room_for_root(root_id, display_name)
        .await
        .map_err(|error| error.to_string())
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub async fn replay_agent_events(
    runtime: tauri::State<'_, AgentRuntime>,
    sync_store: tauri::State<'_, AgentSyncStore>,
) -> Result<SyncReplay, String> {
    replay_agent_events_impl(&runtime, &sync_store).await
}

#[cfg(not(feature = "tauri-commands"))]
pub async fn replay_agent_events(
    runtime: &AgentRuntime,
    sync_store: &AgentSyncStore,
) -> Result<SyncReplay, String> {
    replay_agent_events_impl(runtime, sync_store).await
}

async fn replay_agent_events_impl(
    runtime: &AgentRuntime,
    sync_store: &AgentSyncStore,
) -> Result<SyncReplay, String> {
    let device_id = runtime
        .connection_status()
        .device_id
        .ok_or_else(|| "UNCONFIGURED: desktop device pairing is required".to_string())?;
    let previous_cursor = sync_store.cursor(&device_id)?;
    let events = runtime
        .replay_events(previous_cursor, 100)
        .await
        .map_err(|error| error.to_string())?;
    let next_cursor = events
        .last()
        .map(|event| event.sequence)
        .unwrap_or(previous_cursor);
    if next_cursor > previous_cursor {
        sync_store.advance(&device_id, next_cursor).await?;
    }
    Ok(SyncReplay {
        previous_cursor,
        next_cursor,
        events,
    })
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub async fn update_agent_command_status(
    command_id: String,
    status: String,
    runtime: tauri::State<'_, AgentRuntime>,
) -> Result<AgentCommand, String> {
    runtime
        .update_command_status(command_id, status)
        .await
        .map_err(|error| error.to_string())
}

#[cfg(not(feature = "tauri-commands"))]
pub async fn update_agent_command_status(
    runtime: &AgentRuntime,
    command_id: String,
    status: String,
) -> Result<AgentCommand, String> {
    runtime
        .update_command_status(command_id, status)
        .await
        .map_err(|error| error.to_string())
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub async fn forget_agent_device(
    runtime: tauri::State<'_, AgentRuntime>,
    sync_store: tauri::State<'_, AgentSyncStore>,
) -> Result<AgentConnectionStatus, String> {
    forget_agent_device_impl(&runtime, &sync_store).await
}

#[cfg(not(feature = "tauri-commands"))]
pub async fn forget_agent_device(
    runtime: &AgentRuntime,
    sync_store: &AgentSyncStore,
) -> Result<AgentConnectionStatus, String> {
    forget_agent_device_impl(runtime, sync_store).await
}

async fn forget_agent_device_impl(
    runtime: &AgentRuntime,
    sync_store: &AgentSyncStore,
) -> Result<AgentConnectionStatus, String> {
    let device_id = runtime.connection_status().device_id;
    let status = runtime.forget_device().map_err(|error| error.to_string())?;
    if let Some(device_id) = device_id {
        sync_store.clear_device(&device_id).await?;
    }
    Ok(status)
}

// These tests exercise the `not(feature = "tauri-commands")` variants above, which take a plain
// `&AgentRuntime` so they are callable without a live Tauri `State`. The `tauri-commands` variants
// need a running app to construct their `State` argument and are not unit-testable this way, so
// this module only compiles for the CLI/no-tauri-commands build.
#[cfg(all(test, not(feature = "tauri-commands")))]
mod tests {
    use crate::agent::{AgentConnectionState, AgentRuntime};

    use super::{get_agent_connection_status, poll_agent_commands, send_agent_heartbeat};

    #[test]
    fn status_reports_unconfigured_without_faking_online() {
        let runtime = AgentRuntime::default();
        let status = get_agent_connection_status(&runtime).expect("status");

        if status.server_base_url.is_none() {
            assert_eq!(status.state, AgentConnectionState::Unconfigured);
        }
    }

    #[tokio::test]
    async fn polling_commands_refuses_an_unpaired_runtime() {
        let runtime = AgentRuntime::default();
        if runtime.connection_status().device_id.is_none() {
            let error = poll_agent_commands(&runtime).await.expect_err("poll fails");
            assert!(error.contains("UNCONFIGURED"));
        }
    }

    #[tokio::test]
    async fn heartbeat_rejects_invalid_presence() {
        let runtime = AgentRuntime::default();
        let error = send_agent_heartbeat(&runtime, "OFFLINE".to_string())
            .await
            .expect_err("invalid presence");

        assert!(error.contains("VALIDATION_FAILED"));
    }
}
