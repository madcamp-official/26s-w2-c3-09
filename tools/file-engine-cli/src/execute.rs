use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::decision::{DecisionApplication, DecisionError, RejectedProposal};
use crate::journal::{JournalAction, JournalEntry, JournalError, JournalStatus, JournalStore};
use crate::path_guard::{PathGuard, PathGuardError};
use crate::precondition::{
    precheck_proposals, precheck_root, PrecheckError, PrecheckReport, PrecheckResult,
    PrecheckStatus,
};
use crate::proposal::{ProposalAction, ProposalError, ProposalReport};
use crate::trash::{trash_file, TrashError};

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ExecuteReport {
    pub root: String,
    pub journal_path: String,
    pub executed_count: usize,
    pub skipped_count: usize,
    pub rejected_count: usize,
    pub results: Vec<ExecuteResult>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ExecuteResult {
    pub action: ProposalAction,
    pub from: String,
    pub to: String,
    pub status: ExecuteStatus,
    pub reason: Option<String>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteStatus {
    Executed,
    Skipped,
    Rejected,
}

#[derive(Debug)]
pub enum ExecuteError {
    Guard(PathGuardError),
    Precheck(PrecheckError),
    PrecheckProposal(ProposalError),
    Decision(DecisionError),
    Journal(JournalError),
    CreateParentDir {
        path: PathBuf,
        message: String,
    },
    Move {
        from: PathBuf,
        to: PathBuf,
        message: String,
    },
    Trash(TrashError),
    Serialize(String),
}

pub fn execute_root(root: impl AsRef<Path>) -> Result<ExecuteReport, ExecuteError> {
    let guard = PathGuard::new(root).map_err(ExecuteError::Guard)?;
    let precheck = precheck_root(guard.root()).map_err(ExecuteError::Precheck)?;

    execute_prechecked(guard, precheck)
}

pub fn execute_proposals(
    root: impl AsRef<Path>,
    report: ProposalReport,
) -> Result<ExecuteReport, ExecuteError> {
    let guard = PathGuard::new(root).map_err(ExecuteError::Guard)?;
    let precheck = precheck_proposals(guard.root(), report).map_err(ExecuteError::Precheck)?;

    execute_prechecked(guard, precheck)
}

pub fn execute_decision_application(
    root: impl AsRef<Path>,
    application: DecisionApplication,
) -> Result<ExecuteReport, ExecuteError> {
    let rejected = application.rejected;
    let mut report = execute_proposals(root, application.approved)?;

    report.rejected_count = rejected.len();
    report.results.extend(
        rejected
            .into_iter()
            .map(rejected_result)
            .collect::<Vec<_>>(),
    );

    Ok(report)
}

fn execute_prechecked(
    guard: PathGuard,
    precheck: PrecheckReport,
) -> Result<ExecuteReport, ExecuteError> {
    let store = JournalStore::open(guard.root()).map_err(ExecuteError::Journal)?;
    let prior_entries = store.read_all().map_err(ExecuteError::Journal)?;

    let mut executed_count = 0;
    let mut skipped_count = 0;
    let mut results = Vec::new();

    for (index, check) in precheck.checks.iter().enumerate() {
        if check.status != PrecheckStatus::Ready {
            if let Some(result) = recover_planned_move(&guard, &prior_entries, &store, check)? {
                executed_count += 1;
                results.push(result);
                continue;
            }

            skipped_count += 1;
            results.push(skipped_result(check, check.reason.clone()));
            continue;
        }

        match check.action {
            ProposalAction::Move => match execute_move(&guard, &store, check, index)? {
                Ok(result) => {
                    executed_count += 1;
                    results.push(result);
                }
                Err(reason) => {
                    skipped_count += 1;
                    results.push(skipped_result(check, Some(reason)));
                }
            },
            ProposalAction::Trash => {
                let report = trash_file(guard.root(), &check.from).map_err(ExecuteError::Trash)?;
                executed_count += 1;
                results.push(ExecuteResult {
                    action: check.action.clone(),
                    from: check.from.clone(),
                    to: report.trashed_path,
                    status: ExecuteStatus::Executed,
                    reason: None,
                });
            }
        }
    }

    Ok(ExecuteReport {
        root: precheck.root,
        journal_path: store.journal_path_display(),
        executed_count,
        skipped_count,
        rejected_count: 0,
        results,
    })
}

fn execute_move(
    guard: &PathGuard,
    store: &JournalStore,
    check: &PrecheckResult,
    index: usize,
) -> Result<Result<ExecuteResult, String>, ExecuteError> {
    let operation_id = format!("op-{}-{index}", unix_ms());
    store
        .append(&planned_entry(&operation_id, check))
        .map_err(ExecuteError::Journal)?;

    let source = match guard.resolve_existing(&check.from) {
        Ok(source) => source,
        Err(error) => return Ok(Err(error.to_string())),
    };

    let destination = guard.root().join(&check.to);
    let parent = destination
        .parent()
        .ok_or_else(|| ExecuteError::Guard(PathGuardError::MissingPath(destination.clone())))?;
    fs::create_dir_all(parent).map_err(|error| ExecuteError::CreateParentDir {
        path: parent.to_path_buf(),
        message: error.to_string(),
    })?;

    let destination = guard
        .resolve_for_create(&check.to)
        .map_err(ExecuteError::Guard)?;
    if destination.exists() {
        return Ok(Err(
            "destination appeared before move; refusing to overwrite".to_string(),
        ));
    }

    fs::rename(&source, &destination).map_err(|error| ExecuteError::Move {
        from: source,
        to: destination,
        message: error.to_string(),
    })?;

    store
        .append(&executed_entry(&operation_id, check))
        .map_err(ExecuteError::Journal)?;

    Ok(Ok(ExecuteResult {
        action: check.action.clone(),
        from: check.from.clone(),
        to: check.to.clone(),
        status: ExecuteStatus::Executed,
        reason: None,
    }))
}

fn recover_planned_move(
    guard: &PathGuard,
    prior_entries: &[JournalEntry],
    store: &JournalStore,
    check: &PrecheckResult,
) -> Result<Option<ExecuteResult>, ExecuteError> {
    if check.status != PrecheckStatus::MissingSource {
        return Ok(None);
    }

    let Some(operation_id) = pending_planned_operation(prior_entries, &check.from, &check.to)
    else {
        return Ok(None);
    };

    if guard.resolve_existing(&check.to).is_err() {
        return Ok(None);
    }

    store
        .append(&executed_entry(&operation_id, check))
        .map_err(ExecuteError::Journal)?;

    Ok(Some(ExecuteResult {
        action: check.action.clone(),
        from: check.from.clone(),
        to: check.to.clone(),
        status: ExecuteStatus::Executed,
        reason: Some(
            "source missing and destination exists; recorded recovered execute".to_string(),
        ),
    }))
}

fn pending_planned_operation(entries: &[JournalEntry], from: &str, to: &str) -> Option<String> {
    let completed_ids = entries
        .iter()
        .filter(|entry| {
            matches!(
                entry.status,
                JournalStatus::Executed | JournalStatus::Undone | JournalStatus::UndoPlanned
            )
        })
        .map(|entry| entry.operation_id.as_str())
        .collect::<std::collections::HashSet<_>>();

    entries
        .iter()
        .rev()
        .find(|entry| {
            entry.status == JournalStatus::Planned
                && entry.action == JournalAction::Move
                && entry.from == from
                && entry.to == to
                && !completed_ids.contains(entry.operation_id.as_str())
        })
        .map(|entry| entry.operation_id.clone())
}

fn planned_entry(operation_id: &str, check: &PrecheckResult) -> JournalEntry {
    journal_entry(operation_id, JournalStatus::Planned, check)
}

fn executed_entry(operation_id: &str, check: &PrecheckResult) -> JournalEntry {
    journal_entry(operation_id, JournalStatus::Executed, check)
}

fn journal_entry(
    operation_id: &str,
    status: JournalStatus,
    check: &PrecheckResult,
) -> JournalEntry {
    JournalEntry {
        operation_id: operation_id.to_string(),
        status,
        action: match check.action {
            ProposalAction::Move => JournalAction::Move,
            ProposalAction::Trash => JournalAction::Trash,
        },
        from: check.from.clone(),
        to: check.to.clone(),
        created_unix_ms: unix_ms(),
    }
}

fn skipped_result(check: &PrecheckResult, reason: Option<String>) -> ExecuteResult {
    ExecuteResult {
        action: check.action.clone(),
        from: check.from.clone(),
        to: check.to.clone(),
        status: ExecuteStatus::Skipped,
        reason,
    }
}

fn rejected_result(rejected: RejectedProposal) -> ExecuteResult {
    ExecuteResult {
        action: rejected.proposal.action,
        from: rejected.proposal.from,
        to: rejected.proposal.to,
        status: ExecuteStatus::Rejected,
        reason: Some(rejected.reason),
    }
}

fn unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

impl fmt::Display for ExecuteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExecuteError::Guard(error) => write!(formatter, "{error}"),
            ExecuteError::Precheck(error) => write!(formatter, "{error}"),
            ExecuteError::PrecheckProposal(error) => write!(formatter, "{error}"),
            ExecuteError::Decision(error) => write!(formatter, "{error}"),
            ExecuteError::Journal(error) => write!(formatter, "{error}"),
            ExecuteError::CreateParentDir { path, message } => {
                write!(
                    formatter,
                    "cannot create destination parent {}: {message}",
                    path.display()
                )
            }
            ExecuteError::Move { from, to, message } => {
                write!(
                    formatter,
                    "cannot move {} to {}: {message}",
                    from.display(),
                    to.display()
                )
            }
            ExecuteError::Trash(error) => write!(formatter, "{error}"),
            ExecuteError::Serialize(message) => {
                write!(formatter, "cannot serialize execute journal: {message}")
            }
        }
    }
}

