use std::error::Error;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

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
pub struct JournalEntry {
    pub operation_id: String,
    pub status: JournalStatus,
    pub action: JournalAction,
    pub from: String,
    pub to: String,
    pub created_unix_ms: u128,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JournalStatus {
    Planned,
    Executed,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JournalAction {
    Move,
}

#[derive(Debug)]
pub enum JournalError {
    Guard(PathGuardError),
    Precheck(PrecheckError),
    CreateStateDir { path: PathBuf, message: String },
    OpenJournal { path: PathBuf, message: String },
    WriteJournal { path: PathBuf, message: String },
    Serialize(String),
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

    use super::{write_planned_journal, JOURNAL_FILE, STATE_DIR};

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
}
