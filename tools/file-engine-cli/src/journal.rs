use std::error::Error;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::path_guard::{PathGuard, PathGuardError};
use crate::precondition::{precheck_root, PrecheckError, PrecheckStatus};

pub const STATE_DIR: &str = ".housemouse";
pub const JOURNAL_FILE: &str = "journal.jsonl";

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct JournalWriteReport {
    pub root: String,
    pub journal_path: String,
    pub planned_count: usize,
    pub skipped_count: usize,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct OperationHistoryReport {
    pub root: String,
    pub journal_path: String,
    pub operations: Vec<OperationHistoryEntry>,
    pub corruption: Option<JournalCorruption>,
}

/// Describes the first unparseable journal line. `operations` in `OperationHistoryReport`
/// only reflects entries before this line: once one line is untrustworthy, later "undone"
/// markers could reference operations we can no longer verify, so history stops there
/// instead of guessing.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct JournalCorruption {
    pub line: usize,
    pub message: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct OperationHistoryEntry {
    pub operation_id: String,
    pub action: JournalAction,
    pub from: String,
    pub to: String,
    pub latest_status: JournalStatus,
    pub created_unix_ms: u128,
    pub can_undo: bool,
    /// Set whenever `can_undo` is false, explaining why: journal status (not executed yet,
    /// already undone) or, for executed moves, a live filesystem check (destination missing,
    /// original path occupied) mirroring what `undo::undo_operation` would actually refuse.
    pub undo_blocked_reason: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct JournalEntry {
    pub operation_id: String,
    pub status: JournalStatus,
    pub action: JournalAction,
    pub from: String,
    pub to: String,
    pub created_unix_ms: u128,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JournalStatus {
    Planned,
    Executed,
    UndoPlanned,
    Undone,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JournalAction {
    Move,
}

#[derive(Debug)]
pub enum JournalError {
    Guard(PathGuardError),
    Precheck(PrecheckError),
    CreateStateDir { path: PathBuf, message: String },
    ReadJournal { path: PathBuf, message: String },
    ParseJournal { line: usize, message: String },
    OpenJournal { path: PathBuf, message: String },
    WriteJournal { path: PathBuf, message: String },
    NotCorrupted { path: PathBuf },
    Serialize(String),
}

pub fn read_operation_history(
    root: impl AsRef<Path>,
) -> Result<OperationHistoryReport, JournalError> {
    let guard = PathGuard::new(root).map_err(JournalError::Guard)?;
    let journal_path = guard.root().join(STATE_DIR).join(JOURNAL_FILE);
    let (entries, corruption) = read_journal_entries_lenient(&journal_path)?;
    let mut operations: Vec<OperationHistoryEntry> = Vec::new();

    for entry in entries {
        match entry.status {
            JournalStatus::Planned => operations.push(OperationHistoryEntry {
                operation_id: entry.operation_id,
                action: entry.action,
                from: entry.from,
                to: entry.to,
                latest_status: JournalStatus::Planned,
                created_unix_ms: entry.created_unix_ms,
                can_undo: false,
                undo_blocked_reason: Some("operation has not been executed yet".to_string()),
            }),
            JournalStatus::Executed => {
                if let Some(operation) = operations
                    .iter_mut()
                    .find(|operation| operation.operation_id == entry.operation_id)
                {
                    operation.latest_status = JournalStatus::Executed;
                    operation.created_unix_ms = entry.created_unix_ms;
                    operation.can_undo = true;
                    operation.undo_blocked_reason = None;
                } else {
                    operations.push(OperationHistoryEntry {
                        operation_id: entry.operation_id,
                        action: entry.action,
                        from: entry.from,
                        to: entry.to,
                        latest_status: JournalStatus::Executed,
                        created_unix_ms: entry.created_unix_ms,
                        can_undo: true,
                        undo_blocked_reason: None,
                    });
                }
            }
            JournalStatus::UndoPlanned | JournalStatus::Undone => {
                if let Some(operation) = operations
                    .iter_mut()
                    .find(|operation| operation.operation_id == entry.operation_id)
                {
                    operation.can_undo = false;
                    operation.undo_blocked_reason = Some(match entry.status {
                        JournalStatus::UndoPlanned => {
                            "undo is already in progress for this operation".to_string()
                        }
                        JournalStatus::Undone => "already undone".to_string(),
                        JournalStatus::Planned | JournalStatus::Executed => unreachable!(),
                    });
                    operation.latest_status = entry.status;
                }
            }
        }
    }

    for operation in operations.iter_mut().filter(|operation| operation.can_undo) {
        if let Some(reason) = undo_blocked_reason_from_filesystem(guard.root(), operation) {
            operation.can_undo = false;
            operation.undo_blocked_reason = Some(reason);
        }
    }

    operations.sort_by(|left, right| {
        right
            .created_unix_ms
            .cmp(&left.created_unix_ms)
            .then_with(|| left.operation_id.cmp(&right.operation_id))
    });

    Ok(OperationHistoryReport {
        root: guard.root().display().to_string(),
        journal_path: journal_path.display().to_string(),
        operations,
        corruption,
    })
}

/// Mirrors the live-filesystem checks `undo::undo_operation` makes right before moving a
/// file back, so History can tell the user *why* an executed operation would fail to undo
/// instead of only finding out after they click the button. This is diagnostic only: it does
/// not use `PathGuard`'s canonicalizing resolvers, since undo itself still re-validates and
/// enforces the actual safety boundary at execution time.
fn undo_blocked_reason_from_filesystem(
    root: &Path,
    operation: &OperationHistoryEntry,
) -> Option<String> {
    if operation.action != JournalAction::Move {
        return None;
    }

    let destination = root.join(&operation.to);
    let original = root.join(&operation.from);

    if !destination.exists() {
        return if original.exists() {
            // Matches undo::undo_operation's "recovered undo" path: the file already sits
            // back at its original location, so undo just records that instead of failing.
            None
        } else {
            Some(format!(
                "moved file is missing at its expected location ({})",
                operation.to
            ))
        };
    }

    if original.exists() {
        return Some(format!(
            "original path already has a file ({}); undo would overwrite it",
            operation.from
        ));
    }

    None
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct JournalRecoveryReport {
    pub root: String,
    pub journal_path: String,
    pub quarantined_path: String,
}

/// Quarantines a corrupted journal so mutation (execute/undo) can resume. This does not try
/// to repair or reinterpret the bad line: it moves the whole file aside as evidence and lets
/// a fresh journal start from empty. Operations recorded before the corrupt line are no
/// longer undoable through the app after this runs, so callers must get explicit user
/// confirmation before calling it.
pub fn recover_journal(root: impl AsRef<Path>) -> Result<JournalRecoveryReport, JournalError> {
    let guard = PathGuard::new(root).map_err(JournalError::Guard)?;
    let journal_path = guard.root().join(STATE_DIR).join(JOURNAL_FILE);
    let (_, corruption) = read_journal_entries_lenient(&journal_path)?;

    if corruption.is_none() {
        return Err(JournalError::NotCorrupted { path: journal_path });
    }

    let quarantined_path =
        journal_path.with_file_name(format!("{JOURNAL_FILE}.corrupted-{}", unix_ms()));

    fs::rename(&journal_path, &quarantined_path).map_err(|error| JournalError::WriteJournal {
        path: quarantined_path.clone(),
        message: error.to_string(),
    })?;

    Ok(JournalRecoveryReport {
        root: guard.root().display().to_string(),
        journal_path: journal_path.display().to_string(),
        quarantined_path: quarantined_path.display().to_string(),
    })
}

pub fn write_planned_journal(root: impl AsRef<Path>) -> Result<JournalWriteReport, JournalError> {
    let guard = PathGuard::new(root).map_err(JournalError::Guard)?;
    let precheck = precheck_root(guard.root()).map_err(JournalError::Precheck)?;
    let state_dir = guard.root().join(STATE_DIR);
    fs::create_dir_all(&state_dir).map_err(|error| JournalError::CreateStateDir {
        path: state_dir.clone(),
        message: error.to_string(),
    })?;

    let journal_path = state_dir.join(JOURNAL_FILE);
    let mut journal = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&journal_path)
        .map_err(|error| JournalError::OpenJournal {
            path: journal_path.clone(),
            message: error.to_string(),
        })?;

    let created_unix_ms = unix_ms();
    let mut planned_count = 0;
    let mut skipped_count = 0;

    for (index, check) in precheck.checks.iter().enumerate() {
        if check.status != PrecheckStatus::Ready {
            skipped_count += 1;
            continue;
        }

        let entry = JournalEntry {
            operation_id: format!("op-{created_unix_ms}-{index}"),
            status: JournalStatus::Planned,
            action: JournalAction::Move,
            from: check.from.clone(),
            to: check.to.clone(),
            created_unix_ms,
        };
        let line = serde_json::to_string(&entry)
            .map_err(|error| JournalError::Serialize(error.to_string()))?;

        writeln!(journal, "{line}").map_err(|error| JournalError::WriteJournal {
            path: journal_path.clone(),
            message: error.to_string(),
        })?;
        planned_count += 1;
    }

    Ok(JournalWriteReport {
        root: precheck.root,
        journal_path: journal_path.display().to_string(),
        planned_count,
        skipped_count,
    })
}

fn unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

pub fn read_journal_entries(journal_path: &Path) -> Result<Vec<JournalEntry>, JournalError> {
    if !journal_path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(journal_path).map_err(|error| JournalError::ReadJournal {
        path: journal_path.to_path_buf(),
        message: error.to_string(),
    })?;

    content
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(index, line)| {
            serde_json::from_str::<JournalEntry>(line).map_err(|error| JournalError::ParseJournal {
                line: index + 1,
                message: error.to_string(),
            })
        })
        .collect()
}

/// Like `read_journal_entries`, but stops at the first unparseable line instead of failing
/// the whole read. History display can then show everything known up to that point plus a
/// `JournalCorruption` marker, rather than going blank on a single bad line.
fn read_journal_entries_lenient(
    journal_path: &Path,
) -> Result<(Vec<JournalEntry>, Option<JournalCorruption>), JournalError> {
    if !journal_path.exists() {
        return Ok((Vec::new(), None));
    }

    let content = fs::read_to_string(journal_path).map_err(|error| JournalError::ReadJournal {
        path: journal_path.to_path_buf(),
        message: error.to_string(),
    })?;

    let mut entries = Vec::new();

    for (index, line) in content.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }

        match serde_json::from_str::<JournalEntry>(line) {
            Ok(entry) => entries.push(entry),
            Err(error) => {
                return Ok((
                    entries,
                    Some(JournalCorruption {
                        line: index + 1,
                        message: error.to_string(),
                    }),
                ));
            }
        }
    }

    Ok((entries, None))
}

impl fmt::Display for JournalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JournalError::Guard(error) => write!(formatter, "{error}"),
            JournalError::Precheck(error) => write!(formatter, "{error}"),
            JournalError::CreateStateDir { path, message } => {
                write!(
                    formatter,
                    "cannot create state directory {}: {message}",
                    path.display()
                )
            }
            JournalError::ReadJournal { path, message } => {
                write!(
                    formatter,
                    "cannot read journal {}: {message}",
                    path.display()
                )
            }
            JournalError::ParseJournal { line, message } => {
                write!(formatter, "cannot parse journal line {line}: {message}")
            }
            JournalError::OpenJournal { path, message } => {
                write!(
                    formatter,
                    "cannot open journal {}: {message}",
                    path.display()
                )
            }
            JournalError::WriteJournal { path, message } => {
                write!(
                    formatter,
                    "cannot write journal {}: {message}",
                    path.display()
                )
            }
            JournalError::NotCorrupted { path } => {
                write!(
                    formatter,
                    "journal is not corrupted; refusing to recover: {}",
                    path.display()
                )
            }
            JournalError::Serialize(message) => {
                write!(formatter, "cannot serialize journal entry: {message}")
            }
        }
    }
}

