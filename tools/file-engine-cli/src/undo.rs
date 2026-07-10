use std::collections::HashSet;
use std::error::Error;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::journal::{JournalAction, JournalEntry, JournalStatus, JOURNAL_FILE, STATE_DIR};
use crate::path_guard::{PathGuard, PathGuardError};

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct UndoReport {
    pub root: String,
    pub journal_path: String,
    pub undone_count: usize,
    pub skipped_count: usize,
    pub results: Vec<UndoResult>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct UndoResult {
    pub from: String,
    pub to: String,
    pub status: UndoStatus,
    pub reason: Option<String>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UndoStatus {
    Undone,
    Skipped,
}

#[derive(Debug)]
pub enum UndoError {
    Guard(PathGuardError),
    ReadJournal {
        path: PathBuf,
        message: String,
    },
    ParseJournal {
        line: usize,
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

pub fn undo_root(root: impl AsRef<Path>) -> Result<UndoReport, UndoError> {
    let guard = PathGuard::new(root).map_err(UndoError::Guard)?;
    let journal_path = guard.root().join(STATE_DIR).join(JOURNAL_FILE);
    let entries = read_journal(&journal_path)?;
    let undone_operation_ids = entries
        .iter()
        .filter(|entry| entry.status == JournalStatus::Undone)
        .map(|entry| entry.operation_id.clone())
        .collect::<HashSet<_>>();
    let mut journal = open_journal(&journal_path)?;

    let mut undone_count = 0;
    let mut skipped_count = 0;
    let mut results = Vec::new();

    for entry in entries.iter().rev() {
        if entry.status != JournalStatus::Executed
            || entry.action != JournalAction::Move
            || undone_operation_ids.contains(&entry.operation_id)
        {
            continue;
        }

        let current_path = match guard.resolve_existing(&entry.to) {
            Ok(path) => path,
            Err(error) => {
                skipped_count += 1;
                results.push(skipped_result(entry, Some(error.to_string())));
                continue;
            }
        };

        let original_path = guard.root().join(&entry.from);
        let parent = original_path
            .parent()
            .ok_or_else(|| UndoError::Guard(PathGuardError::MissingPath(original_path.clone())))?;
        fs::create_dir_all(parent).map_err(|error| UndoError::CreateParentDir {
            path: parent.to_path_buf(),
            message: error.to_string(),
        })?;

        let original_path = guard
            .resolve_for_create(&entry.from)
            .map_err(UndoError::Guard)?;
        if original_path.exists() {
            skipped_count += 1;
            results.push(skipped_result(
                entry,
                Some("original path already exists; refusing to overwrite".to_string()),
            ));
            continue;
        }

        append_journal(
            &mut journal,
            &journal_path,
            undo_entry(entry, JournalStatus::UndoPlanned),
        )?;
        fs::rename(&current_path, &original_path).map_err(|error| UndoError::Move {
            from: current_path,
            to: original_path,
            message: error.to_string(),
        })?;
        append_journal(
            &mut journal,
            &journal_path,
            undo_entry(entry, JournalStatus::Undone),
        )?;

        undone_count += 1;
        results.push(UndoResult {
            from: entry.to.clone(),
            to: entry.from.clone(),
            status: UndoStatus::Undone,
            reason: None,
        });
    }

    Ok(UndoReport {
        root: guard.root().display().to_string(),
        journal_path: journal_path.display().to_string(),
        undone_count,
        skipped_count,
        results,
    })
}

fn read_journal(journal_path: &Path) -> Result<Vec<JournalEntry>, UndoError> {
    let content = fs::read_to_string(journal_path).map_err(|error| UndoError::ReadJournal {
        path: journal_path.to_path_buf(),
        message: error.to_string(),
    })?;

    content
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(index, line)| {
            serde_json::from_str::<JournalEntry>(line).map_err(|error| UndoError::ParseJournal {
                line: index + 1,
                message: error.to_string(),
            })
        })
        .collect()
}

fn open_journal(journal_path: &Path) -> Result<fs::File, UndoError> {
    OpenOptions::new()
        .append(true)
        .open(journal_path)
        .map_err(|error| UndoError::OpenJournal {
            path: journal_path.to_path_buf(),
            message: error.to_string(),
        })
}

fn append_journal(
    journal: &mut fs::File,
    journal_path: &Path,
    entry: JournalEntry,
) -> Result<(), UndoError> {
    let line =
        serde_json::to_string(&entry).map_err(|error| UndoError::Serialize(error.to_string()))?;
    writeln!(journal, "{line}").map_err(|error| UndoError::WriteJournal {
        path: journal_path.to_path_buf(),
        message: error.to_string(),
    })
}

fn undo_entry(entry: &JournalEntry, status: JournalStatus) -> JournalEntry {
    JournalEntry {
        operation_id: entry.operation_id.clone(),
        status,
        action: JournalAction::Move,
        from: entry.to.clone(),
        to: entry.from.clone(),
        created_unix_ms: unix_ms(),
    }
}

fn skipped_result(entry: &JournalEntry, reason: Option<String>) -> UndoResult {
    UndoResult {
        from: entry.to.clone(),
        to: entry.from.clone(),
        status: UndoStatus::Skipped,
        reason,
    }
}

fn unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

impl fmt::Display for UndoError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UndoError::Guard(error) => write!(formatter, "{error}"),
            UndoError::ReadJournal { path, message } => {
                write!(
                    formatter,
                    "cannot read journal {}: {message}",
                    path.display()
                )
            }
            UndoError::ParseJournal { line, message } => {
                write!(formatter, "cannot parse journal line {line}: {message}")
            }
            UndoError::CreateParentDir { path, message } => {
                write!(
                    formatter,
                    "cannot create original parent {}: {message}",
                    path.display()
                )
            }
            UndoError::OpenJournal { path, message } => {
                write!(
                    formatter,
                    "cannot open journal {}: {message}",
                    path.display()
                )
            }
            UndoError::WriteJournal { path, message } => {
                write!(
                    formatter,
                    "cannot write journal {}: {message}",
                    path.display()
                )
            }
            UndoError::Move { from, to, message } => {
                write!(
                    formatter,
                    "cannot move {} to {}: {message}",
                    from.display(),
                    to.display()
                )
            }
            UndoError::Serialize(message) => {
                write!(formatter, "cannot serialize undo journal: {message}")
            }
        }
    }
}

