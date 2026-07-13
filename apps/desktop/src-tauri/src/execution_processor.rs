//! Delegated execution boundary. This module intentionally never reads
//! `crate::storage::auto_approval::AutoApprovalStore` or calls
//! `file_engine_cli::auto_approval::auto_approve_decisions`: the local "auto approve proposals"
//! policy is a manual-tool convenience that only pre-checks decision checkboxes in the desktop
//! UI. Delegated work always executes strictly from an explicit `APPROVE` decision the server
//! already recorded (see `pending_decisions`/`create_execution`/`update_execution` on
//! `AgentRuntime`) — there is no path from a locally enabled auto-approval policy to a remote
//! proposal being executed.

use std::path::Path;

use file_engine_cli::decision::DecisionApplication;
use file_engine_cli::execute::{execute_decision_application, ExecuteReport};
use file_engine_cli::journal::TRASH_DIR;
use file_engine_cli::path_guard::PathGuard;
use file_engine_cli::proposal::{
    proposal_id, Proposal, ProposalAction, ProposalReport, ProposalStatus,
};
use serde::Serialize;
use serde_json::Value;

use crate::agent::{AgentPendingDecision, AgentProposalItemRecord, AgentRuntime};
use crate::outbox_processor::enqueue_execution_result;
use crate::storage::managed_roots::ManagedRootStore;
use crate::storage::outbox::OutboxStore;