impl Error for JournalError {}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use crate::execute::execute_root;
    use crate::undo::undo_operation;

    use super::{
        read_operation_history, recover_journal, write_planned_journal, JournalError,
        JournalStatus, JOURNAL_FILE, STATE_DIR,
    };

    #[test]
    fn writes_ready_items_to_jsonl_journal() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("inbox")).expect("create inbox");
        fs::write(root.join("inbox").join("note.md"), "# note").expect("write note");

        let report = write_planned_journal(&root).expect("journal");
        let journal_path = root.join(STATE_DIR).join(JOURNAL_FILE);
        let journal = fs::read_to_string(&journal_path).expect("read journal");

        assert_eq!(report.planned_count, 1);
        assert_eq!(report.skipped_count, 0);
        assert!(journal.contains("\"status\":\"planned\""));
        assert!(journal.contains("\"from\":\"inbox/note.md\""));
        assert!(journal.contains("\"to\":\"documents/note.md\""));
    }

    #[test]
    fn skips_blocked_items() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("inbox")).expect("create inbox");
        fs::create_dir_all(root.join("documents")).expect("create documents");
        fs::write(root.join("inbox").join("note.md"), "# note").expect("write note");
        fs::write(root.join("documents").join("note.md"), "# existing").expect("write existing");

        let report = write_planned_journal(&root).expect("journal");
        let journal_path = root.join(STATE_DIR).join(JOURNAL_FILE);
        let journal = fs::read_to_string(&journal_path).expect("read journal");

        assert_eq!(report.planned_count, 0);
        assert_eq!(report.skipped_count, 1);
        assert!(journal.is_empty());
    }

    #[test]
    fn reads_operation_history_from_execute_journal() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("inbox")).expect("create inbox");
        fs::write(root.join("inbox").join("note.md"), "# note").expect("write note");

        execute_root(&root).expect("execute");
        let history = read_operation_history(&root).expect("history");

        assert_eq!(history.operations.len(), 1);
        assert_eq!(history.operations[0].from, "inbox/note.md");
        assert_eq!(history.operations[0].to, "documents/note.md");
        assert_eq!(history.operations[0].latest_status, JournalStatus::Executed);
        assert!(history.operations[0].can_undo);
    }

    #[test]
    fn history_marks_selected_undo_as_not_undoable() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("inbox")).expect("create inbox");
        fs::write(root.join("inbox").join("note.md"), "# note").expect("write note");
        execute_root(&root).expect("execute");
        let operation_id = read_operation_history(&root).expect("history").operations[0]
            .operation_id
            .clone();

        undo_operation(&root, &operation_id).expect("undo selected operation");
        let history = read_operation_history(&root).expect("history after undo");

        assert_eq!(history.operations[0].latest_status, JournalStatus::Undone);
        assert!(!history.operations[0].can_undo);
        assert_eq!(
            history.operations[0].undo_blocked_reason.as_deref(),
            Some("already undone")
        );
    }

    #[test]
    fn history_reports_reason_when_operation_not_yet_executed() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("inbox")).expect("create inbox");
        fs::write(root.join("inbox").join("note.md"), "# note").expect("write note");

        write_planned_journal(&root).expect("plan journal without executing");
        let history = read_operation_history(&root).expect("history");

        assert_eq!(history.operations.len(), 1);
        assert!(!history.operations[0].can_undo);
        assert_eq!(
            history.operations[0].undo_blocked_reason.as_deref(),
            Some("operation has not been executed yet")
        );
    }

    #[test]
    fn history_reports_reason_when_destination_missing() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("inbox")).expect("create inbox");
        fs::write(root.join("inbox").join("note.md"), "# note").expect("write note");
        execute_root(&root).expect("execute");

        fs::remove_file(root.join("documents").join("note.md")).expect("remove moved file");
        let history = read_operation_history(&root).expect("history");

        assert!(!history.operations[0].can_undo);
        let reason = history.operations[0]
            .undo_blocked_reason
            .as_deref()
            .expect("reason present");
        assert!(reason.contains("missing"));
    }

    #[test]
    fn history_reports_reason_when_original_path_occupied() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("inbox")).expect("create inbox");
        fs::write(root.join("inbox").join("note.md"), "# note").expect("write note");
        execute_root(&root).expect("execute");

        fs::write(root.join("inbox").join("note.md"), "# new").expect("recreate original path");
        let history = read_operation_history(&root).expect("history");

        assert!(!history.operations[0].can_undo);
        let reason = history.operations[0]
            .undo_blocked_reason
            .as_deref()
            .expect("reason present");
        assert!(reason.contains("overwrite"));
    }

    #[test]
    fn history_stays_undoable_when_file_already_restored_externally() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("inbox")).expect("create inbox");
        fs::write(root.join("inbox").join("note.md"), "# note").expect("write note");
        execute_root(&root).expect("execute");

        fs::rename(
            root.join("documents").join("note.md"),
            root.join("inbox").join("note.md"),
        )
        .expect("simulate restore before undo runs");
        let history = read_operation_history(&root).expect("history");

        assert!(history.operations[0].can_undo);
        assert!(history.operations[0].undo_blocked_reason.is_none());
    }

    #[test]
    fn history_reports_corruption_and_keeps_entries_before_it() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("inbox")).expect("create inbox");
        fs::write(root.join("inbox").join("note.md"), "# note").expect("write note");
        execute_root(&root).expect("execute");

        let journal_path = root.join(STATE_DIR).join(JOURNAL_FILE);
        let mut journal = fs::read_to_string(&journal_path).expect("read journal");
        journal.push_str("{not valid json\n");
        fs::write(&journal_path, journal).expect("append corrupt line");

        let history = read_operation_history(&root).expect("history tolerates corruption");

        assert_eq!(history.operations.len(), 1);
        assert!(history.operations[0].can_undo);
        let corruption = history.corruption.expect("corruption reported");
        assert_eq!(corruption.line, 3);
    }

    #[test]
    fn recover_journal_quarantines_broken_file_and_starts_fresh() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("inbox")).expect("create inbox");
        fs::write(root.join("inbox").join("note.md"), "# note").expect("write note");
        execute_root(&root).expect("execute");

        let journal_path = root.join(STATE_DIR).join(JOURNAL_FILE);
        let mut journal = fs::read_to_string(&journal_path).expect("read journal");
        journal.push_str("{not valid json\n");
        fs::write(&journal_path, journal).expect("append corrupt line");

        let report = recover_journal(&root).expect("recover journal");

        assert!(!journal_path.exists());
        assert!(std::path::Path::new(&report.quarantined_path).exists());

        let history = read_operation_history(&root).expect("history after recovery");
        assert!(history.operations.is_empty());
        assert!(history.corruption.is_none());
    }

    #[test]
    fn recover_journal_rejects_healthy_journal() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("inbox")).expect("create inbox");
        fs::write(root.join("inbox").join("note.md"), "# note").expect("write note");
        execute_root(&root).expect("execute");

        let error = recover_journal(&root).expect_err("reject healthy journal");

        assert!(matches!(error, JournalError::NotCorrupted { .. }));
    }
}
