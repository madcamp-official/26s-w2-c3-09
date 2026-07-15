use crate::agent::{
    AgentChatMessage, AgentChatQuickCleanupResult, AgentChatQuickView, AgentChatSendResult,
    AgentChatSession, AgentCommand, AgentCommandDraftConfirmation, AgentConnectionStatus,
    AgentDecision, AgentOpenProposal, AgentProposalDetails, AgentRoomSnapshot, AgentRoomSync,
    AgentRule, AgentRuleDraftConfirmation, AgentRuleDraftRejection, AgentRuntime, HeartbeatResult,
    PairingSession, PairingStatus, SyncEvent,
};
use crate::cleanliness::{calculate_cleanliness_snapshot, CleanlinessSnapshot};
#[cfg(feature = "tauri-commands")]
use crate::command_processor::{process_commands, CommandProcessingStatus};
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
#[cfg(feature = "tauri-commands")]
use crate::work_limiter::WorkLimiter;
use file_engine_cli::journal::{read_operation_history, JournalStatus};
use file_engine_cli::rules::{default_rule_set, load_rule_set_for_root, RuleSet, RULES_FILE};
use std::fs;
use std::path::Path;

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

#[derive(Clone, Debug, serde::Serialize, PartialEq)]
pub struct ChatCommandDraftExecutionReport {
    pub draft: AgentCommandDraftConfirmation,
    pub command_report: CommandProcessingReport,
    pub proposal: AgentProposalDetails,
    pub decision: AgentDecision,
    pub execution_report: DecisionProcessingReport,
    pub proposal_outbox_report: crate::outbox_processor::OutboxFlushReport,
    pub execution_outbox_report: crate::outbox_processor::OutboxFlushReport,
}

