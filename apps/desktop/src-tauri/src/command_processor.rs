use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use file_engine_cli::proposal::{
    proposal_id, propose_for_root, propose_for_root_with_rule_set, Proposal, ProposalAction,
    ProposalReport, ProposalStatus,
};
use file_engine_cli::rules::RuleSet;
use file_engine_cli::{fs_safety::is_link_or_reparse_point, path_guard::PathGuard};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::agent::{AgentCommand, AgentRuntime};
use crate::outbox_processor::{enqueue_command_status, enqueue_proposal};
use crate::storage::managed_roots::ManagedRootStore;
use crate::storage::outbox::OutboxStore;

/// Boundary for delegated work from mobile/server/AI.
///
/// This module may generate proposal submissions, but it must not call direct file operations
/// such as create, rename, or trash. Those commands stay local/manual in `commands::file_engine`.
const ANALYZE: &str = "ANALYZE";
const RENAME: &str = "RENAME";
const MOVE: &str = "MOVE";
const TRASH: &str = "TRASH";
const CREATE: &str = "CREATE";
const FIND: &str = "FIND";
const DOWNLOAD: &str = "DOWNLOAD";
const UPLOAD: &str = "UPLOAD";
const ORGANIZE: &str = "ORGANIZE";
const ORGANIZE_FILES: &str = "organize_files";
const ORGANIZE_ROOT: &str = "ORGANIZE_ROOT";
const DEFAULT_MAX_PROPOSALS: usize = 50;
const USER_REQUESTED_RENAME_REASON: &str = "USER_REQUESTED_RENAME";
const USER_REQUESTED_MOVE_REASON: &str = "USER_REQUESTED_MOVE";
const USER_REQUESTED_TRASH_REASON: &str = "USER_REQUESTED_TRASH";
const USER_REQUESTED_CREATE_DIR_REASON: &str = "USER_REQUESTED_CREATE_DIR";
const USER_REQUESTED_CREATE_FILE_REASON: &str = "USER_REQUESTED_CREATE_FILE";

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct CommandProcessingReport {
    pub inspected_count: usize,
    pub processed_count: usize,
    pub submitted_proposal_count: usize,
    pub failed_count: usize,
    pub skipped_count: usize,
    pub results: Vec<CommandProcessingResult>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct CommandProcessingResult {
    pub command_id: String,
    pub command_type: String,
    pub status: CommandProcessingStatus,
    pub message: Option<String>,
    pub proposal_item_count: usize,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CommandProcessingStatus {
    SubmittedProposal,
    Failed,
    Skipped,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct OrganizeFilesPayload {
    #[serde(default)]
    root_id: Option<String>,
    #[serde(default)]
    relative_path: Option<String>,
    #[serde(default)]
    scope_relative_path: Option<String>,
    #[serde(default)]
    max_proposals: Option<usize>,
    #[serde(default)]
    rule_mode: Option<String>,
    #[serde(default, alias = "instruction")]
    user_intent: Option<String>,
    /// An AI/server-produced Rule DSL draft (plan item 12). When present the desktop uses this
    /// draft instead of the root's saved rules — but only after strictly parsing and validating it
    /// (see `resolve_proposal`), so malformed AI output is rejected before any file logic runs.
    #[serde(default)]
    rule_draft: Option<Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct RenameCommandPayload {
    root_id: String,
    source_relative_path: String,
    new_name: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct MoveCommandPayload {
    root_id: String,
    source_relative_paths: Vec<String>,
    destination_relative_directory: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct TrashCommandPayload {
    root_id: String,
    source_relative_paths: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct CreateCommandPayload {
    root_id: String,
    kind: String,
    relative_path: String,
    #[serde(default)]
    content: Option<String>,
}

impl OrganizeFilesPayload {
    fn scope(&self) -> &str {
        self.scope_relative_path
            .as_deref()
            .or(self.relative_path.as_deref())
            .unwrap_or_default()
    }

    fn max_proposals(&self) -> usize {
        self.max_proposals.unwrap_or(DEFAULT_MAX_PROPOSALS)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentProposalSubmission {
    pub command_id: String,
    pub room_id: String,
    pub summary: AgentProposalSummary,
    pub expires_at: Option<String>,
    pub items: Vec<AgentProposalItem>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentProposalSummary {
    pub item_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub readme_draft: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub readme_diff: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentProposalItem {
    pub item_order: usize,
    pub action_type: AgentProposalActionType,
    pub source_relative_path: Option<String>,
    pub destination_relative_path: Option<String>,
    pub reason_code: String,
    pub precondition: serde_json::Value,
    pub conflict_state: AgentProposalConflictState,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AgentProposalActionType {
    Move,
    Quarantine,
    CreateDir,
    CreateFile,
    ReadmeWrite,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AgentProposalConflictState {
    None,
    NameConflict,
}

pub async fn process_pending_commands(
    agent: &AgentRuntime,
    roots: &ManagedRootStore,
    outbox: &OutboxStore,
) -> Result<CommandProcessingReport, String> {
    let commands = agent
        .poll_commands()
        .await
        .map_err(|error| error.to_string())?;
    process_commands(agent, roots, outbox, commands).await
}

pub async fn process_commands(
    agent: &AgentRuntime,
    roots: &ManagedRootStore,
    outbox: &OutboxStore,
    commands: Vec<AgentCommand>,
) -> Result<CommandProcessingReport, String> {
    let mut report = CommandProcessingReport {
        inspected_count: commands.len(),
        processed_count: 0,
        submitted_proposal_count: 0,
        failed_count: 0,
        skipped_count: 0,
        results: Vec::new(),
    };

    for command in commands {
        if !is_processable_command_status(&command.status) {
            report.skipped_count += 1;
            report.results.push(CommandProcessingResult {
                command_id: command.command_id,
                command_type: command.command_type,
                status: CommandProcessingStatus::Skipped,
                message: Some(
                    "command type or status is not handled by the desktop proposal processor"
                        .to_string(),
                ),
                proposal_item_count: 0,
            });
            continue;
        }

        if is_contract_command_without_desktop_handler(&command) {
            report.processed_count += 1;
            report.failed_count += 1;
            let claim_error = claim_command_for_analysis(agent, &command).await.err();
            let sync_error = if claim_error.is_none() {
                enqueue_command_status(outbox, &command.command_id, "FAILED").err()
            } else {
                None
            };
            report.results.push(CommandProcessingResult {
                command_id: command.command_id,
                command_type: command.command_type,
                status: CommandProcessingStatus::Failed,
                message: Some(match (claim_error, sync_error) {
                    (Some(error), _) => format!(
                        "command intent is not yet handled by the desktop processor; additionally failed to claim command for failure reporting: {error}"
                    ),
                    (None, Some(error)) => format!(
                        "command intent is not yet handled by the desktop processor; additionally failed to queue FAILED status: {error}"
                    ),
                    (None, None) => "command intent is not yet handled by the desktop processor".to_string(),
                }),
                proposal_item_count: 0,
            });
            continue;
        }

        if !is_supported_command(&command) {
            report.skipped_count += 1;
            report.results.push(CommandProcessingResult {
                command_id: command.command_id,
                command_type: command.command_type,
                status: CommandProcessingStatus::Skipped,
                message: Some(
                    "command type or status is not handled by the desktop proposal processor"
                        .to_string(),
                ),
                proposal_item_count: 0,
            });
            continue;
        }

        report.processed_count += 1;
        let result = match command.command_type.as_str() {
            RENAME => process_rename_command(agent, roots, outbox, &command).await,
            MOVE => process_move_command(agent, roots, outbox, &command).await,
            TRASH => process_trash_command(agent, roots, outbox, &command).await,
            CREATE => process_create_command(agent, roots, outbox, &command).await,
            _ => process_organize_command(agent, roots, outbox, &command).await,
        };
        match result {
            Ok(item_count) => {
                report.submitted_proposal_count += 1;
                report.results.push(CommandProcessingResult {
                    command_id: command.command_id,
                    command_type: command.command_type,
                    status: CommandProcessingStatus::SubmittedProposal,
                    message: None,
                    proposal_item_count: item_count,
                });
            }
            Err(error) => {
                report.failed_count += 1;
                // The FAILED report goes through the outbox so it is not lost if the network is
                // down; the command is already in ANALYZING so it will not be re-polled.
                let _ = enqueue_command_status(outbox, &command.command_id, "FAILED");
                report.results.push(CommandProcessingResult {
                    command_id: command.command_id,
                    command_type: command.command_type,
                    status: CommandProcessingStatus::Failed,
                    message: Some(error),
                    proposal_item_count: 0,
                });
            }
        }
    }

    Ok(report)
}

fn is_processable_command_status(status: &str) -> bool {
    matches!(status, "QUEUED" | "DELIVERED" | "ANALYZING")
}

pub(crate) async fn claim_command_for_analysis(
    agent: &AgentRuntime,
    command: &AgentCommand,
) -> Result<(), String> {
    match command.status.as_str() {
        "QUEUED" => {
            agent
                .update_command_status(command.command_id.clone(), "DELIVERED".to_string())
                .await
                .map_err(|error| error.to_string())?;
            agent
                .update_command_status(command.command_id.clone(), "ANALYZING".to_string())
                .await
                .map_err(|error| error.to_string())?;
            Ok(())
        }
        "DELIVERED" => {
            agent
                .update_command_status(command.command_id.clone(), "ANALYZING".to_string())
                .await
                .map_err(|error| error.to_string())?;
            Ok(())
        }
        "ANALYZING" => Ok(()),
        _ => Err(format!(
            "command status is not claimable for analysis: {}",
            command.status
        )),
    }
}

async fn process_create_command(
    agent: &AgentRuntime,
    roots: &ManagedRootStore,
    outbox: &OutboxStore,
    command: &AgentCommand,
) -> Result<usize, String> {
    let payload = parse_create_payload(command)?;

    claim_command_for_analysis(agent, command).await?;

    let room = agent
        .root_id_for_room(command.room_id.clone())
        .await
        .map_err(|error| error.to_string())?;
    let managed_root = roots.ensure_active_room_binding(&room.root_id, &command.room_id)?;
    if payload.root_id != managed_root.root_id {
        return Err("command rootId does not match the room binding".to_string());
    }
    if !managed_root.enabled {
        return Err(format!(
            "managed root is disabled: {}",
            managed_root.root_id
        ));
    }

    let local_report = build_create_proposal_report(&managed_root.root, &payload)?;
    let submission = build_agent_proposal_submission(command, local_report, 1)?;
    let item_count = submission.items.len();
    enqueue_proposal(outbox, &submission)?;

    Ok(item_count)
}

async fn process_trash_command(
    agent: &AgentRuntime,
    roots: &ManagedRootStore,
    outbox: &OutboxStore,
    command: &AgentCommand,
) -> Result<usize, String> {
    let payload = parse_trash_payload(command)?;

    claim_command_for_analysis(agent, command).await?;

    let room = agent
        .root_id_for_room(command.room_id.clone())
        .await
        .map_err(|error| error.to_string())?;
    let managed_root = roots.ensure_active_room_binding(&room.root_id, &command.room_id)?;
    if payload.root_id != managed_root.root_id {
        return Err("command rootId does not match the room binding".to_string());
    }
    if !managed_root.enabled {
        return Err(format!(
            "managed root is disabled: {}",
            managed_root.root_id
        ));
    }

    let local_report = build_trash_proposal_report(&managed_root.root, &payload)?;
    let submission = build_agent_proposal_submission(command, local_report, 200)?;
    let item_count = submission.items.len();
    enqueue_proposal(outbox, &submission)?;

    Ok(item_count)
}

async fn process_move_command(
    agent: &AgentRuntime,
    roots: &ManagedRootStore,
    outbox: &OutboxStore,
    command: &AgentCommand,
) -> Result<usize, String> {
    let payload = parse_move_payload(command)?;

    claim_command_for_analysis(agent, command).await?;

    let room = agent
        .root_id_for_room(command.room_id.clone())
        .await
        .map_err(|error| error.to_string())?;
    let managed_root = roots.ensure_active_room_binding(&room.root_id, &command.room_id)?;
    if payload.root_id != managed_root.root_id {
        return Err("command rootId does not match the room binding".to_string());
    }
    if !managed_root.enabled {
        return Err(format!(
            "managed root is disabled: {}",
            managed_root.root_id
        ));
    }

    let local_report = build_move_proposal_report(&managed_root.root, &payload)?;
    let submission = build_agent_proposal_submission(command, local_report, 200)?;
    let item_count = submission.items.len();
    enqueue_proposal(outbox, &submission)?;

    Ok(item_count)
}

async fn process_rename_command(
    agent: &AgentRuntime,
    roots: &ManagedRootStore,
    outbox: &OutboxStore,
    command: &AgentCommand,
) -> Result<usize, String> {
    let payload = parse_rename_payload(command)?;

    // Claim the command before local disk inspection so a slow proposal cannot be polled and
    // processed twice. This still performs no file mutation; it only moves the server command into
    // the same analysis/proposal path used by organize commands.
    claim_command_for_analysis(agent, command).await?;

    let room = agent
        .root_id_for_room(command.room_id.clone())
        .await
        .map_err(|error| error.to_string())?;
    let managed_root = roots.ensure_active_room_binding(&room.root_id, &command.room_id)?;
    if payload.root_id != managed_root.root_id {
        return Err("command rootId does not match the room binding".to_string());
    }
    if !managed_root.enabled {
        return Err(format!(
            "managed root is disabled: {}",
            managed_root.root_id
        ));
    }

    let local_report = build_rename_proposal_report(&managed_root.root, &payload)?;
    let submission = build_agent_proposal_submission(command, local_report, 1)?;
    let item_count = submission.items.len();
    enqueue_proposal(outbox, &submission)?;

    Ok(item_count)
}

async fn process_organize_command(
    agent: &AgentRuntime,
    roots: &ManagedRootStore,
    outbox: &OutboxStore,
    command: &AgentCommand,
) -> Result<usize, String> {
    let payload = parse_organize_payload(command)?;
    validate_relative_scope(payload.scope())?;

    // ANALYZING stays a synchronous claim: it moves the command out of QUEUED so it is not polled
    // and re-processed again before the proposal is built. (Symmetric with the execution claim.)
    claim_command_for_analysis(agent, command).await?;

    let room = agent
        .root_id_for_room(command.room_id.clone())
        .await
        .map_err(|error| error.to_string())?;
    let managed_root = roots.ensure_active_room_binding(&room.root_id, &command.room_id)?;
    if let Some(requested_root_id) = payload.root_id.as_deref() {
        if requested_root_id != managed_root.root_id {
            return Err("command rootId does not match the room binding".to_string());
        }
    }
    if !managed_root.enabled {
        return Err(format!(
            "managed root is disabled: {}",
            managed_root.root_id
        ));
    }

    let local_report = resolve_proposal(&managed_root.root, &payload)?;
    let submission =
        build_agent_proposal_submission(command, local_report, payload.max_proposals())?;
    let item_count = submission.items.len();
    if item_count == 0 {
        return Err("proposal command produced no proposal items".to_string());
    }

    // The proposal is the durable result of this command. Enqueue it rather than POSTing inline:
    // if delivery fails the flush loop retries, and because the command is already ANALYZING it
    // will not be rebuilt from a fresh poll — so without the outbox a network blip here would
    // strand the command with no proposal.
    enqueue_proposal(outbox, &submission)?;

    Ok(item_count)
}

fn parse_rename_payload(command: &AgentCommand) -> Result<RenameCommandPayload, String> {
    serde_json::from_value(command.payload.clone())
        .map_err(|error| format!("invalid rename payload: {error}"))
}

fn parse_move_payload(command: &AgentCommand) -> Result<MoveCommandPayload, String> {
    serde_json::from_value(command.payload.clone())
        .map_err(|error| format!("invalid move payload: {error}"))
}

fn parse_trash_payload(command: &AgentCommand) -> Result<TrashCommandPayload, String> {
    serde_json::from_value(command.payload.clone())
        .map_err(|error| format!("invalid trash payload: {error}"))
}

fn parse_create_payload(command: &AgentCommand) -> Result<CreateCommandPayload, String> {
    serde_json::from_value(command.payload.clone())
        .map_err(|error| format!("invalid create payload: {error}"))
}

fn build_create_proposal_report(
    root: &str,
    payload: &CreateCommandPayload,
) -> Result<ProposalReport, String> {
    let is_directory = match payload.kind.as_str() {
        "DIRECTORY" => true,
        "FILE" => false,
        _ => return Err("CREATE kind must be FILE or DIRECTORY".to_string()),
    };
    if is_directory && payload.content.is_some() {
        return Err("CREATE DIRECTORY must not include file content".to_string());
    }
    if !is_directory && payload.content.as_deref().unwrap_or_default() != "" {
        return Err("CREATE FILE currently supports empty files only".to_string());
    }

    let relative_path = if is_directory {
        normalize_remote_directory_target(&payload.relative_path)?
    } else {
        normalize_remote_file_source(&payload.relative_path)?
    };
    let parent = parent_relative_directory(&relative_path);
    let guard = PathGuard::new(root).map_err(|error| error.to_string())?;
    ensure_existing_directory_is_safe(&guard, parent)?;
    let target = guard.root().join(&relative_path);
    let status = if target.exists() {
        ProposalStatus::DestinationExists
    } else {
        ProposalStatus::Ready
    };
    let action = if is_directory {
        ProposalAction::CreateDir
    } else {
        ProposalAction::CreateFile
    };

    Ok(ProposalReport {
        root: guard.root().display().to_string(),
        proposals: vec![Proposal {
            proposal_id: proposal_id(&action, "", &relative_path),
            action,
            from: String::new(),
            to: relative_path,
            content: None,
            source_size_bytes: 0,
            source_modified_unix_ms: None,
            source_file_id: None,
            reason: if is_directory {
                USER_REQUESTED_CREATE_DIR_REASON.to_string()
            } else {
                USER_REQUESTED_CREATE_FILE_REASON.to_string()
            },
            status,
        }],
    })
}

fn build_trash_proposal_report(
    root: &str,
    payload: &TrashCommandPayload,
) -> Result<ProposalReport, String> {
    if payload.source_relative_paths.is_empty() || payload.source_relative_paths.len() > 200 {
        return Err("TRASH sourceRelativePaths must contain 1 to 200 paths".to_string());
    }

    let guard = PathGuard::new(root).map_err(|error| error.to_string())?;
    let mut proposals = Vec::with_capacity(payload.source_relative_paths.len());
    for source_relative_path in &payload.source_relative_paths {
        let source_relative_path = normalize_remote_file_source(source_relative_path)?;
        let metadata = inspect_regular_source_file(&guard, &source_relative_path, TRASH)?;
        let action = ProposalAction::Trash;
        proposals.push(Proposal {
            proposal_id: proposal_id(&action, &source_relative_path, ".mousekeeper_trash"),
            action,
            from: source_relative_path.clone(),
            to: ".mousekeeper_trash".to_string(),
            content: None,
            source_size_bytes: metadata.len(),
            source_modified_unix_ms: modified_unix_ms(&metadata),
            source_file_id: source_file_id_for_path(&guard, &source_relative_path),
            reason: USER_REQUESTED_TRASH_REASON.to_string(),
            status: ProposalStatus::Ready,
        });
    }

    Ok(ProposalReport {
        root: guard.root().display().to_string(),
        proposals,
    })
}

fn build_move_proposal_report(
    root: &str,
    payload: &MoveCommandPayload,
) -> Result<ProposalReport, String> {
    if payload.source_relative_paths.is_empty() || payload.source_relative_paths.len() > 200 {
        return Err("MOVE sourceRelativePaths must contain 1 to 200 paths".to_string());
    }
    let destination_directory =
        normalize_remote_relative_directory(&payload.destination_relative_directory)?;
    let guard = PathGuard::new(root).map_err(|error| error.to_string())?;
    ensure_destination_directory_is_safe(&guard, &destination_directory)?;

    let mut proposals = Vec::with_capacity(payload.source_relative_paths.len());
    for source_relative_path in &payload.source_relative_paths {
        let source_relative_path = normalize_remote_file_source(source_relative_path)?;
        let file_name = source_relative_path
            .rsplit_once('/')
            .map(|(_, name)| name)
            .unwrap_or(source_relative_path.as_str());
        let destination_relative_path = if destination_directory.is_empty() {
            file_name.to_string()
        } else {
            format!("{destination_directory}/{file_name}")
        };
        if source_relative_path == destination_relative_path {
            return Err("MOVE destination must differ from the source path".to_string());
        }
        let proposal = build_single_move_proposal(
            &guard,
            &source_relative_path,
            &destination_relative_path,
            USER_REQUESTED_MOVE_REASON,
        )?;
        proposals.push(proposal);
    }

    Ok(ProposalReport {
        root: guard.root().display().to_string(),
        proposals,
    })
}

fn build_rename_proposal_report(
    root: &str,
    payload: &RenameCommandPayload,
) -> Result<ProposalReport, String> {
    let source_relative_path = normalize_remote_file_source(&payload.source_relative_path)?;
    let new_name = validate_remote_file_name(&payload.new_name)?;
    let destination_relative_path = sibling_relative_path(&source_relative_path, &new_name);
    if source_relative_path == destination_relative_path {
        return Err("rename newName must differ from the source file name".to_string());
    }

    let guard = PathGuard::new(root).map_err(|error| error.to_string())?;
    let proposal = build_single_move_proposal(
        &guard,
        &source_relative_path,
        &destination_relative_path,
        USER_REQUESTED_RENAME_REASON,
    )?;

    Ok(ProposalReport {
        root: guard.root().display().to_string(),
        proposals: vec![proposal],
    })
}

fn build_single_move_proposal(
    guard: &PathGuard,
    source_relative_path: &str,
    destination_relative_path: &str,
    reason: &str,
) -> Result<Proposal, String> {
    let metadata = inspect_regular_source_file(guard, source_relative_path, "MOVE")?;

    let destination = guard.root().join(destination_relative_path);
    let status = if destination.exists() {
        ProposalStatus::DestinationExists
    } else {
        ProposalStatus::Ready
    };
    let action = ProposalAction::Move;

    Ok(Proposal {
        proposal_id: proposal_id(&action, source_relative_path, destination_relative_path),
        action,
        from: source_relative_path.to_string(),
        to: destination_relative_path.to_string(),
        content: None,
        source_size_bytes: metadata.len(),
        source_modified_unix_ms: modified_unix_ms(&metadata),
        source_file_id: source_file_id_for_path(guard, source_relative_path),
        reason: reason.to_string(),
        status,
    })
}

fn inspect_regular_source_file(
    guard: &PathGuard,
    source_relative_path: &str,
    intent: &str,
) -> Result<fs::Metadata, String> {
    let source = guard
        .resolve_existing(source_relative_path)
        .map_err(|error| error.to_string())?;
    let raw_source = guard.root().join(source_relative_path);
    let metadata = fs::symlink_metadata(&raw_source).map_err(|error| {
        format!(
            "cannot read source metadata {}: {error}",
            raw_source.display()
        )
    })?;
    if is_link_or_reparse_point(&metadata, metadata.file_type()) {
        return Err(format!(
            "{intent} refuses symlinks, junctions, and reparse points"
        ));
    }
    if !metadata.is_file() {
        return Err(format!("{intent} currently supports regular files only"));
    }
    if !source.starts_with(guard.root()) {
        return Err(format!("{intent} source escaped managed root"));
    }
    Ok(metadata)
}

fn source_file_id_for_path(guard: &PathGuard, source_relative_path: &str) -> Option<String> {
    guard
        .resolve_existing(source_relative_path)
        .ok()
        .and_then(|path| file_engine_cli::file_identity::file_id_for_path(&path))
}

/// Computes the local proposal for a command. If the command carries an AI/server rule draft, the
/// draft is treated as untrusted input: it is strictly parsed and validated here before any file
/// access, so malformed AI output is rejected before the engine reads the disk. The deterministic
/// Rust engine — never the AI — computes the concrete file targets. Without a draft, the root's own
/// saved rules are used.
fn resolve_proposal(root: &str, payload: &OrganizeFilesPayload) -> Result<ProposalReport, String> {
    match &payload.rule_draft {
        Some(draft) => {
            let rule_set: RuleSet = serde_json::from_value(draft.clone())
                .map_err(|error| format!("invalid AI rule draft shape: {error}"))?;
            propose_for_root_with_rule_set(root, rule_set).map_err(|error| error.to_string())
        }
        None => propose_for_root(root).map_err(|error| error.to_string()),
    }
}

pub fn build_agent_proposal_submission(
    command: &AgentCommand,
    report: ProposalReport,
    max_proposals: usize,
) -> Result<AgentProposalSubmission, String> {
    if !(1..=200).contains(&max_proposals) {
        return Err("organize_files.maxProposals must be between 1 and 200".to_string());
    }

    let items = report
        .proposals
        .iter()
        .take(max_proposals)
        .enumerate()
        .map(|(index, proposal)| proposal_item(index, proposal))
        .collect::<Vec<_>>();

    Ok(AgentProposalSubmission {
        command_id: command.command_id.clone(),
        room_id: command.room_id.clone(),
        summary: AgentProposalSummary {
            item_count: items.len(),
            readme_draft: None,
            readme_diff: None,
        },
        expires_at: None,
        items,
    })
}

fn proposal_item(item_order: usize, proposal: &Proposal) -> AgentProposalItem {
    // The local engine only produces Move and Trash proposals, so the submission side can only
    // ever emit MOVE or QUARANTINE. Create/write actions have no local `ProposalAction`, so the
    // desktop can never submit an arbitrary delegated write; the match below is exhaustive.
    let action_type = match proposal.action {
        ProposalAction::Move => AgentProposalActionType::Move,
        ProposalAction::Trash => AgentProposalActionType::Quarantine,
        ProposalAction::CreateDir => AgentProposalActionType::CreateDir,
        ProposalAction::CreateFile => AgentProposalActionType::CreateFile,
        ProposalAction::ReadmeWrite => AgentProposalActionType::ReadmeWrite,
    };
    let conflict_state = match proposal.status {
        ProposalStatus::Ready => AgentProposalConflictState::None,
        ProposalStatus::DestinationExists => AgentProposalConflictState::NameConflict,
    };
    let mut precondition = serde_json::json!({
        "proposalId": proposal.proposal_id,
        "sourceSizeBytes": proposal.source_size_bytes,
        "sourceModifiedUnixMs": proposal.source_modified_unix_ms,
        "checkedAtUnixMs": unix_ms()
    });
    if let Some(source_file_id) = &proposal.source_file_id {
        precondition["sourceFileId"] = serde_json::json!(source_file_id);
    }

    AgentProposalItem {
        item_order,
        action_type,
        source_relative_path: match proposal.action {
            ProposalAction::Move | ProposalAction::Trash => Some(proposal.from.clone()),
            ProposalAction::CreateDir
            | ProposalAction::CreateFile
            | ProposalAction::ReadmeWrite => None,
        },
        destination_relative_path: match proposal.action {
            ProposalAction::Move => Some(proposal.to.clone()),
            ProposalAction::Trash => None,
            ProposalAction::CreateDir | ProposalAction::CreateFile => Some(proposal.to.clone()),
            ProposalAction::ReadmeWrite => Some("README.md".to_string()),
        },
        reason_code: reason_code(proposal),
        precondition,
        conflict_state,
    }
}

fn reason_code(proposal: &Proposal) -> String {
    if proposal.reason == USER_REQUESTED_RENAME_REASON {
        return USER_REQUESTED_RENAME_REASON.to_string();
    }
    if proposal.reason == USER_REQUESTED_MOVE_REASON {
        return USER_REQUESTED_MOVE_REASON.to_string();
    }
    if proposal.reason == USER_REQUESTED_TRASH_REASON {
        return USER_REQUESTED_TRASH_REASON.to_string();
    }
    if proposal.reason == USER_REQUESTED_CREATE_DIR_REASON {
        return USER_REQUESTED_CREATE_DIR_REASON.to_string();
    }
    if proposal.reason == USER_REQUESTED_CREATE_FILE_REASON {
        return USER_REQUESTED_CREATE_FILE_REASON.to_string();
    }

    match proposal.action {
        ProposalAction::Move => "RULE_MOVE_BY_EXTENSION",
        ProposalAction::Trash => "RULE_QUARANTINE",
        ProposalAction::CreateDir => "RULE_CREATE_DIR",
        ProposalAction::CreateFile => "CREATE_FILE",
        ProposalAction::ReadmeWrite => "README_WRITE",
    }
    .to_string()
}

fn parse_organize_payload(command: &AgentCommand) -> Result<OrganizeFilesPayload, String> {
    let payload: OrganizeFilesPayload = if command.command_type == ANALYZE {
        if !command.payload.is_object()
            || command
                .payload
                .as_object()
                .is_some_and(|payload| !payload.is_empty())
        {
            return Err("ANALYZE payload must be an empty object".to_string());
        }
        OrganizeFilesPayload {
            root_id: None,
            relative_path: None,
            scope_relative_path: None,
            max_proposals: Some(DEFAULT_MAX_PROPOSALS),
            rule_mode: None,
            user_intent: None,
            rule_draft: None,
        }
    } else {
        serde_json::from_value(command.payload.clone())
            .map_err(|error| format!("invalid organize_files payload: {error}"))?
    };
    if !(1..=200).contains(&payload.max_proposals()) {
        return Err("organize_files.maxProposals must be between 1 and 200".to_string());
    }
    if let Some(rule_mode) = payload.rule_mode.as_deref() {
        if !matches!(rule_mode, "default" | "managed_root_rules") {
            return Err("organize_files.ruleMode is not supported".to_string());
        }
    }
    if payload
        .user_intent
        .as_ref()
        .is_some_and(|intent| intent.chars().count() > 2000)
    {
        return Err("organize_files.userIntent is too long".to_string());
    }
    Ok(payload)
}

fn validate_relative_scope(path: &str) -> Result<(), String> {
    if !path.is_empty() {
        return Err(
            "organize_files.relativePath is not supported until scoped analysis is implemented"
                .to_string(),
        );
    }
    if path.contains('\\')
        || path.starts_with('/')
        || path.starts_with("\\\\")
        || path.contains(':')
        || path.split('/').any(|part| part == "..")
    {
        return Err("organize_files.relativePath must stay inside the managed root".to_string());
    }
    Ok(())
}

fn normalize_remote_file_source(path: &str) -> Result<String, String> {
    validate_remote_relative_path(path)?;
    Ok(normalize_slashes(path))
}

fn validate_remote_relative_path(path: &str) -> Result<(), String> {
    if path.trim().is_empty()
        || path.contains('\\')
        || path.starts_with('/')
        || path.starts_with("\\\\")
        || path.contains(':')
        || path.contains('\0')
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err("remote command path must stay inside the managed root".to_string());
    }
    Ok(())
}

fn normalize_remote_relative_directory(path: &str) -> Result<String, String> {
    let trimmed = path.trim().trim_matches('/');
    if trimmed.is_empty() {
        return Ok(String::new());
    }
    validate_remote_relative_path(trimmed)?;
    Ok(normalize_slashes(trimmed))
}

fn normalize_remote_directory_target(path: &str) -> Result<String, String> {
    let normalized = normalize_remote_relative_directory(path)?;
    if normalized.is_empty() {
        return Err("CREATE_DIR target must not be the managed root".to_string());
    }
    Ok(normalized)
}

fn parent_relative_directory(path: &str) -> &str {
    path.rsplit_once('/')
        .map(|(parent, _)| parent)
        .unwrap_or("")
}

fn ensure_existing_directory_is_safe(
    guard: &PathGuard,
    relative_directory: &str,
) -> Result<(), String> {
    if relative_directory.is_empty() {
        return Ok(());
    }
    let resolved = guard
        .resolve_existing(relative_directory)
        .map_err(|error| error.to_string())?;
    let metadata = fs::symlink_metadata(guard.root().join(relative_directory))
        .map_err(|error| format!("cannot inspect destination parent: {error}"))?;
    if is_link_or_reparse_point(&metadata, metadata.file_type()) {
        return Err(
            "CREATE_DIR parent refuses symlinks, junctions, and reparse points".to_string(),
        );
    }
    if !metadata.is_dir() {
        return Err("CREATE_DIR parent is not a directory".to_string());
    }
    if !resolved.starts_with(guard.root()) {
        return Err("CREATE_DIR parent escaped managed root".to_string());
    }
    Ok(())
}

fn ensure_destination_directory_is_safe(
    guard: &PathGuard,
    relative_directory: &str,
) -> Result<(), String> {
    let mut current = guard.root().to_path_buf();
    for segment in relative_directory
        .split('/')
        .filter(|segment| !segment.is_empty())
    {
        current.push(segment);
        if !current.exists() {
            continue;
        }
        let metadata = fs::symlink_metadata(&current)
            .map_err(|error| format!("cannot inspect destination directory: {error}"))?;
        if is_link_or_reparse_point(&metadata, metadata.file_type()) {
            return Err(
                "MOVE destination refuses symlinks, junctions, and reparse points".to_string(),
            );
        }
        if !metadata.is_dir() {
            return Err("MOVE destination path contains a non-directory entry".to_string());
        }
        let resolved = current
            .canonicalize()
            .map_err(|error| format!("cannot canonicalize destination directory: {error}"))?;
        if !resolved.starts_with(guard.root()) {
            return Err("MOVE destination escaped managed root".to_string());
        }
    }
    Ok(())
}

fn validate_remote_file_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    let stem = trimmed
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    let reserved = matches!(
        stem.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    );
    if trimmed.is_empty()
        || trimmed == "."
        || trimmed == ".."
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed.contains('\0')
        || trimmed.ends_with('.')
        || trimmed.ends_with(' ')
        || reserved
    {
        return Err("remote rename newName is not a safe file name".to_string());
    }
    Ok(trimmed.to_string())
}

fn sibling_relative_path(source_relative_path: &str, new_name: &str) -> String {
    let source = normalize_slashes(source_relative_path);
    source
        .rsplit_once('/')
        .map(|(parent, _)| format!("{parent}/{new_name}"))
        .unwrap_or_else(|| new_name.to_string())
}

fn normalize_slashes(path: &str) -> String {
    Path::new(path)
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn modified_unix_ms(metadata: &fs::Metadata) -> Option<u128> {
    metadata
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_millis())
}

fn is_supported_command(command: &AgentCommand) -> bool {
    matches!(
        command.command_type.as_str(),
        ANALYZE | RENAME | MOVE | TRASH | CREATE | ORGANIZE | ORGANIZE_FILES | ORGANIZE_ROOT
    )
}

fn is_contract_command_without_desktop_handler(command: &AgentCommand) -> bool {
    matches!(command.command_type.as_str(), FIND | DOWNLOAD | UPLOAD)
}

fn unix_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use file_engine_cli::proposal::{Proposal, ProposalAction, ProposalReport, ProposalStatus};
    use serde_json::json;

    use crate::agent::AgentRuntime;
    use crate::storage::managed_roots::ManagedRootStore;
    use crate::storage::outbox::OutboxStore;

    use super::{
        build_agent_proposal_submission, build_create_proposal_report, build_move_proposal_report,
        build_rename_proposal_report, build_trash_proposal_report, parse_organize_payload,
        process_commands, resolve_proposal, validate_relative_scope, AgentCommand,
        AgentProposalActionType, AgentProposalConflictState, CommandProcessingStatus,
        CreateCommandPayload, MoveCommandPayload, OrganizeFilesPayload, RenameCommandPayload,
        TrashCommandPayload, USER_REQUESTED_CREATE_DIR_REASON, USER_REQUESTED_CREATE_FILE_REASON,
        USER_REQUESTED_MOVE_REASON, USER_REQUESTED_RENAME_REASON, USER_REQUESTED_TRASH_REASON,
    };

    fn command(payload: serde_json::Value) -> AgentCommand {
        AgentCommand {
            command_id: "command-1".to_string(),
            command_type: "organize_files".to_string(),
            room_id: "room-1".to_string(),
            status: "QUEUED".to_string(),
            payload,
        }
    }

    fn organize_payload(rule_draft: Option<serde_json::Value>) -> OrganizeFilesPayload {
        OrganizeFilesPayload {
            root_id: None,
            relative_path: None,
            scope_relative_path: None,
            max_proposals: Some(50),
            rule_mode: None,
            user_intent: None,
            rule_draft,
        }
    }

    #[test]
    fn ai_rule_draft_produces_a_deterministic_proposal() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("root");
        std::fs::create_dir_all(root.join("downloads")).expect("downloads");
        std::fs::write(root.join("downloads/old.pdf"), "pdf").expect("pdf");
        std::fs::write(root.join("downloads/keep.txt"), "txt").expect("txt");

        // The validated rule-draft shape an AI "clean up PDFs" request would translate to.
        let payload = organize_payload(Some(json!({
            "version": 1,
            "rules": [
                { "id": "cleanup-pdfs", "when": { "extension_in": ["pdf"] }, "then": { "trash": true } }
            ]
        })));

        let report = resolve_proposal(&root.to_string_lossy(), &payload).expect("draft proposal");
        let froms = report
            .proposals
            .iter()
            .map(|p| p.from.as_str())
            .collect::<Vec<_>>();
        assert_eq!(froms, vec!["downloads/old.pdf"]);
    }

    #[test]
    fn malformed_ai_rule_draft_is_rejected_before_file_logic() {
        // A non-existent root: if the draft shape were parsed after touching the disk this would
        // fail with a filesystem error. Instead it must fail at draft parsing.
        let payload = organize_payload(Some(json!({
            "version": 1,
            "rules": [{ "id": "x", "when": {}, "then": { "bogusField": true } }]
        })));

        let error = resolve_proposal("C:/definitely/missing/root", &payload)
            .expect_err("malformed draft rejected");
        assert!(
            error.contains("invalid AI rule draft shape"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn semantically_invalid_ai_rule_draft_is_rejected_before_file_logic() {
        // Well-formed JSON, but a rule with no condition means "match everything" — the engine's
        // validate() must reject it before analyzing the (here non-existent) root.
        let payload = organize_payload(Some(json!({
            "version": 1,
            "rules": [{ "id": "x", "when": {}, "then": { "trash": true } }]
        })));

        assert!(resolve_proposal("C:/definitely/missing/root", &payload).is_err());
    }

    #[test]
    fn validates_organize_payload_before_local_analysis() {
        let parsed = parse_organize_payload(&command(json!({
            "relativePath": "",
            "maxProposals": 10,
            "ruleMode": "managed_root_rules"
        })))
        .expect("payload");

        assert_eq!(parsed.max_proposals(), 10);

        let error = parse_organize_payload(&command(json!({
            "relativePath": "../outside",
            "maxProposals": 0
        })))
        .expect_err("reject maxProposals");
        assert!(error.contains("maxProposals"));
    }

    #[test]
    fn parses_rest_analyze_and_organize_intents_for_proposal_generation() {
        let analyze = parse_organize_payload(&AgentCommand {
            command_id: "command-1".to_string(),
            command_type: "ANALYZE".to_string(),
            room_id: "room-1".to_string(),
            status: "QUEUED".to_string(),
            payload: json!({}),
        })
        .expect("ANALYZE defaults");
        assert_eq!(analyze.scope(), "");
        assert_eq!(analyze.max_proposals(), 50);

        let organize = parse_organize_payload(&AgentCommand {
            command_id: "command-2".to_string(),
            command_type: "ORGANIZE".to_string(),
            room_id: "room-1".to_string(),
            status: "QUEUED".to_string(),
            payload: json!({
                "rootId": "root:downloads",
                "scopeRelativePath": "",
                "instruction": "clean up old pdfs"
            }),
        })
        .expect("ORGANIZE payload");
        assert_eq!(organize.root_id.as_deref(), Some("root:downloads"));
        assert_eq!(organize.scope(), "");
        assert_eq!(organize.max_proposals(), 50);
        assert_eq!(organize.user_intent.as_deref(), Some("clean up old pdfs"));
    }

    #[test]
    fn rename_command_builds_a_move_proposal_without_touching_files() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("root");
        std::fs::create_dir_all(root.join("notes")).expect("notes");
        std::fs::write(root.join("notes/old.txt"), "original").expect("old");

        let payload = RenameCommandPayload {
            root_id: "root:notes".to_string(),
            source_relative_path: "notes/old.txt".to_string(),
            new_name: "new.txt".to_string(),
        };

        let report = build_rename_proposal_report(&root.to_string_lossy(), &payload)
            .expect("rename proposal");

        assert!(root.join("notes/old.txt").exists());
        assert!(!root.join("notes/new.txt").exists());
        assert_eq!(report.proposals.len(), 1);
        let proposal = &report.proposals[0];
        assert_eq!(proposal.action, ProposalAction::Move);
        assert_eq!(proposal.from, "notes/old.txt");
        assert_eq!(proposal.to, "notes/new.txt");
        assert_eq!(proposal.source_size_bytes, 8);
        assert_eq!(proposal.reason, USER_REQUESTED_RENAME_REASON);
        assert_eq!(proposal.status, ProposalStatus::Ready);

        let submission =
            build_agent_proposal_submission(&command(json!({"maxProposals": 1})), report, 1)
                .expect("submission");
        assert_eq!(submission.summary.item_count, 1);
        assert_eq!(
            submission.items[0].reason_code,
            USER_REQUESTED_RENAME_REASON
        );
        assert_eq!(
            submission.items[0].conflict_state,
            AgentProposalConflictState::None
        );
    }

    #[test]
    fn rename_command_marks_destination_conflict_without_overwriting() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("root");
        std::fs::create_dir_all(root.join("notes")).expect("notes");
        std::fs::write(root.join("notes/old.txt"), "original").expect("old");
        std::fs::write(root.join("notes/new.txt"), "keep me").expect("new");

        let payload = RenameCommandPayload {
            root_id: "root:notes".to_string(),
            source_relative_path: "notes/old.txt".to_string(),
            new_name: "new.txt".to_string(),
        };

        let report = build_rename_proposal_report(&root.to_string_lossy(), &payload)
            .expect("rename proposal");
        let submission =
            build_agent_proposal_submission(&command(json!({"maxProposals": 1})), report, 1)
                .expect("submission");

        assert_eq!(
            std::fs::read_to_string(root.join("notes/new.txt")).expect("new"),
            "keep me"
        );
        assert_eq!(
            submission.items[0].conflict_state,
            AgentProposalConflictState::NameConflict
        );
    }

    #[test]
    fn rename_command_rejects_unsafe_source_paths_and_names() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("root");
        std::fs::create_dir_all(root.join("notes")).expect("notes");
        std::fs::write(root.join("notes/old.txt"), "original").expect("old");

        let traversal = RenameCommandPayload {
            root_id: "root:notes".to_string(),
            source_relative_path: "../outside.txt".to_string(),
            new_name: "new.txt".to_string(),
        };
        let error = build_rename_proposal_report(&root.to_string_lossy(), &traversal)
            .expect_err("reject traversal");
        assert!(error.contains("managed root"));

        let slash_name = RenameCommandPayload {
            root_id: "root:notes".to_string(),
            source_relative_path: "notes/old.txt".to_string(),
            new_name: "nested/new.txt".to_string(),
        };
        let error = build_rename_proposal_report(&root.to_string_lossy(), &slash_name)
            .expect_err("reject name");
        assert!(error.contains("safe file name"));
    }

    #[test]
    fn move_command_builds_one_proposal_per_file_without_touching_files() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("root");
        std::fs::create_dir_all(root.join("inbox")).expect("inbox");
        std::fs::write(root.join("inbox/a.txt"), "a").expect("a");
        std::fs::write(root.join("inbox/b.txt"), "bb").expect("b");

        let payload = MoveCommandPayload {
            root_id: "root:inbox".to_string(),
            source_relative_paths: vec!["inbox/a.txt".to_string(), "inbox/b.txt".to_string()],
            destination_relative_directory: "archive".to_string(),
        };

        let report =
            build_move_proposal_report(&root.to_string_lossy(), &payload).expect("move proposal");

        assert!(root.join("inbox/a.txt").exists());
        assert!(root.join("inbox/b.txt").exists());
        assert!(!root.join("archive").exists());
        assert_eq!(report.proposals.len(), 2);
        assert_eq!(report.proposals[0].from, "inbox/a.txt");
        assert_eq!(report.proposals[0].to, "archive/a.txt");
        assert_eq!(report.proposals[0].reason, USER_REQUESTED_MOVE_REASON);
        assert_eq!(report.proposals[1].from, "inbox/b.txt");
        assert_eq!(report.proposals[1].to, "archive/b.txt");

        let submission =
            build_agent_proposal_submission(&command(json!({"maxProposals": 2})), report, 200)
                .expect("submission");
        assert_eq!(submission.summary.item_count, 2);
        assert!(submission
            .items
            .iter()
            .all(|item| item.reason_code == USER_REQUESTED_MOVE_REASON));
    }

    #[test]
    fn move_command_marks_destination_conflicts_per_file() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("root");
        std::fs::create_dir_all(root.join("inbox")).expect("inbox");
        std::fs::create_dir_all(root.join("archive")).expect("archive");
        std::fs::write(root.join("inbox/a.txt"), "a").expect("a");
        std::fs::write(root.join("archive/a.txt"), "existing").expect("existing");

        let payload = MoveCommandPayload {
            root_id: "root:inbox".to_string(),
            source_relative_paths: vec!["inbox/a.txt".to_string()],
            destination_relative_directory: "archive".to_string(),
        };

        let report =
            build_move_proposal_report(&root.to_string_lossy(), &payload).expect("move proposal");
        let submission =
            build_agent_proposal_submission(&command(json!({"maxProposals": 1})), report, 200)
                .expect("submission");

        assert_eq!(
            std::fs::read_to_string(root.join("archive/a.txt")).expect("existing"),
            "existing"
        );
        assert_eq!(
            submission.items[0].conflict_state,
            AgentProposalConflictState::NameConflict
        );
    }

    #[test]
    fn move_command_rejects_unsafe_paths() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("root");
        std::fs::create_dir_all(root.join("inbox")).expect("inbox");
        std::fs::write(root.join("inbox/a.txt"), "a").expect("a");

        let traversal = MoveCommandPayload {
            root_id: "root:inbox".to_string(),
            source_relative_paths: vec!["inbox/../outside.txt".to_string()],
            destination_relative_directory: "archive".to_string(),
        };
        let error = build_move_proposal_report(&root.to_string_lossy(), &traversal)
            .expect_err("reject traversal");
        assert!(error.contains("managed root"));

        let unsafe_destination = MoveCommandPayload {
            root_id: "root:inbox".to_string(),
            source_relative_paths: vec!["inbox/a.txt".to_string()],
            destination_relative_directory: "../archive".to_string(),
        };
        let error = build_move_proposal_report(&root.to_string_lossy(), &unsafe_destination)
            .expect_err("reject destination");
        assert!(error.contains("managed root"));
    }

    #[test]
    fn trash_command_builds_quarantine_proposals_without_touching_files() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("root");
        std::fs::create_dir_all(root.join("inbox")).expect("inbox");
        std::fs::write(root.join("inbox/a.tmp"), "a").expect("a");
        std::fs::write(root.join("inbox/b.tmp"), "bb").expect("b");

        let payload = TrashCommandPayload {
            root_id: "root:inbox".to_string(),
            source_relative_paths: vec!["inbox/a.tmp".to_string(), "inbox/b.tmp".to_string()],
        };

        let report =
            build_trash_proposal_report(&root.to_string_lossy(), &payload).expect("trash proposal");

        assert!(root.join("inbox/a.tmp").exists());
        assert!(root.join("inbox/b.tmp").exists());
        assert!(!root.join(".mousekeeper_trash").exists());
        assert_eq!(report.proposals.len(), 2);
        assert_eq!(report.proposals[0].action, ProposalAction::Trash);
        assert_eq!(report.proposals[0].from, "inbox/a.tmp");
        assert_eq!(report.proposals[0].to, ".mousekeeper_trash");
        assert_eq!(report.proposals[0].reason, USER_REQUESTED_TRASH_REASON);

        let submission =
            build_agent_proposal_submission(&command(json!({"maxProposals": 2})), report, 200)
                .expect("submission");
        assert_eq!(submission.summary.item_count, 2);
        assert_eq!(
            submission.items[0].action_type,
            AgentProposalActionType::Quarantine
        );
        assert!(submission
            .items
            .iter()
            .all(|item| item.reason_code == USER_REQUESTED_TRASH_REASON));
    }

    #[test]
    fn trash_command_rejects_unsafe_paths_and_directories() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("root");
        std::fs::create_dir_all(root.join("inbox/dir")).expect("dir");
        std::fs::write(root.join("inbox/a.tmp"), "a").expect("a");

        let traversal = TrashCommandPayload {
            root_id: "root:inbox".to_string(),
            source_relative_paths: vec!["../outside.tmp".to_string()],
        };
        let error = build_trash_proposal_report(&root.to_string_lossy(), &traversal)
            .expect_err("reject traversal");
        assert!(error.contains("managed root"));

        let directory = TrashCommandPayload {
            root_id: "root:inbox".to_string(),
            source_relative_paths: vec!["inbox/dir".to_string()],
        };
        let error = build_trash_proposal_report(&root.to_string_lossy(), &directory)
            .expect_err("reject directory");
        assert!(error.contains("regular files only"));
    }

    #[test]
    fn create_directory_command_builds_a_create_dir_proposal_without_touching_files() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("root");
        std::fs::create_dir_all(root.join("archive")).expect("archive parent");

        let payload = CreateCommandPayload {
            root_id: "root:archive".to_string(),
            kind: "DIRECTORY".to_string(),
            relative_path: "archive/reports".to_string(),
            content: None,
        };

        let report = build_create_proposal_report(&root.to_string_lossy(), &payload)
            .expect("create dir proposal");

        assert!(!root.join("archive/reports").exists());
        assert_eq!(report.proposals.len(), 1);
        let proposal = &report.proposals[0];
        assert_eq!(proposal.action, ProposalAction::CreateDir);
        assert_eq!(proposal.from, "");
        assert_eq!(proposal.to, "archive/reports");
        assert_eq!(proposal.reason, USER_REQUESTED_CREATE_DIR_REASON);
        assert_eq!(proposal.status, ProposalStatus::Ready);

        let submission =
            build_agent_proposal_submission(&command(json!({"maxProposals": 1})), report, 1)
                .expect("submission");
        assert_eq!(submission.summary.item_count, 1);
        assert_eq!(
            submission.items[0].action_type,
            AgentProposalActionType::CreateDir
        );
        assert_eq!(submission.items[0].source_relative_path, None);
        assert_eq!(
            submission.items[0].destination_relative_path.as_deref(),
            Some("archive/reports")
        );
        assert_eq!(
            submission.items[0].reason_code,
            USER_REQUESTED_CREATE_DIR_REASON
        );
    }

    #[test]
    fn create_directory_command_marks_existing_directory_as_conflict() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("root");
        std::fs::create_dir_all(root.join("archive/reports")).expect("existing");

        let payload = CreateCommandPayload {
            root_id: "root:archive".to_string(),
            kind: "DIRECTORY".to_string(),
            relative_path: "archive/reports".to_string(),
            content: None,
        };

        let report = build_create_proposal_report(&root.to_string_lossy(), &payload)
            .expect("create dir proposal");
        let submission =
            build_agent_proposal_submission(&command(json!({"maxProposals": 1})), report, 1)
                .expect("submission");

        assert_eq!(
            submission.items[0].conflict_state,
            AgentProposalConflictState::NameConflict
        );
    }

    #[test]
    fn create_directory_command_rejects_files_content_and_missing_parent() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("root");
        std::fs::create_dir_all(&root).expect("root");

        let with_content = CreateCommandPayload {
            root_id: "root:archive".to_string(),
            kind: "DIRECTORY".to_string(),
            relative_path: "reports".to_string(),
            content: Some("nope".to_string()),
        };
        let error = build_create_proposal_report(&root.to_string_lossy(), &with_content)
            .expect_err("reject content");
        assert!(error.contains("must not include file content"));

        let missing_parent = CreateCommandPayload {
            root_id: "root:archive".to_string(),
            kind: "DIRECTORY".to_string(),
            relative_path: "missing/reports".to_string(),
            content: None,
        };
        let error = build_create_proposal_report(&root.to_string_lossy(), &missing_parent)
            .expect_err("reject missing parent");
        assert!(error.contains("does not exist"));
    }

    #[test]
    fn create_file_command_builds_an_empty_file_proposal_without_touching_files() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("root");
        std::fs::create_dir_all(root.join("notes")).expect("notes parent");

        let payload = CreateCommandPayload {
            root_id: "root:notes".to_string(),
            kind: "FILE".to_string(),
            relative_path: "notes/todo.txt".to_string(),
            content: None,
        };

        let report =
            build_create_proposal_report(&root.to_string_lossy(), &payload).expect("create file");

        assert!(!root.join("notes/todo.txt").exists());
        assert_eq!(report.proposals[0].action, ProposalAction::CreateFile);
        assert_eq!(report.proposals[0].from, "");
        assert_eq!(report.proposals[0].to, "notes/todo.txt");
        assert_eq!(
            report.proposals[0].reason,
            USER_REQUESTED_CREATE_FILE_REASON
        );

        let submission =
            build_agent_proposal_submission(&command(json!({"maxProposals": 1})), report, 1)
                .expect("submission");
        assert_eq!(
            submission.items[0].action_type,
            AgentProposalActionType::CreateFile
        );
        assert_eq!(submission.items[0].source_relative_path, None);
        assert_eq!(
            submission.items[0].destination_relative_path.as_deref(),
            Some("notes/todo.txt")
        );
    }

    #[test]
    fn create_file_command_rejects_non_empty_content() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("root");
        std::fs::create_dir_all(root.join("notes")).expect("notes parent");

        let payload = CreateCommandPayload {
            root_id: "root:notes".to_string(),
            kind: "FILE".to_string(),
            relative_path: "notes/todo.txt".to_string(),
            content: Some("write me".to_string()),
        };

        let error = build_create_proposal_report(&root.to_string_lossy(), &payload)
            .expect_err("reject content");
        assert!(error.contains("empty files only"));
    }

    #[test]
    fn analyze_rejects_non_empty_payloads() {
        let error = parse_organize_payload(&AgentCommand {
            command_id: "command-1".to_string(),
            command_type: "ANALYZE".to_string(),
            room_id: "room-1".to_string(),
            status: "QUEUED".to_string(),
            payload: json!({"shell": "nope"}),
        })
        .expect_err("ANALYZE stays empty");

        assert!(error.contains("empty object"));
    }

    #[test]
    fn rejects_scoped_relative_path_until_partial_analysis_exists() {
        let error =
            validate_relative_scope("inbox").expect_err("scoped command must be explicit failure");

        assert!(error.contains("scoped analysis"));
    }

    #[tokio::test]
    async fn skips_remote_commands_that_look_like_direct_file_tools() {
        let runtime = AgentRuntime::default();
        let roots = ManagedRootStore::default();
        let commands = vec![
            AgentCommand {
                command_id: "command-1".to_string(),
                command_type: "rename_file".to_string(),
                room_id: "room-1".to_string(),
                status: "QUEUED".to_string(),
                payload: json!({"path": "notes/a.txt", "newName": "b.txt"}),
            },
            AgentCommand {
                command_id: "command-2".to_string(),
                command_type: "create_file".to_string(),
                room_id: "room-1".to_string(),
                status: "QUEUED".to_string(),
                payload: json!({"path": "notes/new.txt"}),
            },
            AgentCommand {
                command_id: "command-3".to_string(),
                command_type: "trash_file".to_string(),
                room_id: "room-1".to_string(),
                status: "QUEUED".to_string(),
                payload: json!({"path": "notes/old.txt"}),
            },
        ];

        // All three are skipped before any network or outbox use, so an uninitialized outbox is
        // never touched.
        let outbox = OutboxStore::default();
        let report = process_commands(&runtime, &roots, &outbox, commands)
            .await
            .expect("unsupported direct commands are skipped before runtime calls");

        assert_eq!(report.processed_count, 0);
        assert_eq!(report.skipped_count, 3);
        assert_eq!(report.submitted_proposal_count, 0);
    }

    #[tokio::test]
    async fn fails_contract_commands_that_are_not_wired_yet() {
        let runtime = AgentRuntime::default();
        let roots = ManagedRootStore::default();
        let temp = tempfile::tempdir().expect("tempdir");
        let outbox = OutboxStore::default();
        outbox
            .load_from_db(temp.path().join("outbox.db"))
            .expect("outbox");
        let commands = vec![
            AgentCommand {
                command_id: "find-command".to_string(),
                command_type: "FIND".to_string(),
                room_id: "room-1".to_string(),
                status: "QUEUED".to_string(),
                payload: json!({
                    "rootId": "root:docs",
                    "query": "report",
                    "limit": 20
                }),
            },
            AgentCommand {
                command_id: "download-command".to_string(),
                command_type: "DOWNLOAD".to_string(),
                room_id: "room-1".to_string(),
                status: "QUEUED".to_string(),
                payload: json!({
                    "rootId": "root:docs",
                    "sourceRelativePath": "reports/final.pdf"
                }),
            },
            AgentCommand {
                command_id: "upload-command".to_string(),
                command_type: "UPLOAD".to_string(),
                room_id: "room-1".to_string(),
                status: "QUEUED".to_string(),
                payload: json!({
                    "rootId": "root:docs",
                    "destinationRelativePath": "incoming/photo.png",
                    "transferId": "00000000-0000-4000-8000-000000000001",
                    "expectedSha256": "a".repeat(64),
                    "expectedSize": 10
                }),
            },
        ];

        let report = process_commands(&runtime, &roots, &outbox, commands)
            .await
            .expect("unwired commands fail visibly");

        assert_eq!(report.processed_count, 3);
        assert_eq!(report.failed_count, 3);
        assert_eq!(report.skipped_count, 0);
        assert!(report
            .results
            .iter()
            .all(|result| result.status == CommandProcessingStatus::Failed));
        let batch = outbox.pending_batch(10).expect("pending status rows");
        assert_eq!(batch.len(), 3);
        assert!(batch
            .iter()
            .all(|item| item.kind == "command_status" && item.payload_json.contains("FAILED")));
    }

    #[test]
    fn maps_local_move_and_trash_proposals_to_server_items() {
        let report = ProposalReport {
            root: "C:/root".to_string(),
            proposals: vec![
                Proposal {
                    proposal_id: "move:0000000000000001".to_string(),
                    action: ProposalAction::Move,
                    from: "inbox/note.md".to_string(),
                    to: "documents/note.md".to_string(),
                    content: None,
                    source_size_bytes: 12,
                    source_modified_unix_ms: Some(10),
                    source_file_id: Some("win:v00000001:i0000000000000001".to_string()),
                    reason: "move note".to_string(),
                    status: ProposalStatus::Ready,
                },
                Proposal {
                    proposal_id: "trash:0000000000000002".to_string(),
                    action: ProposalAction::Trash,
                    from: "inbox/noise.tmp".to_string(),
                    to: ".mousekeeper_trash/noise.tmp".to_string(),
                    content: None,
                    source_size_bytes: 3,
                    source_modified_unix_ms: None,
                    source_file_id: None,
                    reason: "trash temp".to_string(),
                    status: ProposalStatus::DestinationExists,
                },
            ],
        };

        let submission =
            build_agent_proposal_submission(&command(json!({"maxProposals": 200})), report, 200)
                .expect("submission");

        assert_eq!(submission.summary.item_count, 2);
        assert_eq!(
            submission.items[0].action_type,
            AgentProposalActionType::Move
        );
        assert_eq!(
            submission.items[0].destination_relative_path.as_deref(),
            Some("documents/note.md")
        );
        assert_eq!(
            submission.items[0]
                .precondition
                .get("sourceFileId")
                .and_then(serde_json::Value::as_str),
            Some("win:v00000001:i0000000000000001")
        );
        assert_eq!(
            submission.items[1].action_type,
            AgentProposalActionType::Quarantine
        );
        assert_eq!(submission.items[1].destination_relative_path, None);
        assert_eq!(
            submission.items[1].conflict_state,
            AgentProposalConflictState::NameConflict
        );
    }
}