impl Error for UndoError {}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use crate::execute::execute_root;

    use super::undo_root;

    #[test]
    fn restores_executed_move() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("inbox")).expect("create inbox");
        fs::write(root.join("inbox").join("note.md"), "# note").expect("write note");
        execute_root(&root).expect("execute");

        let report = undo_root(&root).expect("undo");
        let journal =
            fs::read_to_string(root.join(".housemouse").join("journal.jsonl")).expect("journal");

        assert_eq!(report.undone_count, 1);
        assert_eq!(report.skipped_count, 0);
        assert!(root.join("inbox").join("note.md").exists());
        assert!(!root.join("documents").join("note.md").exists());
        assert!(journal.contains("\"status\":\"undo_planned\""));
        assert!(journal.contains("\"status\":\"undone\""));
    }

    #[test]
    fn refuses_to_overwrite_original_path() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("inbox")).expect("create inbox");
        fs::write(root.join("inbox").join("note.md"), "# note").expect("write note");
        execute_root(&root).expect("execute");
        fs::write(root.join("inbox").join("note.md"), "# new").expect("write replacement");

        let report = undo_root(&root).expect("undo");
        let original = fs::read_to_string(root.join("inbox").join("note.md")).expect("original");

        assert_eq!(report.undone_count, 0);
        assert_eq!(report.skipped_count, 1);
        assert_eq!(original, "# new");
        assert!(root.join("documents").join("note.md").exists());
    }
}
