use crate::agent::{
    AgentCommand, AgentConnectionStatus, AgentRoomSnapshot, AgentRoomSync, AgentRuntime,
    HeartbeatResult, PairingSession, PairingStatus, SyncEvent,
};
use crate::cleanliness::{calculate_cleanliness_snapshot, CleanlinessSnapshot};
use crate::command_processor::{process_pending_commands, CommandProcessingReport};
use crate::execution_processor::{process_pending_decisions, DecisionProcessingReport};
use crate::file_browse_processor::{
    process_pending_file_browse_requests, FileBrowseProcessingReport,
};
use crate::file_transfer_processor::{
    process_pending_file_transfers, FileTransferProcessingReport,
};
use crate::storage::agent_sync::AgentSyncStore;
use crate::storage::managed_roots::{ManagedRootStore, RoomBindingStatus};
use crate::storage::outbox::OutboxStore;
use crate::storage::watchers::WatcherStore;
use file_engine_cli::journal::{read_operation_history, JournalStatus};

#[derive(Clone, Debug, serde::Serialize, PartialEq)]
pub struct SyncReplay {
    pub previous_cursor: u64,
    pub next_cursor: u64,
    pub events: Vec<SyncEvent>,
}

#[derive(Clone, Debug, serde::Serialize, PartialEq)]
pub struct CleanlinessSnapshotSyncReport {
    pub root_id: String,
    pub room_id: String,
    pub room_created: bool,
    pub snapshot: CleanlinessSnapshot,
    pub server_snapshot: AgentRoomSnapshot,
}

#[derive(Clone, Debug, serde::Serialize, PartialEq, Eq)]
pub struct RoomDisconnectPreflight {
    pub root_id: String,
    pub room_id: String,
    pub blocking_reasons: Vec<String>,
    pub undoable_operation_count: usize,
    pub requires_confirmation: bool,
}