/// Boundary for delegated execution. Turns a server-approved decision into a local
/// journal-before-write execution, then uploads the result. Never executes anything the server
/// has not recorded as an APPROVE decision, and never leaves a claimed execution without a
/// terminal result upload.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct DecisionProcessingReport {
    pub inspected_count: usize,
    pub processed_count: usize,
    pub executed_item_count: usize,
    pub skipped_item_count: usize,
    pub failed_count: usize,
    pub results: Vec<DecisionProcessingResult>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct DecisionProcessingResult {
    pub decision_id: String,
    pub proposal_id: String,
    pub status: DecisionProcessingStatus,
    pub message: Option<String>,
    pub executed_item_count: usize,
    pub skipped_item_count: usize,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DecisionProcessingStatus {
    Completed,
    Failed,
}

pub async fn process_pending_decisions(
    agent: &AgentRuntime,
    roots: &ManagedRootStore,
    outbox: &OutboxStore,
) -> Result<DecisionProcessingReport, String> {
    let decisions = agent
        .pending_decisions()
        .await
        .map_err(|error| error.to_string())?;

    let mut report = DecisionProcessingReport {
        inspected_count: decisions.len(),
        processed_count: 0,
        executed_item_count: 0,
        skipped_item_count: 0,
        failed_count: 0,
        results: Vec::new(),
    };

    for decision in decisions {
        let decision_id = decision.decision_id.clone();
        let proposal_id = decision.proposal_id.clone();

        match process_decision(agent, roots, outbox, decision).await {
            Ok(execute_report) => {
                report.processed_count += 1;
                report.executed_item_count += execute_report.executed_count;
                report.skipped_item_count += execute_report.skipped_count;
                report.results.push(DecisionProcessingResult {
                    decision_id,
                    proposal_id,
                    status: DecisionProcessingStatus::Completed,
                    message: None,
                    executed_item_count: execute_report.executed_count,
                    skipped_item_count: execute_report.skipped_count,
                });
            }
            Err(error) => {
                report.failed_count += 1;
                report.results.push(DecisionProcessingResult {
                    decision_id,
                    proposal_id,
                    status: DecisionProcessingStatus::Failed,
                    message: Some(error),
                    executed_item_count: 0,
                    skipped_item_count: 0,
                });
            }
        }
    }

    Ok(report)
}

async fn process_decision(
    agent: &AgentRuntime,
    roots: &ManagedRootStore,
    outbox: &OutboxStore,
    decision: AgentPendingDecision,
) -> Result<ExecuteReport, String> {
    // Claiming the execution moves the command to EXECUTING on the server. This stays a synchronous
    // network call because we need the returned execution id, and because a failed claim means we
    // must not touch any files. The claim also gates re-processing: once claimed, this decision is
    // no longer returned by pending_decisions.
    let execution = agent
        .create_execution(decision.proposal_id.clone(), decision.decision_id.clone())
        .await
        .map_err(|error| error.to_string())?;

    // From here the local files may change, so the result must be durable. We enqueue it to the
    // outbox (a local SQLite write) instead of sending it inline: if the network is down right
    // after the move, the flush loop still delivers the result later. The execution id is the
    // server idempotency key, so redelivery is safe.
    match execute_approved_decision(agent, roots, &decision).await {
        Ok(execute_report) => {
            let (status, summary) = summarize_execution(&execute_report);
            enqueue_execution_result(outbox, &execution.execution_id, status, summary)?;
            Ok(execute_report)
        }
        Err(error) => {
            enqueue_execution_result(
                outbox,
                &execution.execution_id,
                "FAILED",
                serde_json::json!({ "reason": error.clone() }),
            )?;
            Err(error)
        }
    }
}

async fn execute_approved_decision(
    agent: &AgentRuntime,
    roots: &ManagedRootStore,
    decision: &AgentPendingDecision,
) -> Result<ExecuteReport, String> {
    let room = agent
        .root_id_for_room(decision.room_id.clone())
        .await
        .map_err(|error| error.to_string())?;
    let managed_root = roots.ensure_active_room_binding(&room.root_id, &decision.room_id)?;
    if !managed_root.enabled {
        return Err(format!(
            "managed root is disabled: {}",
            managed_root.root_id
        ));
    }

    let local_report = build_local_proposal_report(&managed_root.root, &decision.items)?;
    let application = DecisionApplication {
        approved: local_report,
        rejected: Vec::new(),
    };

    execute_decision_application(&managed_root.root, application).map_err(|error| error.to_string())
}

fn build_local_proposal_report(
    root: impl AsRef<Path>,
    items: &[AgentProposalItemRecord],
) -> Result<ProposalReport, String> {
    if items.is_empty() {
        return Err("approved proposal has no items".to_string());
    }

    // Canonicalize through the same guard the local file engine uses so the reconstructed
    // report's root matches exactly what execute_decision_application will re-derive.
    let guard = PathGuard::new(root).map_err(|error| error.to_string())?;

    let mut sorted = items.to_vec();
    sorted.sort_by_key(|item| item.item_order);

    let proposals = sorted
        .iter()
        .map(local_proposal_from_item)
        .collect::<Result<Vec<_>, String>>()?;

    Ok(ProposalReport {
        root: guard.root().display().to_string(),
        proposals,
    })
}

fn local_proposal_from_item(item: &AgentProposalItemRecord) -> Result<Proposal, String> {
    // Delegated write actions stay deliberately narrow. CREATE_DIR can only create one empty
    // directory after explicit server approval and local precheck; README_WRITE can only target
    // README.md with approved content. Arbitrary file writes are still not accepted here.
    let action = match item.action_type.as_str() {
        "MOVE" => ProposalAction::Move,
        "QUARANTINE" => ProposalAction::Trash,
        "CREATE_DIR" => ProposalAction::CreateDir,
        "CREATE_FILE" => ProposalAction::CreateFile,
        "README_WRITE" => ProposalAction::ReadmeWrite,
        other => {
            return Err(format!(
                "delegated action type {other} is not supported yet"
            ))
        }
    };

    let from = match action {
        ProposalAction::Move | ProposalAction::Trash => item
            .source_relative_path
            .clone()
            .ok_or_else(|| format!("proposal item {} is missing a source path", item.item_order))?,
        ProposalAction::CreateDir | ProposalAction::CreateFile => String::new(),
        ProposalAction::ReadmeWrite => "README.md".to_string(),
    };

    let to = match action {
        ProposalAction::Move => item.destination_relative_path.clone().ok_or_else(|| {
            format!(
                "proposal item {} is missing a destination path",
                item.item_order
            )
        })?,
        ProposalAction::Trash => TRASH_DIR.to_string(),
        ProposalAction::CreateDir | ProposalAction::CreateFile => {
            item.destination_relative_path.clone().ok_or_else(|| {
                format!(
                    "proposal item {} is missing a destination path",
                    item.item_order
                )
            })?
        }
        ProposalAction::ReadmeWrite => "README.md".to_string(),
    };

    let status = match item.conflict_state.as_str() {
        "NONE" => ProposalStatus::Ready,
        "NAME_CONFLICT" => ProposalStatus::DestinationExists,
        other => {
            return Err(format!(
                "delegated conflict state {other} is not supported yet"
            ))
        }
    };

    let (source_size_bytes, source_modified_unix_ms, source_file_id) =
        precondition_snapshot(&item.precondition);
    let content = if action == ProposalAction::ReadmeWrite {
        Some(readme_write_content(&item.precondition)?)
    } else {
        None
    };

    Ok(Proposal {
        proposal_id: proposal_id(&action, &from, &to),
        action,
        from,
        to,
        content,
        source_size_bytes,
        source_modified_unix_ms,
        source_file_id,
        reason: item.reason_code.clone(),
        status,
    })
}

fn readme_write_content(value: &Value) -> Result<String, String> {
    let content = value
        .get("content")
        .or_else(|| value.get("readmeContent"))
        .and_then(Value::as_str)
        .ok_or_else(|| "README_WRITE precondition is missing approved content".to_string())?;
    if content.len() > 200_000 {
        return Err("README_WRITE content exceeds 200000 bytes".to_string());
    }
    Ok(content.to_string())
}

fn precondition_snapshot(value: &Value) -> (u64, Option<u128>, Option<String>) {
    let size = value
        .get("sourceSizeBytes")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let modified = value
        .get("sourceModifiedUnixMs")
        .and_then(Value::as_u64)
        .map(u128::from);
    let file_id = value
        .get("sourceFileId")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    (size, modified, file_id)
}

/// Maps a completed local execution to the control-plane execution status contract. MVP
/// approval always covers every item, so `rejected_count` from this path is always zero; a
/// batch that executed nothing is reported STALE rather than FAILED because the only way to
/// execute zero items here is every item failing precheck (missing source, changed source, or a
/// destination collision) rather than a hard error, which is handled separately.
fn summarize_execution(report: &ExecuteReport) -> (&'static str, Value) {
    let total = report.executed_count + report.skipped_count + report.rejected_count;
    let status = if total > 0 && report.executed_count == total {
        "SUCCEEDED"
    } else if report.executed_count > 0 {
        "PARTIALLY_SUCCEEDED"
    } else {
        "STALE"
    };
    let summary = serde_json::to_value(report)
        .unwrap_or_else(|_| serde_json::json!({ "executedCount": report.executed_count }));
    (status, summary)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use file_engine_cli::decision::DecisionApplication;
    use file_engine_cli::execute::execute_decision_application;
    use serde_json::json;
    use tempfile::tempdir;

    use super::{build_local_proposal_report, summarize_execution};
    use crate::agent::AgentProposalItemRecord;

    fn move_item(order: i64, from: &str, to: &str) -> AgentProposalItemRecord {
        AgentProposalItemRecord {
            item_order: order,
            action_type: "MOVE".to_string(),
            source_relative_path: Some(from.to_string()),
            destination_relative_path: Some(to.to_string()),
            reason_code: "RULE_MOVE_BY_EXTENSION".to_string(),
            precondition: json!({ "sourceSizeBytes": 3, "sourceModifiedUnixMs": 0 }),
            conflict_state: "NONE".to_string(),
        }
    }

    /// Builds a move item whose precondition matches the file's actual on-disk size and
    /// modified time, mirroring what command_processor::proposal_item() records at proposal
    /// creation time so the reconstructed proposal precisely matches the current file.
    fn move_item_matching_disk(
        root: &std::path::Path,
        order: i64,
        from: &str,
        to: &str,
    ) -> AgentProposalItemRecord {
        let metadata = fs::metadata(root.join(from)).expect("seeded file metadata");
        let modified_unix_ms = metadata
            .modified()
            .expect("mtime")
            .duration_since(std::time::UNIX_EPOCH)
            .expect("mtime after epoch")
            .as_millis() as u64;
        let source_file_id =
            file_engine_cli::file_identity::file_id_for_path(&root.join(from)).expect("file id");
        AgentProposalItemRecord {
            item_order: order,
            action_type: "MOVE".to_string(),
            source_relative_path: Some(from.to_string()),
            destination_relative_path: Some(to.to_string()),
            reason_code: "RULE_MOVE_BY_EXTENSION".to_string(),
            precondition: json!({
                "sourceSizeBytes": metadata.len(),
                "sourceModifiedUnixMs": modified_unix_ms,
                "sourceFileId": source_file_id
            }),
            conflict_state: "NONE".to_string(),
        }
    }

    #[test]
    fn delegated_create_dir_executes_and_stays_undoable() {
        use file_engine_cli::journal::{read_operation_history, JournalAction};
        use file_engine_cli::undo::undo_operation;

        let dir = tempdir().expect("tempdir");
        let root = dir.path();
        fs::create_dir_all(root.join("Archive")).expect("archive parent");

        let item = AgentProposalItemRecord {
            item_order: 0,
            action_type: "CREATE_DIR".to_string(),
            source_relative_path: None,
            destination_relative_path: Some("Archive/Reports".to_string()),
            reason_code: "USER_REQUESTED_CREATE_DIR".to_string(),
            precondition: json!({}),
            conflict_state: "NONE".to_string(),
        };

        let report = build_local_proposal_report(root, &[item]).expect("local create dir proposal");
        let application = DecisionApplication {
            approved: report,
            rejected: Vec::new(),
        };
        let execute_report =
            execute_decision_application(root, application).expect("execute create dir");
        let history = read_operation_history(root).expect("history");

        assert_eq!(execute_report.executed_count, 1);
        assert!(root.join("Archive/Reports").is_dir());
        assert_eq!(history.operations[0].action, JournalAction::CreateDir);
        assert!(history.operations[0].can_undo);

        let undo = undo_operation(root, &history.operations[0].operation_id).expect("undo");
        assert_eq!(undo.undone_count, 1);
        assert!(!root.join("Archive/Reports").exists());
    }

    #[test]
    fn delegated_create_file_executes_empty_file_and_stays_undoable() {
        use file_engine_cli::journal::{read_operation_history, JournalAction};
        use file_engine_cli::undo::undo_operation;

        let dir = tempdir().expect("tempdir");
        let root = dir.path();
        fs::create_dir_all(root.join("Notes")).expect("notes parent");

        let item = AgentProposalItemRecord {
            item_order: 0,
            action_type: "CREATE_FILE".to_string(),
            source_relative_path: None,
            destination_relative_path: Some("Notes/todo.txt".to_string()),
            reason_code: "USER_REQUESTED_CREATE_FILE".to_string(),
            precondition: json!({}),
            conflict_state: "NONE".to_string(),
        };

        let report =
            build_local_proposal_report(root, &[item]).expect("local create file proposal");
        let application = DecisionApplication {
            approved: report,
            rejected: Vec::new(),
        };
        let execute_report =
            execute_decision_application(root, application).expect("execute create file");
        let history = read_operation_history(root).expect("history");

        assert_eq!(execute_report.executed_count, 1);
        assert_eq!(
            fs::metadata(root.join("Notes/todo.txt"))
                .expect("created file")
                .len(),
            0
        );
        assert_eq!(history.operations[0].action, JournalAction::CreateFile);
        assert!(history.operations[0].can_undo);

        let undo = undo_operation(root, &history.operations[0].operation_id).expect("undo");
        assert_eq!(undo.undone_count, 1);
        assert!(!root.join("Notes/todo.txt").exists());
    }

    #[test]
    fn delegated_readme_write_executes_and_stays_undoable() {
        use file_engine_cli::journal::{read_operation_history, JournalAction};
        use file_engine_cli::undo::undo_operation;

        let dir = tempdir().expect("tempdir");
        let root = dir.path();
        fs::write(root.join("README.md"), "# Old\n").expect("seed readme");
        let metadata = fs::metadata(root.join("README.md")).expect("readme metadata");
        let modified_unix_ms = metadata
            .modified()
            .expect("mtime")
            .duration_since(std::time::UNIX_EPOCH)
            .expect("mtime after epoch")
            .as_millis() as u64;
        let item = AgentProposalItemRecord {
            item_order: 0,
            action_type: "README_WRITE".to_string(),
            source_relative_path: None,
            destination_relative_path: Some("README.md".to_string()),
            reason_code: "README_WRITE".to_string(),
            precondition: json!({
                "sourceSizeBytes": metadata.len(),
                "sourceModifiedUnixMs": modified_unix_ms,
                "content": "# New\n"
            }),
            conflict_state: "NONE".to_string(),
        };
        let report = build_local_proposal_report(root, &[item]).expect("local readme proposal");
        let execute_report = execute_decision_application(
            root,
            DecisionApplication {
                approved: report,
                rejected: Vec::new(),
            },
        )
        .expect("execute readme write");

        assert_eq!(execute_report.executed_count, 1);
        assert_eq!(
            fs::read_to_string(root.join("README.md")).expect("read readme"),
            "# New\n"
        );
        let history = read_operation_history(root).expect("history");
        assert_eq!(history.operations[0].action, JournalAction::ReadmeWrite);
        assert!(history.operations[0].can_undo);

        undo_operation(root, &history.operations[0].operation_id).expect("undo readme write");
        assert_eq!(
            fs::read_to_string(root.join("README.md")).expect("read restored readme"),
            "# Old\n"
        );
    }

    #[test]
    fn delegated_rename_executes_as_a_journaled_move() {
        use file_engine_cli::journal::{read_operation_history, JournalAction};

        let dir = tempdir().expect("tempdir");
        let root = dir.path();
        fs::create_dir_all(root.join("notes")).expect("notes dir");
        fs::write(root.join("notes/old.txt"), b"hi").expect("seed file");

        // A delegated rename is just a MOVE whose destination is a sibling of the source.
        let items = vec![move_item_matching_disk(
            root,
            0,
            "notes/old.txt",
            "notes/new.txt",
        )];
        let report = build_local_proposal_report(root, &items).expect("local proposal");
        let application = DecisionApplication {
            approved: report,
            rejected: Vec::new(),
        };
        let execute_report = execute_decision_application(root, application).expect("execute");

        assert_eq!(execute_report.executed_count, 1);
        assert!(root.join("notes/new.txt").exists());
        assert!(!root.join("notes/old.txt").exists());

        // The rename is journaled as a Move, exactly like a manual rename, so it stays undoable.
        let history = read_operation_history(root).expect("history");
        assert_eq!(history.operations[0].action, JournalAction::Move);
        assert!(history.operations[0].can_undo);

        let (status, _) = summarize_execution(&execute_report);
        assert_eq!(status, "SUCCEEDED");
    }

    #[test]
    fn build_local_proposal_report_rejects_empty_batches() {
        let dir = tempdir().expect("tempdir");
        let error = build_local_proposal_report(dir.path(), &[]).expect_err("no items");
        assert!(error.contains("no items"));
    }

    #[test]
    fn approved_decision_executes_and_journals_a_move() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();
        fs::write(root.join("a.pdf"), b"pdf").expect("seed file");
        fs::create_dir_all(root.join("Documents")).expect("dest dir");

        let items = vec![move_item_matching_disk(root, 0, "a.pdf", "Documents/a.pdf")];
        let report = build_local_proposal_report(root, &items).expect("local proposal");
        assert!(report.proposals[0].source_file_id.is_some());

        let application = DecisionApplication {
            approved: report,
            rejected: Vec::new(),
        };
        let execute_report = execute_decision_application(root, application).expect("execute");

        assert_eq!(execute_report.executed_count, 1);
        assert!(root.join("Documents/a.pdf").exists());
        assert!(!root.join("a.pdf").exists());

        let (status, _) = summarize_execution(&execute_report);
        assert_eq!(status, "SUCCEEDED");
    }

    #[test]
    fn changed_source_is_reported_stale_without_executing() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();
        // precondition below claims 3 bytes; writing more here simulates the file changing
        // between proposal submission and mobile approval.
        fs::write(root.join("a.pdf"), b"changed-content").expect("seed file");
        fs::create_dir_all(root.join("Documents")).expect("dest dir");

        let items = vec![move_item(0, "a.pdf", "Documents/a.pdf")];
        let report = build_local_proposal_report(root, &items).expect("local proposal");
        let application = DecisionApplication {
            approved: report,
            rejected: Vec::new(),
        };
        let execute_report = execute_decision_application(root, application).expect("execute");

        assert_eq!(execute_report.executed_count, 0);
        assert_eq!(execute_report.skipped_count, 1);
        assert!(root.join("a.pdf").exists());

        let (status, _) = summarize_execution(&execute_report);
        assert_eq!(status, "STALE");
    }
}
