use std::error::Error;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::decision::{DecisionApplication, DecisionError, RejectedProposal};
use crate::journal::{JournalAction, JournalEntry, JournalStatus, JOURNAL_FILE, STATE_DIR};
use crate::path_guard::{PathGuard, PathGuardError};
use crate::precondition::{
    precheck_proposals, precheck_root, PrecheckError, PrecheckReport, PrecheckResult,
    PrecheckStatus,
};
use crate::proposal::{ProposalError, ProposalReport};

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
    CreateStateDir {
        path: PathBuf,
        message: String,
    },
    CreateParentDir {
        path: PathBuf,
        message: String,
    },
    OpenJournal {
        path: PathBuf,
        message: String,
    },
    ReadJournal {
        path: PathBuf,
        message: String,
    },
    ParseJournal {
        line: usize,
        message: String,
    },
    WriteJournal {
        path: PathBuf,
        message: String,
    },
    Move {
        from: PathBuf,
        to: PathBuf,
        message: String,
    },
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
    let journal_path = ensure_journal_path(guard.root())?;
    let prior_entries = read_journal_entries(&journal_path)?;
    let mut journal = open_journal(&journal_path)?;

    let mut executed_count = 0;
    let mut skipped_count = 0;
    let mut results = Vec::new();

    for (index, check) in precheck.checks.iter().enumerate() {
        if check.status != PrecheckStatus::Ready {
            if let Some(result) =
                recover_planned_move(&guard, &prior_entries, &mut journal, &journal_path, check)?
            {
                executed_count += 1;
                results.push(result);
                continue;
            }

            skipped_count += 1;
            results.push(skipped_result(check, check.reason.clone()));
            continue;
        }

        let operation_id = format!("op-{}-{index}", unix_ms());
        append_journal(
            &mut journal,
            &journal_path,
            planned_entry(&operation_id, check),
        )?;

        let source = match guard.resolve_existing(&check.from) {
            Ok(source) => source,
            Err(error) => {
                skipped_count += 1;
                results.push(skipped_result(check, Some(error.to_string())));
                continue;
            }
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
            skipped_count += 1;
            results.push(skipped_result(
                check,
                Some("destination appeared before move; refusing to overwrite".to_string()),
            ));
            continue;
        }

        fs::rename(&source, &destination).map_err(|error| ExecuteError::Move {
            from: source,
            to: destination,
            message: error.to_string(),
        })?;

        append_journal(
            &mut journal,
            &journal_path,
            executed_entry(&operation_id, check),
        )?;
        executed_count += 1;
        results.push(ExecuteResult {
            from: check.from.clone(),
            to: check.to.clone(),
            status: ExecuteStatus::Executed,
            reason: None,
        });
    }

    Ok(ExecuteReport {
        root: precheck.root,
        journal_path: journal_path.display().to_string(),
        executed_count,
        skipped_count,
        rejected_count: 0,
        results,
    })
}

fn ensure_journal_path(root: &Path) -> Result<PathBuf, ExecuteError> {
    let state_dir = root.join(STATE_DIR);
    fs::create_dir_all(&state_dir).map_err(|error| ExecuteError::CreateStateDir {
        path: state_dir.clone(),
        message: error.to_string(),
    })?;
    Ok(state_dir.join(JOURNAL_FILE))
}

fn open_journal(journal_path: &Path) -> Result<fs::File, ExecuteError> {
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(journal_path)
        .map_err(|error| ExecuteError::OpenJournal {
            path: journal_path.to_path_buf(),
            message: error.to_string(),
        })
}

fn read_journal_entries(journal_path: &Path) -> Result<Vec<JournalEntry>, ExecuteError> {
    if !journal_path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(journal_path).map_err(|error| ExecuteError::ReadJournal {
        path: journal_path.to_path_buf(),
        message: error.to_string(),
    })?;

    content
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(index, line)| {
            serde_json::from_str::<JournalEntry>(line).map_err(|error| ExecuteError::ParseJournal {
                line: index + 1,
                message: error.to_string(),
            })
        })
        .collect()
}

