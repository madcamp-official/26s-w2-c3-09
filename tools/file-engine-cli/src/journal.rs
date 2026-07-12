use std::error::Error;
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};

use crate::db::{block_on, db_path_for_root, open_root_db, DbError};
use crate::path_guard::{PathGuard, PathGuardError};
use crate::precondition::{precheck_root, PrecheckError, PrecheckStatus};

pub const STATE_DIR: &str = ".housemouse";
pub const TRASH_DIR: &str = ".housemouse_trash";
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

/// Describes the first unreadable journal row. `operations` in `OperationHistoryReport` only
/// reflects entries before this point: once one row is untrustworthy, later "undone" markers
/// could reference operations we can no longer verify, so history stops there instead of
/// guessing. (`line` is the 1-based row ordinal; the name is kept for API stability.)
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

impl JournalStatus {
    fn as_db_str(&self) -> &'static str {
        match self {
            JournalStatus::Planned => "planned",
            JournalStatus::Executed => "executed",
            JournalStatus::UndoPlanned => "undo_planned",
            JournalStatus::Undone => "undone",
        }
    }

    fn from_db_str(value: &str) -> Option<Self> {
        match value {
            "planned" => Some(JournalStatus::Planned),
            "executed" => Some(JournalStatus::Executed),
            "undo_planned" => Some(JournalStatus::UndoPlanned),
            "undone" => Some(JournalStatus::Undone),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JournalAction {
    Move,
    Trash,
    ReadmeWrite,
}

impl JournalAction {
    fn as_db_str(&self) -> &'static str {
        match self {
            JournalAction::Move => "move",
            JournalAction::Trash => "trash",
            JournalAction::ReadmeWrite => "readme_write",
        }
    }

    fn from_db_str(value: &str) -> Option<Self> {
        match value {
            "move" => Some(JournalAction::Move),
            "trash" => Some(JournalAction::Trash),
            "readme_write" => Some(JournalAction::ReadmeWrite),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub enum JournalError {
    Guard(PathGuardError),
    Precheck(PrecheckError),
    Db(DbError),
    /// A strict read (execute/undo) hit a row whose stored status/action is not recognized.
    /// These paths refuse to mutate on an untrustworthy journal.
    Corrupt {
        line: usize,
        message: String,
    },
    NotCorrupted {
        path: PathBuf,
    },
    WriteQuarantine {
        path: PathBuf,
        message: String,
    },
    Serialize(String),
}

/// SQLite-backed operation journal for one managed root. Every execute/undo/plan event is a
/// row in `operation_journal`; this type is the single place that reads and appends them,
/// replacing the append-to-JSONL logic that used to live separately in execute and undo.
pub struct JournalStore {
    pool: SqlitePool,
    db_path: PathBuf,
}

impl JournalStore {
    pub fn open(root: &Path) -> Result<Self, JournalError> {
        let pool = open_root_db(root).map_err(JournalError::Db)?;
        Ok(Self {
            pool,
            db_path: db_path_for_root(root),
        })
    }

    pub fn journal_path_display(&self) -> String {
        self.db_path.display().to_string()
    }

    pub fn append(&self, entry: &JournalEntry) -> Result<(), JournalError> {
        block_on(async {
            sqlx::query(
                "INSERT INTO operation_journal
                    (operation_id, status, action, from_path, to_path, created_unix_ms)
                 VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(&entry.operation_id)
            .bind(entry.status.as_db_str())
            .bind(entry.action.as_db_str())
            .bind(&entry.from)
            .bind(&entry.to)
            .bind(entry.created_unix_ms as i64)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(query_error)
        })
        .map_err(JournalError::Db)
    }

    /// Reads every entry in insertion order, failing if any row is unrecognized. Used by
    /// execute/undo, which must not proceed on a journal they cannot fully trust.
    pub fn read_all(&self) -> Result<Vec<JournalEntry>, JournalError> {
        let (entries, corruption) = self.read_all_lenient()?;
        if let Some(corruption) = corruption {
            return Err(JournalError::Corrupt {
                line: corruption.line,
                message: corruption.message,
            });
        }
        Ok(entries)
    }

    /// Reads entries in order but stops at the first unrecognized row, returning what came
    /// before plus a corruption marker — the tolerant read history display uses so a single
    /// bad row does not blank the whole timeline.
    fn read_all_lenient(
        &self,
    ) -> Result<(Vec<JournalEntry>, Option<JournalCorruption>), JournalError> {
        let raw = self.read_raw()?;
        let mut entries = Vec::with_capacity(raw.len());

        for (index, row) in raw.iter().enumerate() {
            let status = JournalStatus::from_db_str(&row.status);
            let action = JournalAction::from_db_str(&row.action);

            match (status, action) {
                (Some(status), Some(action)) => entries.push(JournalEntry {
                    operation_id: row.operation_id.clone(),
                    status,
                    action,
                    from: row.from_path.clone(),
                    to: row.to_path.clone(),
                    created_unix_ms: row.created_unix_ms as u128,
                }),
                _ => {
                    return Ok((
                        entries,
                        Some(JournalCorruption {
                            line: index + 1,
                            message: format!(
                                "unrecognized journal row (status='{}', action='{}')",
                                row.status, row.action
                            ),
                        }),
                    ));
                }
            }
        }

        Ok((entries, None))
    }

    fn read_raw(&self) -> Result<Vec<RawJournalRow>, JournalError> {
        block_on(async {
            let rows = sqlx::query(
                "SELECT seq, operation_id, status, action, from_path, to_path, created_unix_ms
                 FROM operation_journal
                 ORDER BY seq",
            )
            .fetch_all(&self.pool)
            .await
            .map_err(query_error)?;

            let mut parsed = Vec::with_capacity(rows.len());
            for row in rows {
                parsed.push(RawJournalRow {
                    seq: row.try_get("seq").map_err(query_error)?,
                    operation_id: row.try_get("operation_id").map_err(query_error)?,
                    status: row.try_get("status").map_err(query_error)?,
                    action: row.try_get("action").map_err(query_error)?,
                    from_path: row.try_get("from_path").map_err(query_error)?,
                    to_path: row.try_get("to_path").map_err(query_error)?,
                    created_unix_ms: row.try_get("created_unix_ms").map_err(query_error)?,
                });
            }
            Ok(parsed)
        })
        .map_err(JournalError::Db)
    }

    fn clear(&self) -> Result<(), JournalError> {
        block_on(async {
            sqlx::query("DELETE FROM operation_journal")
                .execute(&self.pool)
                .await
                .map(|_| ())
                .map_err(query_error)
        })
        .map_err(JournalError::Db)
    }
}

#[derive(Serialize)]
struct RawJournalRow {
    seq: i64,
    operation_id: String,
    status: String,
    action: String,
    from_path: String,
    to_path: String,
    created_unix_ms: i64,
}

pub fn read_operation_history(
    root: impl AsRef<Path>,
) -> Result<OperationHistoryReport, JournalError> {
    let guard = PathGuard::new(root).map_err(JournalError::Guard)?;
    let store = JournalStore::open(guard.root())?;
    let (entries, corruption) = store.read_all_lenient()?;
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
        journal_path: store.journal_path_display(),
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
    if !matches!(
        operation.action,
        JournalAction::Move | JournalAction::Trash | JournalAction::ReadmeWrite
    ) {
        return None;
    }

    if operation.action == JournalAction::ReadmeWrite {
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

/// Quarantines a corrupted journal so mutation (execute/undo) can resume. It does not repair
/// or reinterpret the bad row: it dumps every current row to a `journal.jsonl.corrupted-<ts>`
/// file as evidence, then clears the table so a fresh journal starts empty. Operations
/// recorded before the corruption are no longer undoable through the app afterward, so callers
/// must get explicit user confirmation first.
pub fn recover_journal(root: impl AsRef<Path>) -> Result<JournalRecoveryReport, JournalError> {
    let guard = PathGuard::new(root).map_err(JournalError::Guard)?;
    let store = JournalStore::open(guard.root())?;
    let (_, corruption) = store.read_all_lenient()?;

    if corruption.is_none() {
        return Err(JournalError::NotCorrupted {
            path: store.db_path.clone(),
        });
    }

    let quarantined_path = store
        .db_path
        .with_file_name(format!("{JOURNAL_FILE}.corrupted-{}", unix_ms()));
    let raw = store.read_raw()?;

    let mut dump =
        fs::File::create(&quarantined_path).map_err(|error| JournalError::WriteQuarantine {
            path: quarantined_path.clone(),
            message: error.to_string(),
        })?;
    for row in &raw {
        let line = serde_json::to_string(row)
            .map_err(|error| JournalError::Serialize(error.to_string()))?;
        writeln!(dump, "{line}").map_err(|error| JournalError::WriteQuarantine {
            path: quarantined_path.clone(),
            message: error.to_string(),
        })?;
    }

    store.clear()?;

    Ok(JournalRecoveryReport {
        root: guard.root().display().to_string(),
        journal_path: store.journal_path_display(),
        quarantined_path: quarantined_path.display().to_string(),
    })
}

pub fn write_planned_journal(root: impl AsRef<Path>) -> Result<JournalWriteReport, JournalError> {
    let guard = PathGuard::new(root).map_err(JournalError::Guard)?;
    let precheck = precheck_root(guard.root()).map_err(JournalError::Precheck)?;
    let store = JournalStore::open(guard.root())?;

    let created_unix_ms = unix_ms();
    let mut planned_count = 0;
    let mut skipped_count = 0;

    for (index, check) in precheck.checks.iter().enumerate() {
        if check.status != PrecheckStatus::Ready {
            skipped_count += 1;
            continue;
        }

        store.append(&JournalEntry {
            operation_id: format!("op-{created_unix_ms}-{index}"),
            status: JournalStatus::Planned,
            action: JournalAction::Move,
            from: check.from.clone(),
            to: check.to.clone(),
            created_unix_ms,
        })?;
        planned_count += 1;
    }

    Ok(JournalWriteReport {
        root: precheck.root,
        journal_path: store.journal_path_display(),
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

fn query_error(error: sqlx::Error) -> DbError {
    DbError::Query {
        message: error.to_string(),
    }
}

impl fmt::Display for JournalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JournalError::Guard(error) => write!(formatter, "{error}"),
            JournalError::Precheck(error) => write!(formatter, "{error}"),
            JournalError::Db(error) => write!(formatter, "{error}"),
            JournalError::Corrupt { line, message } => {
                write!(formatter, "cannot parse journal row {line}: {message}")
            }
            JournalError::NotCorrupted { path } => {
                write!(
                    formatter,
                    "journal is not corrupted; refusing to recover: {}",
                    path.display()
                )
            }
            JournalError::WriteQuarantine { path, message } => {
                write!(
                    formatter,
                    "cannot write quarantine file {}: {message}",
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
    use std::path::Path;

    use tempfile::tempdir;

    use crate::execute::execute_root;
    use crate::undo::undo_operation;

    use super::{
        read_operation_history, recover_journal, write_planned_journal, JournalError, JournalStatus,
    };

    /// Injects an unrecognized row directly into the journal table to simulate a corrupted
    /// journal (the SQLite analog of the old "append a bad JSONL line").
    fn corrupt_journal(root: &Path) {
        let pool = crate::db::open_root_db(root).expect("open db");
        crate::db::block_on(async {
            sqlx::query(
                "INSERT INTO operation_journal
                    (operation_id, status, action, from_path, to_path, created_unix_ms)
                 VALUES ('op-bad', 'garbage', 'move', 'a', 'b', 1)",
            )
            .execute(&pool)
            .await
            .expect("insert bad row");
        });
    }

    #[test]
    fn writes_ready_items_to_journal() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("inbox")).expect("create inbox");
        fs::write(root.join("inbox").join("note.md"), "# note").expect("write note");

        let report = write_planned_journal(&root).expect("journal");
        let history = read_operation_history(&root).expect("history");

        assert_eq!(report.planned_count, 1);
        assert_eq!(report.skipped_count, 0);
        assert_eq!(history.operations.len(), 1);
        assert_eq!(history.operations[0].latest_status, JournalStatus::Planned);
        assert_eq!(history.operations[0].from, "inbox/note.md");
        assert_eq!(history.operations[0].to, "documents/note.md");
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
        let history = read_operation_history(&root).expect("history");

        assert_eq!(report.planned_count, 0);
        assert_eq!(report.skipped_count, 1);
        assert!(history.operations.is_empty());
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

        // Execute writes a planned + executed row; the injected bad row is the third.
        corrupt_journal(&root);

        let history = read_operation_history(&root).expect("history tolerates corruption");

        assert_eq!(history.operations.len(), 1);
        assert!(history.operations[0].can_undo);
        let corruption = history.corruption.expect("corruption reported");
        assert_eq!(corruption.line, 3);
    }

    #[test]
    fn recover_journal_quarantines_broken_journal_and_starts_fresh() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("inbox")).expect("create inbox");
        fs::write(root.join("inbox").join("note.md"), "# note").expect("write note");
        execute_root(&root).expect("execute");
        corrupt_journal(&root);

        let report = recover_journal(&root).expect("recover journal");

        assert!(Path::new(&report.quarantined_path).exists());

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
