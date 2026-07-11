use std::error::Error;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::fs_safety::is_link_or_reparse_point;
use crate::journal::{JournalAction, JournalEntry, JournalError, JournalStatus, JournalStore};
use crate::path_guard::{PathGuard, PathGuardError};

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct CreateFileReport {
    pub root: String,
    pub created_path: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct RenameFileReport {
    pub root: String,
    pub journal_path: String,
    pub operation_id: String,
    pub from: String,
    pub to: String,
}

#[derive(Debug)]
pub enum FileOpError {
    Guard(PathGuardError),
    Journal(JournalError),
    InvalidName(String),
    NotAFile {
        path: PathBuf,
    },
    BlockedLinkOrReparse {
        path: PathBuf,
    },
    DestinationExists {
        path: PathBuf,
    },
    Create {
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

pub fn create_empty_file(
    root: impl AsRef<Path>,
    relative_path: impl AsRef<str>,
) -> Result<CreateFileReport, FileOpError> {
    let guard = PathGuard::new(root).map_err(FileOpError::Guard)?;
    let relative_path = relative_path.as_ref();
    let target = guard
        .resolve_for_create(relative_path)
        .map_err(FileOpError::Guard)?;

    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&target)
        .map_err(|error| {
            if target.exists() {
                FileOpError::DestinationExists {
                    path: target.clone(),
                }
            } else {
                FileOpError::Create {
                    path: target.clone(),
                    message: error.to_string(),
                }
            }
        })?;

    Ok(CreateFileReport {
        root: guard.root().display().to_string(),
        created_path: relative_path.to_string(),
    })
}

pub fn rename_file(
    root: impl AsRef<Path>,
    relative_path: impl AsRef<str>,
    new_name: impl AsRef<str>,
) -> Result<RenameFileReport, FileOpError> {
    let guard = PathGuard::new(root).map_err(FileOpError::Guard)?;
    let relative_path = relative_path.as_ref();
    let new_name = validate_file_name(new_name.as_ref())?;
    let source = guard
        .resolve_existing(relative_path)
        .map_err(FileOpError::Guard)?;
    let metadata = fs::symlink_metadata(guard.root().join(relative_path)).map_err(|error| {
        FileOpError::Create {
            path: guard.root().join(relative_path),
            message: error.to_string(),
        }
    })?;

    if is_link_or_reparse_point(&metadata, metadata.file_type()) {
        return Err(FileOpError::BlockedLinkOrReparse {
            path: guard.root().join(relative_path),
        });
    }

    if !metadata.is_file() {
        return Err(FileOpError::NotAFile {
            path: guard.root().join(relative_path),
        });
    }

    let to = sibling_relative_path(relative_path, &new_name);
    let destination = guard.resolve_for_create(&to).map_err(FileOpError::Guard)?;
    if destination.exists() {
        return Err(FileOpError::DestinationExists { path: destination });
    }

    let store = JournalStore::open(guard.root()).map_err(FileOpError::Journal)?;
    let operation_id = format!("rename-{}", unix_nanos());
    let planned = journal_entry(&operation_id, JournalStatus::Planned, relative_path, &to);
    store.append(&planned).map_err(FileOpError::Journal)?;

    let destination = guard.resolve_for_create(&to).map_err(FileOpError::Guard)?;
    fs::rename(&source, &destination).map_err(|error| FileOpError::Move {
        from: source,
        to: destination,
        message: error.to_string(),
    })?;

    let executed = journal_entry(&operation_id, JournalStatus::Executed, relative_path, &to);
    store.append(&executed).map_err(FileOpError::Journal)?;

    Ok(RenameFileReport {
        root: guard.root().display().to_string(),
        journal_path: store.journal_path_display(),
        operation_id,
        from: relative_path.to_string(),
        to,
    })
}

fn validate_file_name(name: &str) -> Result<String, FileOpError> {
    let trimmed = name.trim();
    if trimmed.is_empty()
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed == "."
        || trimmed == ".."
    {
        return Err(FileOpError::InvalidName(name.to_string()));
    }

    Ok(trimmed.to_string())
}

fn sibling_relative_path(relative_path: &str, new_name: &str) -> String {
    Path::new(relative_path)
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map(|parent| parent.join(new_name).to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|| new_name.to_string())
}

fn journal_entry(operation_id: &str, status: JournalStatus, from: &str, to: &str) -> JournalEntry {
    JournalEntry {
        operation_id: operation_id.to_string(),
        status,
        action: JournalAction::Move,
        from: from.to_string(),
        to: to.to_string(),
        created_unix_ms: unix_ms(),
    }
}

fn unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

fn unix_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0)
}

impl fmt::Display for FileOpError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FileOpError::Guard(error) => write!(formatter, "{error}"),
            FileOpError::Journal(error) => write!(formatter, "{error}"),
            FileOpError::InvalidName(name) => write!(formatter, "invalid file name: {name}"),
            FileOpError::NotAFile { path } => {
                write!(formatter, "target is not a file: {}", path.display())
            }
            FileOpError::BlockedLinkOrReparse { path } => {
                write!(
                    formatter,
                    "refusing to rename symlink, junction, or reparse point: {}",
                    path.display()
                )
            }
            FileOpError::DestinationExists { path } => {
                write!(formatter, "destination already exists: {}", path.display())
            }
            FileOpError::Create { path, message } => {
                write!(formatter, "cannot create {}: {message}", path.display())
            }
            FileOpError::Move { from, to, message } => {
                write!(
                    formatter,
                    "cannot move {} to {}: {message}",
                    from.display(),
                    to.display()
                )
            }
            FileOpError::Serialize(message) => {
                write!(formatter, "cannot serialize file op report: {message}")
            }
        }
    }
}

impl Error for FileOpError {}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use crate::journal::{read_operation_history, JournalAction, JournalStatus};
    use crate::undo::undo_operation;

    use super::{create_empty_file, rename_file, FileOpError};

    #[test]
    fn creates_empty_file_without_overwrite() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("notes")).expect("create root");

        let report = create_empty_file(&root, "notes/new.txt").expect("create file");

        assert_eq!(report.created_path, "notes/new.txt");
        assert!(root.join("notes").join("new.txt").exists());
        assert!(matches!(
            create_empty_file(&root, "notes/new.txt"),
            Err(FileOpError::DestinationExists { .. })
        ));
    }

    #[test]
    fn renames_file_with_journal_and_undo() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("notes")).expect("create root");
        fs::write(root.join("notes").join("old.txt"), "hello").expect("write file");

        let report = rename_file(&root, "notes/old.txt", "new.txt").expect("rename");
        let history = read_operation_history(&root).expect("history");

        assert_eq!(report.to, "notes/new.txt");
        assert!(!root.join("notes").join("old.txt").exists());
        assert!(root.join("notes").join("new.txt").exists());
        assert_eq!(history.operations[0].action, JournalAction::Move);
        assert_eq!(history.operations[0].latest_status, JournalStatus::Executed);

        let undo = undo_operation(&root, &report.operation_id).expect("undo rename");

        assert_eq!(undo.undone_count, 1);
        assert!(root.join("notes").join("old.txt").exists());
    }
}