#[derive(Clone, Debug, serde::Serialize, PartialEq, Eq)]
pub struct RoomDisconnectReport {
    pub root_id: String,
    pub room_id: String,
    pub watcher_stopped: bool,
    pub index_cleared: bool,
    pub undoable_operation_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LocalRoomDetachReport {
    pub root_id: String,
    pub room_id: String,
    pub watcher_stopped: bool,
    pub index_cleared: bool,
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
    roots: tauri::State<'_, ManagedRootStore>,
    watchers: tauri::State<'_, WatcherStore>,
    app: tauri::AppHandle,
) -> Result<AgentRoomSync, String> {
    preflight_agent_room_binding(&roots, &root_id)?;
    let room = runtime
        .ensure_room_for_root(root_id, display_name)
        .await
        .map_err(|error| error.to_string())?;
    activate_agent_room_binding(&roots, &room)?;
    crate::watcher_lifecycle::start_root_watcher(room.root_id.clone(), app, &roots, &watchers)?;
    Ok(room)
}

#[cfg(not(feature = "tauri-commands"))]
pub async fn ensure_agent_room(
    runtime: &AgentRuntime,
    roots: &ManagedRootStore,
    root_id: String,
    display_name: String,
) -> Result<AgentRoomSync, String> {
    preflight_agent_room_binding(roots, &root_id)?;
    let room = runtime
        .ensure_room_for_root(root_id, display_name)
        .await
        .map_err(|error| error.to_string())?;
    activate_agent_room_binding(roots, &room)?;
    Ok(room)
}

#[cfg(all(test, not(feature = "tauri-commands")))]
fn prepare_agent_room_binding(
    roots: &ManagedRootStore,
    room: &AgentRoomSync,
) -> Result<(), String> {
    preflight_agent_room_binding(roots, &room.root_id)?;
    activate_agent_room_binding(roots, room)
}

fn preflight_agent_room_binding(roots: &ManagedRootStore, root_id: &str) -> Result<(), String> {
    let root = roots.get(root_id)?;
    if !root.enabled {
        return Err(format!("managed root is disabled: {root_id}"));
    }
    // Rebuild before making the room active. A search can never observe an active reconnected
    // binding backed by the intentionally-cleared index left by the previous detach. Running this
    // preflight before the server mutation also prevents creating a cloud room for a missing root.
    file_engine_cli::file_index::reindex_root(&root.root).map_err(|error| error.to_string())?;
    Ok(())
}

fn activate_agent_room_binding(
    roots: &ManagedRootStore,
    room: &AgentRoomSync,
) -> Result<(), String> {
    roots.bind_room(&room.root_id, room.room_id.clone())?;
    Ok(())
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub async fn submit_cleanliness_snapshot(
    root_id: String,
    runtime: tauri::State<'_, AgentRuntime>,
    roots: tauri::State<'_, ManagedRootStore>,
) -> Result<CleanlinessSnapshotSyncReport, String> {
    submit_cleanliness_snapshot_impl(root_id, &runtime, &roots).await
}

#[cfg(not(feature = "tauri-commands"))]
pub async fn submit_cleanliness_snapshot(
    root_id: String,
    runtime: &AgentRuntime,
    roots: &ManagedRootStore,
) -> Result<CleanlinessSnapshotSyncReport, String> {
    submit_cleanliness_snapshot_impl(root_id, runtime, roots).await
}

async fn submit_cleanliness_snapshot_impl(
    root_id: String,
    runtime: &AgentRuntime,
    roots: &ManagedRootStore,
) -> Result<CleanlinessSnapshotSyncReport, String> {
    let managed_root = roots.get(&root_id)?;
    if !managed_root.enabled {
        return Err(format!("managed root is disabled: {root_id}"));
    }
    if managed_root.room_binding_status == RoomBindingStatus::Detached {
        return Err(
            "ROOM_DETACHED: reconnect this managed root explicitly before syncing cleanliness"
                .to_string(),
        );
    }

    let snapshot = calculate_cleanliness_snapshot(&managed_root.root)?;
    let room = runtime
        .ensure_room_for_root(
            managed_root.root_id.clone(),
            managed_root.display_name.clone(),
        )
        .await
        .map_err(|error| error.to_string())?;
    roots.bind_room(&managed_root.root_id, room.room_id.clone())?;
    let server_snapshot = runtime
        .submit_room_snapshot(room.room_id.clone(), snapshot.clone())
        .await
        .map_err(|error| error.to_string())?;

    Ok(CleanlinessSnapshotSyncReport {
        root_id: managed_root.root_id,
        room_id: room.room_id,
        room_created: room.created,
        snapshot,
        server_snapshot,
    })
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub async fn replay_agent_events(
    runtime: tauri::State<'_, AgentRuntime>,
    sync_store: tauri::State<'_, AgentSyncStore>,
    roots: tauri::State<'_, ManagedRootStore>,
    watchers: tauri::State<'_, WatcherStore>,
    app: tauri::AppHandle,
) -> Result<SyncReplay, String> {
    let replay = replay_agent_events_impl(&runtime, &sync_store, &roots, &watchers).await?;
    use tauri::Emitter;
    for event in &replay.events {
        if event.event_type == "room.removed" {
            let _ = app.emit("managed-root-binding-changed", event.room_id.clone());
        }
        if event.event_type == "device.revoked" {
            let _ = app.emit("desktop-device-revoked", event.device_id.clone());
        }
    }
    Ok(replay)
}

#[cfg(not(feature = "tauri-commands"))]
pub async fn replay_agent_events(
    runtime: &AgentRuntime,
    sync_store: &AgentSyncStore,
    roots: &ManagedRootStore,
    watchers: &WatcherStore,
) -> Result<SyncReplay, String> {
    replay_agent_events_impl(runtime, sync_store, roots, watchers).await
}

async fn replay_agent_events_impl(
    runtime: &AgentRuntime,
    sync_store: &AgentSyncStore,
    roots: &ManagedRootStore,
    watchers: &WatcherStore,
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
    let mut device_revoked = false;
    for event in &events {
        if event.event_type == "room.removed" {
            let room_id = event.room_id.as_deref().or_else(|| {
                (event.aggregate_type == "room").then_some(event.aggregate_id.as_str())
            });
            if let Some(room_id) = room_id {
                let _ = apply_local_room_detached(room_id, roots, watchers)?;
            }
        }
        if event.event_type == "device.revoked"
            && (event.device_id.as_deref() == Some(device_id.as_str())
                || (event.aggregate_type == "device" && event.aggregate_id == device_id))
        {
            apply_local_device_revoked(runtime, sync_store, roots, watchers).await?;
            device_revoked = true;
            break;
        }
    }
    let next_cursor = events
        .last()
        .map(|event| event.sequence)
        .unwrap_or(previous_cursor);
    if !device_revoked && next_cursor > previous_cursor {
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
pub fn preflight_agent_room_disconnect(
    root_id: String,
    roots: tauri::State<'_, ManagedRootStore>,
) -> Result<RoomDisconnectPreflight, String> {
    preflight_agent_room_disconnect_impl(root_id, &roots)
}

#[cfg(not(feature = "tauri-commands"))]
pub fn preflight_agent_room_disconnect(
    roots: &ManagedRootStore,
    root_id: String,
) -> Result<RoomDisconnectPreflight, String> {
    preflight_agent_room_disconnect_impl(root_id, roots)
}

fn preflight_agent_room_disconnect_impl(
    root_id: String,
    roots: &ManagedRootStore,
) -> Result<RoomDisconnectPreflight, String> {
    let root = roots.get(&root_id)?;
    let room_id = root
        .room_id
        .clone()
        .or_else(|| root.detached_room_id.clone())
        .ok_or_else(|| {
            "ROOM_NOT_BOUND: managed root is not connected to a mobile room".to_string()
        })?;
    if root.room_binding_status == RoomBindingStatus::Detached {
        return Ok(RoomDisconnectPreflight {
            root_id,
            room_id,
            blocking_reasons: Vec::new(),
            undoable_operation_count: 0,
            requires_confirmation: false,
        });
    }
    let history = read_operation_history(&root.root).map_err(|error| error.to_string())?;
    let mut blocking_reasons = Vec::new();
    if history.corruption.is_some() {
        blocking_reasons.push(
            "operation journal is corrupted; recover or preserve it before disconnecting"
                .to_string(),
        );
    }
    let incomplete = history
        .operations
        .iter()
        .filter(|operation| {
            matches!(
                operation.latest_status,
                JournalStatus::Planned | JournalStatus::UndoPlanned
            )
        })
        .count();
    if incomplete > 0 {
        blocking_reasons.push(format!(
            "{incomplete} journal operation(s) are incomplete; finish recovery before disconnecting"
        ));
    }
    let undoable_operation_count = history
        .operations
        .iter()
        .filter(|operation| operation.can_undo)
        .count();
    Ok(RoomDisconnectPreflight {
        root_id,
        room_id,
        requires_confirmation: undoable_operation_count > 0,
        blocking_reasons,
        undoable_operation_count,
    })
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub async fn disconnect_agent_room(
    root_id: String,
    idempotency_key: String,
    acknowledge_undoable: bool,
    runtime: tauri::State<'_, AgentRuntime>,
    roots: tauri::State<'_, ManagedRootStore>,
    watchers: tauri::State<'_, WatcherStore>,
) -> Result<RoomDisconnectReport, String> {
    disconnect_agent_room_impl(
        root_id,
        idempotency_key,
        acknowledge_undoable,
        &runtime,
        &roots,
        &watchers,
    )
    .await
}

#[cfg(not(feature = "tauri-commands"))]
pub async fn disconnect_agent_room(
    runtime: &AgentRuntime,
    roots: &ManagedRootStore,
    watchers: &WatcherStore,
    root_id: String,
    idempotency_key: String,
    acknowledge_undoable: bool,
) -> Result<RoomDisconnectReport, String> {
    disconnect_agent_room_impl(
        root_id,
        idempotency_key,
        acknowledge_undoable,
        runtime,
        roots,
        watchers,
    )
    .await
}

async fn disconnect_agent_room_impl(
    root_id: String,
    idempotency_key: String,
    acknowledge_undoable: bool,
    runtime: &AgentRuntime,
    roots: &ManagedRootStore,
    watchers: &WatcherStore,
) -> Result<RoomDisconnectReport, String> {
    let preflight = preflight_agent_room_disconnect_impl(root_id, roots)?;
    if roots.get(&preflight.root_id)?.room_binding_status == RoomBindingStatus::Detached {
        let detached = apply_local_room_detached(&preflight.room_id, roots, watchers)?
            .ok_or_else(|| "detached room tombstone disappeared during cleanup".to_string())?;
        return Ok(RoomDisconnectReport {
            root_id: detached.root_id,
            room_id: detached.room_id,
            watcher_stopped: detached.watcher_stopped,
            index_cleared: detached.index_cleared,
            undoable_operation_count: 0,
        });
    }
    if !preflight.blocking_reasons.is_empty() {
        return Err(format!(
            "ROOM_DISCONNECT_BLOCKED: {}",
            preflight.blocking_reasons.join("; ")
        ));
    }
    if preflight.requires_confirmation && !acknowledge_undoable {
        return Err(format!(
            "ROOM_DISCONNECT_REQUIRES_CONFIRMATION: {} undoable operation(s) will remain available locally",
            preflight.undoable_operation_count
        ));
    }

    runtime
        .disconnect_room(preflight.room_id.clone(), idempotency_key)
        .await
        .map_err(|error| error.to_string())?;
    let detached = apply_local_room_detached(&preflight.room_id, roots, watchers)?
        .ok_or_else(|| "room binding disappeared during disconnect".to_string())?;
    Ok(RoomDisconnectReport {
        root_id: detached.root_id,
        room_id: detached.room_id,
        watcher_stopped: detached.watcher_stopped,
        index_cleared: detached.index_cleared,
        undoable_operation_count: preflight.undoable_operation_count,
    })
}

pub(crate) fn apply_local_room_detached(
    room_id: &str,
    roots: &ManagedRootStore,
    watchers: &WatcherStore,
) -> Result<Option<LocalRoomDetachReport>, String> {
    let Some(root) = roots.find_by_room(room_id)? else {
        return Ok(None);
    };
    if root.room_binding_status != RoomBindingStatus::Detached {
        // Remove remote authority first. Every command/browse/transfer processor checks this
        // durable binding, so no new mobile work can race with the disposable cleanup below.
        roots.detach_room(room_id)?;
    }
    let watcher_stopped = match watchers.stop(&root.root_id) {
        Ok(stopped) => stopped,
        Err(_) => {
            eprintln!(
                "ROOM_DETACH_WATCHER_CLEANUP_FAILED root_id={}",
                root.root_id
            );
            false
        }
    };
    let index_cleared = match file_engine_cli::file_index::clear_index(&root.root) {
        Ok(()) => true,
        Err(_) => {
            // A missing/unmounted root must not leave a removed server room active locally. The
            // local root and its journal remain registered, and an idempotent replay retries this
            // disposable cleanup if the path becomes available again.
            eprintln!("ROOM_DETACH_INDEX_CLEANUP_FAILED root_id={}", root.root_id);
            false
        }
    };
    Ok(Some(LocalRoomDetachReport {
        root_id: root.root_id,
        room_id: room_id.to_string(),
        watcher_stopped,
        index_cleared,
    }))
}

pub(crate) async fn apply_local_device_revoked(
    runtime: &AgentRuntime,
    sync_store: &AgentSyncStore,
    roots: &ManagedRootStore,
    watchers: &WatcherStore,
) -> Result<AgentConnectionStatus, String> {
    let device_id = runtime.connection_status().device_id;
    let bound_roots = roots
        .list()?
        .into_iter()
        .filter(|root| {
            root.room_id.is_some() || root.room_binding_status == RoomBindingStatus::Detached
        })
        .collect::<Vec<_>>();
    if roots.detach_all_rooms().is_err() {
        // The server verdict remains authoritative. Keep progressing toward credential removal;
        // the in-memory store has already been made fail-closed and persistence can retry later.
        eprintln!("DEVICE_REVOKE_ROOM_DETACH_PERSIST_FAILED");
    }
    for root in &bound_roots {
        if watchers.stop(&root.root_id).is_err() {
            eprintln!(
                "DEVICE_REVOKE_WATCHER_CLEANUP_FAILED root_id={}",
                root.root_id
            );
        }
        // The index is disposable; the operation journal shares this DB and is deliberately kept.
        if file_engine_cli::file_index::clear_index(&root.root).is_err() {
            eprintln!(
                "DEVICE_REVOKE_INDEX_CLEANUP_FAILED root_id={}",
                root.root_id
            );
        }
    }
    if let Some(device_id) = device_id {
        if sync_store.clear_device(&device_id).await.is_err() {
            eprintln!("DEVICE_REVOKE_SYNC_CURSOR_CLEANUP_FAILED");
        }
    }
    runtime.forget_device().map_err(|error| error.to_string())
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub async fn revoke_agent_device(
    idempotency_key: String,
    runtime: tauri::State<'_, AgentRuntime>,
    sync_store: tauri::State<'_, AgentSyncStore>,
    roots: tauri::State<'_, ManagedRootStore>,
    watchers: tauri::State<'_, WatcherStore>,
    background: tauri::State<'_, crate::background::BackgroundRuntime>,
) -> Result<AgentConnectionStatus, String> {
    runtime
        .revoke_self(idempotency_key)
        .await
        .map_err(|error| error.to_string())?;
    if background.pause("desktop device revoked by user").is_err() {
        // The server transaction already committed. A poisoned background-status lock must not
        // retain a now-revoked credential or keep remote room bindings active locally.
        eprintln!("DEVICE_REVOKE_BACKGROUND_PAUSE_FAILED");
    }
    apply_local_device_revoked(&runtime, &sync_store, &roots, &watchers).await
}

#[cfg(not(feature = "tauri-commands"))]
pub async fn revoke_agent_device(
    runtime: &AgentRuntime,
    sync_store: &AgentSyncStore,
    roots: &ManagedRootStore,
    watchers: &WatcherStore,
    idempotency_key: String,
) -> Result<AgentConnectionStatus, String> {
    runtime
        .revoke_self(idempotency_key)
        .await
        .map_err(|error| error.to_string())?;
    apply_local_device_revoked(runtime, sync_store, roots, watchers).await
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
    use std::fs;

    use file_engine_cli::journal::{JournalAction, JournalEntry, JournalStatus, JournalStore};
    use tempfile::tempdir;

    use crate::agent::{AgentConnectionState, AgentRuntime};
    use crate::storage::managed_roots::{ManagedRoot, ManagedRootStore, RoomBindingStatus};
    use crate::storage::watchers::WatcherStore;

    use super::{
        apply_local_room_detached, disconnect_agent_room, get_agent_connection_status,
        poll_agent_commands, preflight_agent_room_binding, prepare_agent_room_binding,
        send_agent_heartbeat, submit_cleanliness_snapshot,
    };

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

    #[test]
    fn room_detach_is_idempotent_and_preserves_files_and_journal() {
        let temp = tempdir().expect("tempdir");
        let root_path = temp.path().join("root");
        fs::create_dir_all(&root_path).expect("root");
        fs::write(root_path.join("note.txt"), "note").expect("file");
        let roots = ManagedRootStore::default();
        roots
            .upsert(ManagedRoot::new(
                "root-1".to_string(),
                root_path.display().to_string(),
                "Root".to_string(),
            ))
            .expect("root registration");
        roots
            .bind_room("root-1", "room-1".to_string())
            .expect("bind");
        file_engine_cli::file_index::reindex_root(&root_path).expect("index");
        JournalStore::open(&root_path)
            .expect("journal")
            .append(&JournalEntry {
                operation_id: "op-preserved".to_string(),
                status: JournalStatus::Planned,
                action: JournalAction::Move,
                from: "note.txt".to_string(),
                to: "Documents/note.txt".to_string(),
                created_unix_ms: 1,
            })
            .expect("journal entry");
        let watchers = WatcherStore::default();

        let first = apply_local_room_detached("room-1", &roots, &watchers)
            .expect("detach")
            .expect("report");
        let second = apply_local_room_detached("room-1", &roots, &watchers)
            .expect("replayed detach")
            .expect("idempotent report");

        assert!(first.index_cleared);
        assert!(second.index_cleared);
        assert!(root_path.join("note.txt").is_file());
        assert!(file_engine_cli::file_index::list_index(&root_path)
            .expect("index")
            .files
            .is_empty());
        assert_eq!(
            JournalStore::open(&root_path)
                .expect("journal")
                .read_all()
                .expect("journal entries")
                .len(),
            1
        );
        let root = roots.get("root-1").expect("root");
        assert_eq!(root.room_id, None);
        assert_eq!(root.detached_room_id.as_deref(), Some("room-1"));
        assert_eq!(root.room_binding_status, RoomBindingStatus::Detached);
    }

    #[test]
    fn missing_root_does_not_keep_a_removed_room_active() {
        let temp = tempdir().expect("tempdir");
        let root_path = temp.path().join("root");
        fs::create_dir_all(&root_path).expect("root");
        let roots = ManagedRootStore::default();
        roots
            .upsert(ManagedRoot::new(
                "root-1".to_string(),
                root_path.display().to_string(),
                "Root".to_string(),
            ))
            .expect("root registration");
        roots
            .bind_room("root-1", "room-1".to_string())
            .expect("bind");
        fs::remove_dir(&root_path).expect("remove root");

        let report = apply_local_room_detached("room-1", &roots, &WatcherStore::default())
            .expect("detach remains authoritative")
            .expect("report");

        let root = roots.get("root-1").expect("root");
        assert!(!report.index_cleared);
        assert_eq!(root.room_id, None);
        assert_eq!(root.detached_room_id.as_deref(), Some("room-1"));
        assert_eq!(root.room_binding_status, RoomBindingStatus::Detached);
    }

    #[tokio::test]
    async fn retry_after_event_won_the_delete_race_is_local_success() {
        let temp = tempdir().expect("tempdir");
        let root_path = temp.path().join("root");
        fs::create_dir_all(&root_path).expect("root");
        let roots = ManagedRootStore::default();
        roots
            .upsert(ManagedRoot::new(
                "root-1".to_string(),
                root_path.display().to_string(),
                "Root".to_string(),
            ))
            .expect("root registration");
        roots
            .bind_room("root-1", "room-1".to_string())
            .expect("bind");
        let watchers = WatcherStore::default();
        apply_local_room_detached("room-1", &roots, &watchers)
            .expect("event detach")
            .expect("event report");

        // No server or credential is configured. Success proves the retry short-circuits from
        // the durable detached tombstone instead of issuing another DELETE or failing preflight.
        let report = disconnect_agent_room(
            &AgentRuntime::default(),
            &roots,
            &watchers,
            "root-1".to_string(),
            "same-retry-key".to_string(),
            false,
        )
        .await
        .expect("idempotent retry");

        assert_eq!(report.room_id, "room-1");
        assert!(report.index_cleared);
    }

    #[test]
    fn explicit_room_reconnect_rebuilds_cleared_index_before_binding_active() {
        let temp = tempdir().expect("tempdir");
        let root_path = temp.path().join("root");
        fs::create_dir_all(&root_path).expect("root");
        fs::write(root_path.join("report.txt"), "report").expect("file");
        let roots = ManagedRootStore::default();
        roots
            .upsert(ManagedRoot::new(
                "root-1".to_string(),
                root_path.display().to_string(),
                "Root".to_string(),
            ))
            .expect("root registration");
        roots
            .bind_room("root-1", "old-room".to_string())
            .expect("old bind");
        file_engine_cli::file_index::reindex_root(&root_path).expect("old index");
        apply_local_room_detached("old-room", &roots, &WatcherStore::default())
            .expect("detach")
            .expect("report");
        assert!(
            !file_engine_cli::file_index::index_is_initialized(&root_path).expect("cleared state")
        );

        prepare_agent_room_binding(
            &roots,
            &crate::agent::AgentRoomSync {
                room_id: "new-room".to_string(),
                root_id: "root-1".to_string(),
                name: "Root".to_string(),
                created: true,
            },
        )
        .expect("explicit reconnect");

        let root = roots.get("root-1").expect("root");
        assert_eq!(root.room_id.as_deref(), Some("new-room"));
        assert_eq!(root.room_binding_status, RoomBindingStatus::Active);
        assert!(
            file_engine_cli::file_index::index_is_initialized(&root_path).expect("initialized")
        );
        assert_eq!(
            file_engine_cli::file_index::search_index(&root_path, "report")
                .expect("search")
                .files
                .len(),
            1
        );
    }

    #[test]
    fn reconnect_preflight_rejects_a_missing_root_before_server_mutation() {
        let temp = tempdir().expect("tempdir");
        let root_path = temp.path().join("root");
        fs::create_dir_all(&root_path).expect("root");
        let roots = ManagedRootStore::default();
        roots
            .upsert(ManagedRoot::new(
                "root-1".to_string(),
                root_path.display().to_string(),
                "Root".to_string(),
            ))
            .expect("root registration");
        fs::remove_dir_all(&root_path).expect("remove root");

        preflight_agent_room_binding(&roots, "root-1")
            .expect_err("a missing root must fail before the server call");

        let root = roots.get("root-1").expect("root state");
        assert_eq!(root.room_binding_status, RoomBindingStatus::Unbound);
        assert!(root.room_id.is_none());
    }

    #[tokio::test]
    async fn cleanliness_sync_cannot_implicitly_rebind_a_detached_root() {
        let temp = tempdir().expect("tempdir");
        let root_path = temp.path().join("root");
        fs::create_dir_all(&root_path).expect("root");
        let roots = ManagedRootStore::default();
        roots
            .upsert(ManagedRoot::new(
                "root-1".to_string(),
                root_path.display().to_string(),
                "Root".to_string(),
            ))
            .expect("root registration");
        roots
            .bind_room("root-1", "room-1".to_string())
            .expect("bind");
        roots.detach_room("room-1").expect("detach");

        let error =
            submit_cleanliness_snapshot("root-1".to_string(), &AgentRuntime::default(), &roots)
                .await
                .expect_err("detached root must not recreate a room");

        assert!(error.contains("ROOM_DETACHED"));
        assert_eq!(
            roots.get("root-1").expect("root").room_binding_status,
            RoomBindingStatus::Detached
        );
    }
}