#[derive(Clone, Debug, serde::Serialize, PartialEq)]
pub struct ChatProposalExecutionReport {
    pub proposal: AgentProposalDetails,
    pub decision: AgentDecision,
    pub execution_report: DecisionProcessingReport,
    pub execution_outbox_report: crate::outbox_processor::OutboxFlushReport,
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
    overlay_runtime: tauri::State<'_, crate::overlay::OverlayRuntime>,
    app: tauri::AppHandle,
) -> Result<PairingStatus, String> {
    let status = runtime
        .poll_pairing(session_id, desktop_nonce)
        .await
        .map_err(|error| error.to_string())?;

    // Pairing just completed (CLAIMED): swap the setup/main window for the overlay, mirroring the
    // startup path in `run()` that shows the overlay whenever a device is already paired. Without
    // this, the overlay would only appear on the next app restart.
    if status.device_id.is_some() {
        crate::tray::hide_main_window(&app);
        if let Err(error) = crate::commands::overlay::show_overlay_window(&app, &overlay_runtime) {
            eprintln!("failed to show overlay after pairing: {error}");
        }
    }

    Ok(status)
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
    limiter: tauri::State<'_, WorkLimiter>,
) -> Result<CommandProcessingReport, String> {
    let _permit = limiter.try_scan()?;
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
    limiter: tauri::State<'_, WorkLimiter>,
) -> Result<DecisionProcessingReport, String> {
    let _permit = limiter.try_write()?;
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
    limiter: tauri::State<'_, WorkLimiter>,
) -> Result<FileBrowseProcessingReport, String> {
    let _permit = limiter.try_scan()?;
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
    limiter: tauri::State<'_, WorkLimiter>,
) -> Result<FileTransferProcessingReport, String> {
    let _permit = limiter.try_transfer()?;
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
pub async fn approve_agent_command_draft_and_execute(
    draft_id: String,
    room_id: String,
    idempotency_key: String,
    runtime: tauri::State<'_, AgentRuntime>,
    roots: tauri::State<'_, ManagedRootStore>,
    outbox: tauri::State<'_, OutboxStore>,
    limiter: tauri::State<'_, WorkLimiter>,
) -> Result<ChatCommandDraftExecutionReport, String> {
    approve_agent_command_draft_and_execute_impl(
        draft_id,
        room_id,
        idempotency_key,
        &runtime,
        &roots,
        &outbox,
        &limiter,
    )
    .await
}

#[cfg(feature = "tauri-commands")]
async fn approve_agent_command_draft_and_execute_impl(
    draft_id: String,
    room_id: String,
    idempotency_key: String,
    runtime: &AgentRuntime,
    roots: &ManagedRootStore,
    outbox: &OutboxStore,
    limiter: &WorkLimiter,
) -> Result<ChatCommandDraftExecutionReport, String> {
    let confirm_key = scoped_chat_approval_key("confirm", &idempotency_key)?;
    let decision_key = scoped_chat_approval_key("decision", &idempotency_key)?;
    let draft = runtime
        .confirm_command_draft(draft_id, confirm_key)
        .await
        .map_err(|error| error.to_string())?;
    let command = draft.command.clone().ok_or_else(|| {
        "COMMAND_DRAFT_NOT_EXECUTABLE: this draft did not materialize a desktop command".to_string()
    })?;
    if command.room_id != room_id {
        return Err(
            "COMMAND_ROOM_MISMATCH: confirmed command belongs to a different room".to_string(),
        );
    }

    let command_report = {
        let _permit = limiter.try_scan()?;
        process_commands(runtime, roots, outbox, vec![command.clone()]).await?
    };
    if !ensure_chat_command_submitted(&command_report, &command.command_id)? {
        return Err("NO_PROPOSAL_ITEMS: already clean; no proposal items were found".to_string());
    }
    let proposal_outbox_report = crate::outbox_processor::flush_outbox(runtime, outbox).await?;
    let proposal = proposal_for_command(runtime, room_id, command.command_id.clone()).await?;
    let executed = approve_proposal_details_and_execute(
        proposal.clone(),
        decision_key,
        runtime,
        roots,
        outbox,
        limiter,
    )
    .await?;

    Ok(ChatCommandDraftExecutionReport {
        draft,
        command_report,
        proposal,
        decision: executed.decision,
        execution_report: executed.execution_report,
        proposal_outbox_report,
        execution_outbox_report: executed.execution_outbox_report,
    })
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub async fn confirm_agent_rule_draft(
    draft_id: String,
    room_id: String,
    idempotency_key: String,
    runtime: tauri::State<'_, AgentRuntime>,
    roots: tauri::State<'_, ManagedRootStore>,
) -> Result<AgentRuleDraftConfirmation, String> {
    confirm_agent_rule_draft_impl(draft_id, room_id, idempotency_key, &runtime, &roots).await
}

#[cfg(not(feature = "tauri-commands"))]
pub async fn confirm_agent_rule_draft(
    runtime: &AgentRuntime,
    roots: &ManagedRootStore,
    draft_id: String,
    room_id: String,
    idempotency_key: String,
) -> Result<AgentRuleDraftConfirmation, String> {
    confirm_agent_rule_draft_impl(draft_id, room_id, idempotency_key, runtime, roots).await
}

async fn confirm_agent_rule_draft_impl(
    draft_id: String,
    room_id: String,
    idempotency_key: String,
    runtime: &AgentRuntime,
    roots: &ManagedRootStore,
) -> Result<AgentRuleDraftConfirmation, String> {
    let confirmation = runtime
        .confirm_rule_draft(draft_id, idempotency_key)
        .await
        .map_err(|error| error.to_string())?;
    if confirmation.rule.room_id != room_id {
        return Err("RULE_ROOM_MISMATCH: confirmed rule belongs to a different room".to_string());
    }
    apply_server_rule_to_local_root(roots, &confirmation.rule)?;
    Ok(confirmation)
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub async fn reject_agent_rule_draft(
    draft_id: String,
    runtime: tauri::State<'_, AgentRuntime>,
) -> Result<AgentRuleDraftRejection, String> {
    runtime
        .reject_rule_draft(draft_id)
        .await
        .map_err(|error| error.to_string())
}

#[cfg(not(feature = "tauri-commands"))]
pub async fn reject_agent_rule_draft(
    runtime: &AgentRuntime,
    draft_id: String,
) -> Result<AgentRuleDraftRejection, String> {
    runtime
        .reject_rule_draft(draft_id)
        .await
        .map_err(|error| error.to_string())
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub async fn list_agent_open_proposals(
    room_id: String,
    runtime: tauri::State<'_, AgentRuntime>,
) -> Result<Vec<AgentOpenProposal>, String> {
    runtime
        .open_proposals_for_room(room_id)
        .await
        .map_err(|error| error.to_string())
}

#[cfg(not(feature = "tauri-commands"))]
pub async fn list_agent_open_proposals(
    runtime: &AgentRuntime,
    room_id: String,
) -> Result<Vec<AgentOpenProposal>, String> {
    runtime
        .open_proposals_for_room(room_id)
        .await
        .map_err(|error| error.to_string())
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub async fn approve_agent_open_proposal_and_execute(
    proposal_id: String,
    room_id: String,
    idempotency_key: String,
    runtime: tauri::State<'_, AgentRuntime>,
    roots: tauri::State<'_, ManagedRootStore>,
    outbox: tauri::State<'_, OutboxStore>,
    limiter: tauri::State<'_, WorkLimiter>,
) -> Result<ChatProposalExecutionReport, String> {
    approve_agent_open_proposal_and_execute_impl(
        proposal_id,
        room_id,
        idempotency_key,
        &runtime,
        &roots,
        &outbox,
        &limiter,
    )
    .await
}

#[cfg(feature = "tauri-commands")]
async fn approve_agent_open_proposal_and_execute_impl(
    proposal_id: String,
    room_id: String,
    idempotency_key: String,
    runtime: &AgentRuntime,
    roots: &ManagedRootStore,
    outbox: &OutboxStore,
    limiter: &WorkLimiter,
) -> Result<ChatProposalExecutionReport, String> {
    let proposal = runtime
        .get_proposal(proposal_id)
        .await
        .map_err(|error| error.to_string())?;
    if proposal.room_id != room_id {
        return Err("PROPOSAL_ROOM_MISMATCH: proposal belongs to a different room".to_string());
    }
    let decision_key = scoped_chat_approval_key("decision", &idempotency_key)?;
    approve_proposal_details_and_execute(proposal, decision_key, runtime, roots, outbox, limiter)
        .await
}

#[cfg(feature = "tauri-commands")]
async fn approve_proposal_details_and_execute(
    proposal: AgentProposalDetails,
    decision_key: String,
    runtime: &AgentRuntime,
    roots: &ManagedRootStore,
    outbox: &OutboxStore,
    limiter: &WorkLimiter,
) -> Result<ChatProposalExecutionReport, String> {
    let decision = runtime
        .create_decision(
            proposal.proposal_id.clone(),
            proposal.item_ids.clone(),
            decision_key,
        )
        .await
        .map_err(|error| error.to_string())?;
    let execution_report = {
        let _permit = limiter.try_write()?;
        process_pending_decisions(runtime, roots, outbox).await?
    };
    let execution_outbox_report = crate::outbox_processor::flush_outbox(runtime, outbox).await?;

    Ok(ChatProposalExecutionReport {
        proposal,
        decision,
        execution_report,
        execution_outbox_report,
    })
}

#[cfg(feature = "tauri-commands")]
async fn proposal_for_command(
    runtime: &AgentRuntime,
    room_id: String,
    command_id: String,
) -> Result<AgentProposalDetails, String> {
    let proposals = runtime
        .open_proposals_for_room(room_id)
        .await
        .map_err(|error| error.to_string())?;
    let proposal = proposals
        .into_iter()
        .find(|proposal| proposal.command_id == command_id && proposal.status == "OPEN")
        .ok_or_else(|| {
            "PROPOSAL_NOT_READY: confirmed command did not produce an open proposal yet".to_string()
        })?;
    runtime
        .get_proposal(proposal.proposal_id)
        .await
        .map_err(|error| error.to_string())
}

#[cfg(feature = "tauri-commands")]
fn scoped_chat_approval_key(scope: &str, idempotency_key: &str) -> Result<String, String> {
    if idempotency_key.is_empty()
        || idempotency_key.len() > 100
        || !idempotency_key
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err("approval idempotency key has an invalid format".to_string());
    }
    Ok(format!("{scope}-{idempotency_key}"))
}

#[cfg(feature = "tauri-commands")]
fn ensure_chat_command_submitted(
    report: &CommandProcessingReport,
    command_id: &str,
) -> Result<bool, String> {
    let result = report
        .results
        .iter()
        .find(|result| result.command_id == command_id)
        .ok_or_else(|| {
            "COMMAND_PROCESSING_MISSING: confirmed command was not processed".to_string()
        })?;
    if result.status == CommandProcessingStatus::SubmittedProposal {
        return Ok(true);
    }
    if result.status == CommandProcessingStatus::NoProposal {
        return Ok(false);
    }
    Err(format!(
        "COMMAND_PROCESSING_FAILED: {}",
        result
            .message
            .as_deref()
            .unwrap_or("proposal was not submitted")
    ))
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
pub async fn list_agent_chat_sessions(
    room_id: String,
    runtime: tauri::State<'_, AgentRuntime>,
) -> Result<Vec<AgentChatSession>, String> {
    runtime
        .list_chat_sessions(room_id)
        .await
        .map_err(|error| error.to_string())
}

#[cfg(not(feature = "tauri-commands"))]
pub async fn list_agent_chat_sessions(
    runtime: &AgentRuntime,
    room_id: String,
) -> Result<Vec<AgentChatSession>, String> {
    runtime
        .list_chat_sessions(room_id)
        .await
        .map_err(|error| error.to_string())
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub async fn create_agent_chat_session(
    room_id: String,
    title: Option<String>,
    runtime: tauri::State<'_, AgentRuntime>,
) -> Result<AgentChatSession, String> {
    runtime
        .create_chat_session(room_id, title)
        .await
        .map_err(|error| error.to_string())
}

#[cfg(not(feature = "tauri-commands"))]
pub async fn create_agent_chat_session(
    runtime: &AgentRuntime,
    room_id: String,
    title: Option<String>,
) -> Result<AgentChatSession, String> {
    runtime
        .create_chat_session(room_id, title)
        .await
        .map_err(|error| error.to_string())
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub async fn get_agent_chat_quick_view(
    room_id: String,
    runtime: tauri::State<'_, AgentRuntime>,
) -> Result<AgentChatQuickView, String> {
    runtime
        .chat_quick_view(room_id)
        .await
        .map_err(|error| error.to_string())
}

#[cfg(not(feature = "tauri-commands"))]
pub async fn get_agent_chat_quick_view(
    runtime: &AgentRuntime,
    room_id: String,
) -> Result<AgentChatQuickView, String> {
    runtime
        .chat_quick_view(room_id)
        .await
        .map_err(|error| error.to_string())
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub async fn create_agent_chat_quick_cleanup(
    room_id: String,
    runtime: tauri::State<'_, AgentRuntime>,
) -> Result<AgentChatQuickCleanupResult, String> {
    runtime
        .create_quick_cleanup_suggestion(room_id)
        .await
        .map_err(|error| error.to_string())
}

#[cfg(not(feature = "tauri-commands"))]
pub async fn create_agent_chat_quick_cleanup(
    runtime: &AgentRuntime,
    room_id: String,
) -> Result<AgentChatQuickCleanupResult, String> {
    runtime
        .create_quick_cleanup_suggestion(room_id)
        .await
        .map_err(|error| error.to_string())
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub async fn list_agent_chat_messages(
    session_id: String,
    runtime: tauri::State<'_, AgentRuntime>,
) -> Result<Vec<AgentChatMessage>, String> {
    runtime
        .list_chat_messages(session_id)
        .await
        .map_err(|error| error.to_string())
}

#[cfg(not(feature = "tauri-commands"))]
pub async fn list_agent_chat_messages(
    runtime: &AgentRuntime,
    session_id: String,
) -> Result<Vec<AgentChatMessage>, String> {
    runtime
        .list_chat_messages(session_id)
        .await
        .map_err(|error| error.to_string())
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub async fn mark_agent_chat_session_read(
    session_id: String,
    last_read_message_id: Option<String>,
    runtime: tauri::State<'_, AgentRuntime>,
) -> Result<AgentChatSession, String> {
    runtime
        .mark_chat_session_read(session_id, last_read_message_id)
        .await
        .map_err(|error| error.to_string())
}

#[cfg(not(feature = "tauri-commands"))]
pub async fn mark_agent_chat_session_read(
    runtime: &AgentRuntime,
    session_id: String,
    last_read_message_id: Option<String>,
) -> Result<AgentChatSession, String> {
    runtime
        .mark_chat_session_read(session_id, last_read_message_id)
        .await
        .map_err(|error| error.to_string())
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub async fn send_agent_chat_message(
    session_id: String,
    content: String,
    runtime: tauri::State<'_, AgentRuntime>,
) -> Result<AgentChatSendResult, String> {
    runtime
        .send_chat_message(session_id, content)
        .await
        .map_err(|error| error.to_string())
}

#[cfg(not(feature = "tauri-commands"))]
pub async fn send_agent_chat_message(
    runtime: &AgentRuntime,
    session_id: String,
    content: String,
) -> Result<AgentChatSendResult, String> {
    runtime
        .send_chat_message(session_id, content)
        .await
        .map_err(|error| error.to_string())
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
        if event.event_type == "rule.created" {
            let _ = apply_rule_created_event(runtime, roots, event).await?;
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

pub async fn apply_rule_created_event(
    runtime: &AgentRuntime,
    roots: &ManagedRootStore,
    event: &SyncEvent,
) -> Result<bool, String> {
    if event.event_type != "rule.created" {
        return Ok(false);
    }
    let Some(room_id) = event.room_id.as_deref() else {
        return Ok(false);
    };
    let rule_id = event
        .payload
        .get("ruleId")
        .and_then(|value| value.as_str())
        .unwrap_or(event.aggregate_id.as_str());
    let rules = runtime
        .list_rules(room_id.to_string())
        .await
        .map_err(|error| error.to_string())?;
    let Some(rule) = rules
        .into_iter()
        .find(|candidate| candidate.rule_id == rule_id)
    else {
        return Ok(false);
    };
    apply_server_rule_to_local_root(roots, &rule)?;
    Ok(true)
}

fn apply_server_rule_to_local_root(
    roots: &ManagedRootStore,
    rule: &AgentRule,
) -> Result<(), String> {
    let managed_root = roots
        .find_by_room(&rule.room_id)?
        .ok_or_else(|| format!("managed root is not bound to room: {}", rule.room_id))?;
    let root_path = Path::new(&managed_root.root);
    let state_dir = root_path.join(".mousekeeper");
    let rules_path = state_dir.join(RULES_FILE);
    let mut rule_set = if rules_path.exists() {
        load_rule_set_for_root(root_path)
            .map_err(|error| format!("cannot load existing local rules: {error}"))?
    } else {
        default_rule_set()
    };
    let rule_prefix = format!("{}-any-", rule.rule_id);
    rule_set
        .rules
        .retain(|existing| existing.id != rule.rule_id && !existing.id.starts_with(&rule_prefix));

    if rule.enabled {
        let mut incoming = RuleSet::from_contract_definition(
            rule.rule_id.clone(),
            rule.priority,
            rule.definition.clone(),
        )
        .map_err(|error| format!("invalid server rule definition: {error}"))?;
        rule_set.rules.append(&mut incoming.rules);
    }
    rule_set
        .validate()
        .map_err(|error| format!("invalid merged local rules: {error}"))?;
    fs::create_dir_all(&state_dir).map_err(|error| {
        format!(
            "cannot create rules directory {}: {error}",
            state_dir.display()
        )
    })?;
    let encoded = serde_json::to_string_pretty(&rule_set)
        .map_err(|error| format!("cannot serialize local rules: {error}"))?;
    fs::write(&rules_path, encoded)
        .map_err(|error| format!("cannot write local rules {}: {error}", rules_path.display()))
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

pub(crate) async fn disconnect_agent_room_impl(
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

// These tests exercise the `not(feature = "tauri-commands")` variants above, which take a plain
// `&AgentRuntime` so they are callable without a live Tauri `State`. The `tauri-commands` variants
// need a running app to construct their `State` argument and are not unit-testable this way, so
// this module only compiles for the CLI/no-tauri-commands build.
#[cfg(all(test, not(feature = "tauri-commands")))]
mod tests {
    use std::fs;

    use file_engine_cli::journal::{JournalAction, JournalEntry, JournalStatus, JournalStore};
    use tempfile::tempdir;

    use crate::agent::{AgentConnectionState, AgentRule, AgentRuntime};
    use crate::storage::managed_roots::{ManagedRoot, ManagedRootStore, RoomBindingStatus};
    use crate::storage::watchers::WatcherStore;
    use file_engine_cli::rules::RULES_FILE;

    use super::{
        apply_local_room_detached, apply_server_rule_to_local_root, disconnect_agent_room,
        get_agent_connection_status, poll_agent_commands, preflight_agent_room_binding,
        prepare_agent_room_binding, send_agent_heartbeat, submit_cleanliness_snapshot,
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
    fn corrupt_local_rules_json_blocks_server_rule_merge_without_overwriting() {
        let temp = tempdir().expect("tempdir");
        let root_path = temp.path().join("root");
        let state_dir = root_path.join(".mousekeeper");
        fs::create_dir_all(&state_dir).expect("state dir");
        let rules_path = state_dir.join(RULES_FILE);
        fs::write(&rules_path, "{ not valid json").expect("corrupt rules file");

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

        let rule = AgentRule {
            rule_id: "rule-1".to_string(),
            room_id: "room-1".to_string(),
            name: "PDF rule".to_string(),
            definition: serde_json::json!({
                "match": "ALL",
                "conditions": [{"field": "extension", "operator": "IN", "value": [".pdf"]}],
                "action": {"type": "MOVE", "destinationTemplate": "pdf"}
            }),
            priority: 100,
            enabled: true,
            version: 1,
        };

        let before = fs::read_to_string(&rules_path).expect("read before");
        let result = apply_server_rule_to_local_root(&roots, &rule);

        assert!(
            result.is_err(),
            "merging into a corrupt local rules.json must fail instead of silently succeeding"
        );
        let after = fs::read_to_string(&rules_path).expect("read after");
        assert_eq!(
            before, after,
            "a failed merge must not touch the local rules.json at all"
        );
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
