use std::time::{SystemTime, UNIX_EPOCH};

use file_engine_cli::proposal::{
    propose_for_root, Proposal, ProposalAction, ProposalReport, ProposalStatus,
};
use serde::{Deserialize, Serialize};

use crate::agent::{AgentCommand, AgentRuntime};
use crate::outbox_processor::{enqueue_command_status, enqueue_proposal};
use crate::storage::managed_roots::ManagedRootStore;
use crate::storage::outbox::OutboxStore;

/// Boundary for delegated work from mobile/server/AI.
///
/// This module may generate proposal submissions, but it must not call direct file operations
/// such as create, rename, or trash. Those commands stay local/manual in `commands::file_engine`.
const ORGANIZE_FILES: &str = "organize_files";
const ORGANIZE_ROOT: &str = "ORGANIZE_ROOT";

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

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct OrganizeFilesPayload {
    #[serde(default)]
    relative_path: Option<String>,
    max_proposals: usize,
    #[serde(default)]
    rule_mode: Option<String>,
    #[serde(default)]
    user_intent: Option<String>,
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
        if !is_supported_organize_command(&command) || command.status != "QUEUED" {
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
        let result = process_organize_command(agent, roots, outbox, &command).await;
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

async fn process_organize_command(
    agent: &AgentRuntime,
    roots: &ManagedRootStore,
    outbox: &OutboxStore,
    command: &AgentCommand,
) -> Result<usize, String> {
    let payload = parse_organize_payload(command)?;
    validate_relative_scope(payload.relative_path.as_deref().unwrap_or_default())?;

    // ANALYZING stays a synchronous claim: it moves the command out of QUEUED so it is not polled
    // and re-processed again before the proposal is built. (Symmetric with the execution claim.)
    agent
        .update_command_status(command.command_id.clone(), "ANALYZING".to_string())
        .await
        .map_err(|error| error.to_string())?;

    let room = agent
        .root_id_for_room(command.room_id.clone())
        .await
        .map_err(|error| error.to_string())?;
    let managed_root = roots.get(&room.root_id)?;
    if !managed_root.enabled {
        return Err(format!(
            "managed root is disabled: {}",
            managed_root.root_id
        ));
    }

    let local_report = propose_for_root(&managed_root.root).map_err(|error| error.to_string())?;
    let submission = build_agent_proposal_submission(command, local_report, payload.max_proposals)?;
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
    };
    let conflict_state = match proposal.status {
        ProposalStatus::Ready => AgentProposalConflictState::None,
        ProposalStatus::DestinationExists => AgentProposalConflictState::NameConflict,
    };

    AgentProposalItem {
        item_order,
        action_type,
        source_relative_path: Some(proposal.from.clone()),
        destination_relative_path: match proposal.action {
            ProposalAction::Move => Some(proposal.to.clone()),
            ProposalAction::Trash => None,
        },
        reason_code: reason_code(proposal),
        precondition: serde_json::json!({
            "proposalId": proposal.proposal_id,
            "sourceSizeBytes": proposal.source_size_bytes,
            "sourceModifiedUnixMs": proposal.source_modified_unix_ms,
            "checkedAtUnixMs": unix_ms()
        }),
        conflict_state,
    }
}

fn reason_code(proposal: &Proposal) -> String {
    match proposal.action {
        ProposalAction::Move => "RULE_MOVE_BY_EXTENSION",
        ProposalAction::Trash => "RULE_QUARANTINE",
    }
    .to_string()
}

fn parse_organize_payload(command: &AgentCommand) -> Result<OrganizeFilesPayload, String> {
    let payload: OrganizeFilesPayload = serde_json::from_value(command.payload.clone())
        .map_err(|error| format!("invalid organize_files payload: {error}"))?;
    if !(1..=200).contains(&payload.max_proposals) {
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

fn is_supported_organize_command(command: &AgentCommand) -> bool {
    matches!(
        command.command_type.as_str(),
        ORGANIZE_FILES | ORGANIZE_ROOT
    )
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
        build_agent_proposal_submission, parse_organize_payload, process_commands,
        validate_relative_scope, AgentCommand, AgentProposalActionType, AgentProposalConflictState,
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

    #[test]
    fn validates_organize_payload_before_local_analysis() {
        let parsed = parse_organize_payload(&command(json!({
            "relativePath": "",
            "maxProposals": 10,
            "ruleMode": "managed_root_rules"
        })))
        .expect("payload");

        assert_eq!(parsed.max_proposals, 10);

        let error = parse_organize_payload(&command(json!({
            "relativePath": "../outside",
            "maxProposals": 0
        })))
        .expect_err("reject maxProposals");
        assert!(error.contains("maxProposals"));
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
                    source_size_bytes: 12,
                    source_modified_unix_ms: Some(10),
                    reason: "move note".to_string(),
                    status: ProposalStatus::Ready,
                },
                Proposal {
                    proposal_id: "trash:0000000000000002".to_string(),
                    action: ProposalAction::Trash,
                    from: "inbox/noise.tmp".to_string(),
                    to: ".housemouse_trash/noise.tmp".to_string(),
                    source_size_bytes: 3,
                    source_modified_unix_ms: None,
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