fn append_journal(
    journal: &mut fs::File,
    journal_path: &Path,
    entry: JournalEntry,
) -> Result<(), ExecuteError> {
    let line = serde_json::to_string(&entry)
        .map_err(|error| ExecuteError::Serialize(error.to_string()))?;
    writeln!(journal, "{line}").map_err(|error| ExecuteError::WriteJournal {
        path: journal_path.to_path_buf(),
        message: error.to_string(),
    })
}

fn recover_planned_move(
    guard: &PathGuard,
    prior_entries: &[JournalEntry],
    journal: &mut fs::File,
    journal_path: &Path,
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

    append_journal(journal, journal_path, executed_entry(&operation_id, check))?;

    Ok(Some(ExecuteResult {
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
        action: JournalAction::Move,
        from: check.from.clone(),
        to: check.to.clone(),
        created_unix_ms: unix_ms(),
    }
}

fn skipped_result(check: &PrecheckResult, reason: Option<String>) -> ExecuteResult {
    ExecuteResult {
        from: check.from.clone(),
        to: check.to.clone(),
        status: ExecuteStatus::Skipped,
        reason,
    }
}

fn rejected_result(rejected: RejectedProposal) -> ExecuteResult {
    ExecuteResult {
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
            ExecuteError::CreateStateDir { path, message } => {
                write!(
                    formatter,
                    "cannot create state directory {}: {message}",
                    path.display()
                )
            }
            ExecuteError::CreateParentDir { path, message } => {
                write!(
                    formatter,
                    "cannot create destination parent {}: {message}",
                    path.display()
                )
            }
            ExecuteError::OpenJournal { path, message } => {
                write!(
                    formatter,
                    "cannot open journal {}: {message}",
                    path.display()
                )
            }
            ExecuteError::ReadJournal { path, message } => {
                write!(
                    formatter,
                    "cannot read journal {}: {message}",
                    path.display()
                )
            }
            ExecuteError::ParseJournal { line, message } => {
                write!(formatter, "cannot parse journal line {line}: {message}")
            }
            ExecuteError::WriteJournal { path, message } => {
                write!(
                    formatter,
                    "cannot write journal {}: {message}",
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
    use crate::journal::{JournalAction, JournalEntry, JournalStatus, JOURNAL_FILE, STATE_DIR};
    use crate::proposal::propose_for_root;

    use super::{execute_decision_application, execute_proposals, execute_root, ExecuteStatus};

    #[test]
    fn moves_ready_file_after_journaling() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("inbox")).expect("create inbox");
        fs::write(root.join("inbox").join("note.md"), "# note").expect("write note");

        let report = execute_root(&root).expect("execute");
        let journal = fs::read_to_string(root.join(".housemouse").join("journal.jsonl"))
            .expect("read journal");

        assert_eq!(report.executed_count, 1);
        assert_eq!(report.skipped_count, 0);
        assert_eq!(report.rejected_count, 0);
        assert!(!root.join("inbox").join("note.md").exists());
        assert!(root.join("documents").join("note.md").exists());
        assert!(journal.contains("\"status\":\"planned\""));
        assert!(journal.contains("\"status\":\"executed\""));
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

        let state_dir = root.join(STATE_DIR);
        fs::create_dir_all(&state_dir).expect("create state dir");
        let planned = JournalEntry {
            operation_id: "op-recover-0".to_string(),
            status: JournalStatus::Planned,
            action: JournalAction::Move,
            from: "inbox/note.md".to_string(),
            to: "documents/note.md".to_string(),
            created_unix_ms: 1,
        };
        fs::write(
            state_dir.join(JOURNAL_FILE),
            format!("{}\n", serde_json::to_string(&planned).expect("serialize")),
        )
        .expect("write journal");
        fs::create_dir_all(root.join("documents")).expect("create documents");
        fs::rename(
            root.join("inbox").join("note.md"),
            root.join("documents").join("note.md"),
        )
        .expect("simulate completed move before executed journal");

        let report = execute_proposals(&root, proposal).expect("execute recover");
        let journal =
            fs::read_to_string(root.join(STATE_DIR).join(JOURNAL_FILE)).expect("read journal");

        assert_eq!(report.executed_count, 1);
        assert_eq!(report.skipped_count, 0);
        assert!(journal.contains("\"status\":\"executed\""));
        assert!(root.join("documents").join("note.md").exists());
    }
}