impl Error for ExecuteError {}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use crate::decision::{Decision, DecisionEntry};
    use crate::journal::{JournalAction, JournalEntry, JournalStatus, JournalStore};
    use crate::proposal::propose_for_root;

    use super::{execute_decision_application, execute_proposals, execute_root, ExecuteStatus};

    #[test]
    fn moves_ready_file_after_journaling() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("inbox")).expect("create inbox");
        fs::write(root.join("inbox").join("note.md"), "# note").expect("write note");

        let report = execute_root(&root).expect("execute");
        let history = crate::journal::read_operation_history(&root).expect("history");

        assert_eq!(report.executed_count, 1);
        assert_eq!(report.skipped_count, 0);
        assert_eq!(report.rejected_count, 0);
        assert!(!root.join("inbox").join("note.md").exists());
        assert!(root.join("documents").join("note.md").exists());
        // The journal recorded the executed move (planned then executed collapse to one op).
        assert_eq!(history.operations.len(), 1);
        assert_eq!(history.operations[0].latest_status, JournalStatus::Executed);
    }

    #[test]
    fn skips_destination_collision_without_overwriting() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("inbox")).expect("create inbox");
        fs::create_dir_all(root.join("documents")).expect("create documents");
        fs::write(root.join("inbox").join("note.md"), "# note").expect("write note");
        fs::write(root.join("documents").join("note.md"), "# existing").expect("write existing");

        let report = execute_root(&root).expect("execute");
        let destination =
            fs::read_to_string(root.join("documents").join("note.md")).expect("read destination");

        assert_eq!(report.executed_count, 0);
        assert_eq!(report.skipped_count, 1);
        assert_eq!(report.rejected_count, 0);
        assert!(root.join("inbox").join("note.md").exists());
        assert_eq!(destination, "# existing");
    }

    #[test]
    fn executes_saved_proposal_snapshot() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("inbox")).expect("create inbox");
        fs::write(root.join("inbox").join("note.md"), "# note").expect("write note");
        let proposal = propose_for_root(&root).expect("propose");

        let report = execute_proposals(&root, proposal).expect("execute saved proposal");

        assert_eq!(report.executed_count, 1);
        assert_eq!(report.rejected_count, 0);
        assert!(!root.join("inbox").join("note.md").exists());
        assert!(root.join("documents").join("note.md").exists());
    }

    #[test]
    fn executes_approved_trash_proposal() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("inbox")).expect("create inbox");
        fs::create_dir_all(root.join(".housemouse")).expect("create state dir");
        fs::write(root.join("inbox").join("cache.tmp"), "noise").expect("write temp");
        fs::write(
            root.join(".housemouse").join("rules.json"),
            r#"{"version":1,"rules":[{"id":"temp-trash","when":{"name_matches":"*.tmp"},"then":{"trash":true}}]}"#,
        )
        .expect("write rules");

        let report = execute_root(&root).expect("execute");
        let history = crate::journal::read_operation_history(&root).expect("history");

        assert_eq!(report.executed_count, 1);
        assert_eq!(
            report.results[0].action,
            crate::proposal::ProposalAction::Trash
        );
        assert!(!root.join("inbox").join("cache.tmp").exists());
        assert!(root.join(&report.results[0].to).exists());
        assert_eq!(history.operations[0].action, JournalAction::Trash);
        assert!(history.operations[0].can_undo);
    }

    #[test]
    fn proposal_trash_matches_direct_trash_on_disk_structure() {
        // A proposal-executed QUARANTINE and a direct manual trash must produce the same journal
        // action and the same recoverable on-disk layout (a `file` payload plus an `original.json`
        // metadata sidecar under .housemouse_trash/<op>/), because both go through trash_file().
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("inbox")).expect("create inbox");
        fs::create_dir_all(root.join(".housemouse")).expect("create state dir");
        fs::write(root.join("inbox").join("cache.tmp"), "noise").expect("write temp");
        fs::write(
            root.join(".housemouse").join("rules.json"),
            r#"{"version":1,"rules":[{"id":"temp-trash","when":{"name_matches":"*.tmp"},"then":{"trash":true}}]}"#,
        )
        .expect("write rules");

        let report = execute_root(&root).expect("execute");
        let history = crate::journal::read_operation_history(&root).expect("history");

        let trashed_relative = &report.results[0].to;
        let trashed_dir = std::path::Path::new(trashed_relative)
            .parent()
            .expect("trashed parent");
        // Same payload filename and metadata sidecar a direct trash writes.
        assert!(root.join(trashed_relative).exists());
        assert!(root.join(trashed_dir).join("original.json").exists());
        assert!(trashed_relative.starts_with(crate::journal::TRASH_DIR));
        assert_eq!(history.operations[0].action, JournalAction::Trash);
    }

    #[test]
    fn refuses_saved_proposal_when_source_changed() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("inbox")).expect("create inbox");
        let note = root.join("inbox").join("note.md");
        fs::write(&note, "# note").expect("write note");
        let proposal = propose_for_root(&root).expect("propose");
        fs::write(&note, "# changed").expect("change note");

        let report = execute_proposals(&root, proposal).expect("execute saved proposal");

        assert_eq!(report.executed_count, 0);
        assert_eq!(report.skipped_count, 1);
        assert_eq!(report.rejected_count, 0);
        assert!(root.join("inbox").join("note.md").exists());
        assert!(!root.join("documents").join("note.md").exists());
    }

    #[test]
    fn reports_rejected_decisions_without_moving_files() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("inbox")).expect("create inbox");
        fs::write(root.join("inbox").join("note.md"), "# note").expect("write note");
        let proposal = propose_for_root(&root).expect("propose");
        let decision = DecisionEntry {
            proposal_id: proposal.proposals[0].proposal_id.clone(),
            decision: Decision::Rejected,
            reason: Some("user kept it in place".to_string()),
        };
        let application =
            crate::decision::apply_decisions(proposal, &[decision]).expect("apply decision");

        let report = execute_decision_application(&root, application).expect("execute");

        assert_eq!(report.executed_count, 0);
        assert_eq!(report.skipped_count, 0);
        assert_eq!(report.rejected_count, 1);
        assert_eq!(report.results[0].status, ExecuteStatus::Rejected);
        assert_eq!(
            report.results[0].reason,
            Some("user kept it in place".to_string())
        );
        assert!(root.join("inbox").join("note.md").exists());
        assert!(!root.join("documents").join("note.md").exists());
    }

    #[test]
    fn records_recovered_execute_when_move_finished_before_journal() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("inbox")).expect("create inbox");
        fs::write(root.join("inbox").join("note.md"), "# note").expect("write note");
        let proposal = propose_for_root(&root).expect("propose");

        // Simulate a crash between the planned journal write and the executed one: the planned
        // row exists and the file was already moved, but no executed row was recorded.
        let store = JournalStore::open(&root.canonicalize().expect("canonical")).expect("store");
        store
            .append(&JournalEntry {
                operation_id: "op-recover-0".to_string(),
                status: JournalStatus::Planned,
                action: JournalAction::Move,
                from: "inbox/note.md".to_string(),
                to: "documents/note.md".to_string(),
                created_unix_ms: 1,
            })
            .expect("append planned");
        fs::create_dir_all(root.join("documents")).expect("create documents");
        fs::rename(
            root.join("inbox").join("note.md"),
            root.join("documents").join("note.md"),
        )
        .expect("simulate completed move before executed journal");

        let report = execute_proposals(&root, proposal).expect("execute recover");
        let history = crate::journal::read_operation_history(&root).expect("history");

        assert_eq!(report.executed_count, 1);
        assert_eq!(report.skipped_count, 0);
        assert_eq!(history.operations.len(), 1);
        assert_eq!(history.operations[0].latest_status, JournalStatus::Executed);
        assert!(root.join("documents").join("note.md").exists());
    }
}
