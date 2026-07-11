use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::fs_safety::is_link_or_reparse_point;
use crate::journal::{
    JournalAction, JournalEntry, JournalError, JournalStatus, JournalStore, TRASH_DIR,
};
use crate::path_guard::{PathGuard, PathGuardError};

const TRASHED_FILE_NAME: &str = "file";
const TRASH_METADATA_FILE_NAME: &str = "original.json";

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct TrashReport {
    pub root: String,
    pub journal_path: String,
    pub operation_id: String,
    pub original_path: String,
    pub trashed_path: String,
    pub metadata_path: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct TrashMetadata {
    pub operation_id: String,
    pub original_path: String,
    pub trashed_path: String,
    pub size_bytes: u64,
    pub modified_unix_ms: Option<u128>,
    pub trashed_unix_ms: u128,
}

#[derive(Debug)]
pub enum TrashError {
    Guard(PathGuardError),
    Journal(JournalError),
    Metadata {
        path: PathBuf,
        message: String,
    },
    NotAFile {
        path: PathBuf,
    },
    BlockedLinkOrReparse {
        path: PathBuf,
    },
    CreateTrashDir {
        path: PathBuf,
        message: String,
    },
    WriteMetadata {
        path: PathBuf,
        message: String,
    },
    DestinationExists {
        path: PathBuf,
    },
    Move {
        from: PathBuf,
        to: PathBuf,
        message: String,
    },
    Serialize(String),
}

pub fn trash_file(
    root: impl AsRef<Path>,
    relative_path: impl AsRef<str>,
) -> Result<TrashReport, TrashError> {
    let guard = PathGuard::new(root).map_err(TrashError::Guard)?;
    let relative_path = relative_path.as_ref();
    let source = guard
        .resolve_existing(relative_path)
        .map_err(TrashError::Guard)?;
    let metadata = fs::symlink_metadata(guard.root().join(relative_path)).map_err(|error| {
        TrashError::Metadata {
            path: guard.root().join(relative_path),
            message: error.to_string(),
        }
    })?;
    let file_type = metadata.file_type();

    if is_link_or_reparse_point(&metadata, file_type) {
        return Err(TrashError::BlockedLinkOrReparse {
            path: guard.root().join(relative_path),
        });
    }

    if !metadata.is_file() {
        return Err(TrashError::NotAFile {
            path: guard.root().join(relative_path),
        });
    }

    let store = JournalStore::open(guard.root()).map_err(TrashError::Journal)?;
    let created_unix_ms = unix_ms();
    let operation_id = format!("trash-{}", unix_nanos());
    let trash_relative_dir = format!("{TRASH_DIR}/{operation_id}");
    let trashed_relative_path = format!("{trash_relative_dir}/{TRASHED_FILE_NAME}");
    let metadata_relative_path = format!("{trash_relative_dir}/{TRASH_METADATA_FILE_NAME}");

    store
        .append(&journal_entry(
            &operation_id,
            JournalStatus::Planned,
            relative_path,
            &trashed_relative_path,
            created_unix_ms,
        ))
        .map_err(TrashError::Journal)?;

    let trash_dir = guard.root().join(&trash_relative_dir);
    fs::create_dir_all(&trash_dir).map_err(|error| TrashError::CreateTrashDir {
        path: trash_dir.clone(),
        message: error.to_string(),
    })?;

    let trashed_path = guard
        .resolve_for_create(&trashed_relative_path)
        .map_err(TrashError::Guard)?;
    if trashed_path.exists() {
        return Err(TrashError::DestinationExists { path: trashed_path });
    }

    let metadata_path = guard
        .resolve_for_create(&metadata_relative_path)
        .map_err(TrashError::Guard)?;
    if metadata_path.exists() {
        return Err(TrashError::DestinationExists {
            path: metadata_path,
        });
    }

    let trash_metadata = TrashMetadata {
        operation_id: operation_id.clone(),
        original_path: relative_path.to_string(),
        trashed_path: trashed_relative_path.clone(),
        size_bytes: metadata.len(),
        modified_unix_ms: modified_unix_ms(&metadata),
        trashed_unix_ms: created_unix_ms,
    };
    let metadata_json = serde_json::to_string_pretty(&trash_metadata)
        .map_err(|error| TrashError::Serialize(error.to_string()))?;
    fs::write(&metadata_path, metadata_json).map_err(|error| TrashError::WriteMetadata {
        path: metadata_path.clone(),
        message: error.to_string(),
    })?;

    fs::rename(&source, &trashed_path).map_err(|error| TrashError::Move {
        from: source,
        to: trashed_path.clone(),
        message: error.to_string(),
    })?;

    store
        .append(&journal_entry(
            &operation_id,
            JournalStatus::Executed,
            relative_path,
            &trashed_relative_path,
            unix_ms(),
        ))
        .map_err(TrashError::Journal)?;

    Ok(TrashReport {
        root: guard.root().display().to_string(),
        journal_path: store.journal_path_display(),
        operation_id,
        original_path: relative_path.to_string(),
        trashed_path: trashed_relative_path,
        metadata_path: metadata_relative_path,
    })
}

fn journal_entry(
    operation_id: &str,
    status: JournalStatus,
    from: &str,
    to: &str,
    created_unix_ms: u128,
) -> JournalEntry {
    JournalEntry {
        operation_id: operation_id.to_string(),
        status,
        action: JournalAction::Trash,
        from: from.to_string(),
        to: to.to_string(),
        created_unix_ms,
    }
}

fn modified_unix_ms(metadata: &fs::Metadata) -> Option<u128> {
    metadata
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_millis())
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

impl fmt::Display for TrashError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TrashError::Guard(error) => write!(formatter, "{error}"),
            TrashError::Journal(error) => write!(formatter, "{error}"),
            TrashError::Metadata { path, message } => {
                write!(
                    formatter,
                    "cannot read metadata {}: {message}",
                    path.display()
                )
            }
            TrashError::NotAFile { path } => {
                write!(formatter, "trash target is not a file: {}", path.display())
            }
            TrashError::BlockedLinkOrReparse { path } => {
                write!(
                    formatter,
                    "refusing to trash symlink, junction, or reparse point: {}",
                    path.display()
                )
            }
            TrashError::CreateTrashDir { path, message } => {
                write!(
                    formatter,
                    "cannot create trash directory {}: {message}",
                    path.display()
                )
            }
            TrashError::WriteMetadata { path, message } => {
                write!(
                    formatter,
                    "cannot write trash metadata {}: {message}",
                    path.display()
                )
            }
            TrashError::DestinationExists { path } => {
                write!(
                    formatter,
                    "trash destination already exists: {}",
                    path.display()
                )
            }
            TrashError::Move { from, to, message } => {
                write!(
                    formatter,
                    "cannot move {} to trash {}: {message}",
                    from.display(),
                    to.display()
                )
            }
            TrashError::Serialize(message) => {
                write!(formatter, "cannot serialize trash metadata: {message}")
            }
        }
    }
}

impl Error for TrashError {}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use crate::journal::{read_operation_history, JournalAction, JournalStatus, TRASH_DIR};
    use crate::undo::undo_operation;

    use super::{trash_file, TrashError, TrashMetadata};

    #[test]
    fn trashes_file_with_metadata_and_journal() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("inbox")).expect("create inbox");
        fs::write(root.join("inbox").join("temp.log"), "noise").expect("write temp");

        let report = trash_file(&root, "inbox/temp.log").expect("trash file");

        assert!(!root.join("inbox").join("temp.log").exists());
        assert!(root.join(&report.trashed_path).exists());
        assert!(root.join(&report.metadata_path).exists());
        assert!(report.trashed_path.starts_with(TRASH_DIR));

        let metadata: TrashMetadata = serde_json::from_str(
            &fs::read_to_string(root.join(&report.metadata_path)).expect("read metadata"),
        )
        .expect("parse metadata");
        assert_eq!(metadata.original_path, "inbox/temp.log");
        assert_eq!(metadata.trashed_path, report.trashed_path);

        let history = read_operation_history(&root).expect("history");
        assert_eq!(history.operations.len(), 1);
        assert_eq!(history.operations[0].action, JournalAction::Trash);
        assert_eq!(history.operations[0].latest_status, JournalStatus::Executed);
        assert!(history.operations[0].can_undo);
    }

    #[test]
    fn undo_restores_trashed_file_to_original_path() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("inbox")).expect("create inbox");
        fs::write(root.join("inbox").join("temp.log"), "noise").expect("write temp");
        let report = trash_file(&root, "inbox/temp.log").expect("trash file");

        let undo = undo_operation(&root, &report.operation_id).expect("undo trash");

        assert_eq!(undo.undone_count, 1);
        assert!(root.join("inbox").join("temp.log").exists());
        assert!(!root.join(&report.trashed_path).exists());

        let history = read_operation_history(&root).expect("history");
        assert_eq!(history.operations[0].latest_status, JournalStatus::Undone);
        assert!(!history.operations[0].can_undo);
    }

    #[test]
    fn refuses_to_trash_direct_symlink() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(&root).expect("create root");
        fs::write(root.join("real.txt"), "real").expect("write real");

        #[cfg(unix)]
        std::os::unix::fs::symlink(root.join("real.txt"), root.join("link.txt"))
            .expect("create symlink");

        #[cfg(windows)]
        if let Err(error) =
            std::os::windows::fs::symlink_file(root.join("real.txt"), root.join("link.txt"))
        {
            eprintln!("skipping symlink trash test; cannot create file symlink: {error}");
            return;
        }

        let error = trash_file(&root, "link.txt").expect_err("reject symlink");

        assert!(matches!(error, TrashError::BlockedLinkOrReparse { .. }));
        assert!(root.join("real.txt").exists());
    }
}
