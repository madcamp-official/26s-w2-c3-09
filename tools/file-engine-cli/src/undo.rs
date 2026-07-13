use std::collections::HashSet;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::journal::{JournalAction, JournalEntry, JournalError, JournalStatus, JournalStore};
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
    OperationNotUndoable {
        operation_id: String,
    },
    Serialize(String),
}

pub fn undo_root(root: impl AsRef<Path>) -> Result<UndoReport, UndoError> {
    undo_with_filter(root, None)
}

pub fn undo_operation(
    root: impl AsRef<Path>,
    operation_id: impl AsRef<str>,
) -> Result<UndoReport, UndoError> {
    undo_with_filter(root, Some(operation_id.as_ref()))
}

fn undo_with_filter(
    root: impl AsRef<Path>,
    only_operation_id: Option<&str>,
) -> Result<UndoReport, UndoError> {
    let guard = PathGuard::new(root).map_err(UndoError::Guard)?;
    let store = JournalStore::open(guard.root()).map_err(UndoError::Journal)?;
    let entries = store.read_all().map_err(UndoError::Journal)?;
    let undone_operation_ids = entries
        .iter()
        .filter(|entry| entry.status == JournalStatus::Undone)
        .map(|entry| entry.operation_id.clone())
        .collect::<HashSet<_>>();

    let mut undone_count = 0;
    let mut skipped_count = 0;
    let mut results = Vec::new();

    for entry in entries.iter().rev() {
        if only_operation_id.is_some_and(|operation_id| operation_id != entry.operation_id) {
            continue;
        }

        if entry.status != JournalStatus::Executed
            || !matches!(
                entry.action,
                JournalAction::Move
                    | JournalAction::Trash
                    | JournalAction::CreateDir
                    | JournalAction::CreateFile
                    | JournalAction::ReadmeWrite
            )
            || undone_operation_ids.contains(&entry.operation_id)
        {
            continue;
        }

        if entry.action == JournalAction::CreateDir {
            match undo_create_dir(&guard, &store, entry)? {
                Ok(result) => {
                    undone_count += 1;
                    results.push(result);
                }
                Err(reason) => {
                    skipped_count += 1;
                    results.push(skipped_result(entry, Some(reason)));
                }
            }
            continue;
        }

        if entry.action == JournalAction::CreateFile {
            match undo_create_file(&guard, &store, entry)? {
                Ok(result) => {
                    undone_count += 1;
                    results.push(result);
                }
                Err(reason) => {
                    skipped_count += 1;
                    results.push(skipped_result(entry, Some(reason)));
                }
            }
            continue;
        }

        if entry.action == JournalAction::ReadmeWrite {
            match undo_readme_write(&guard, &store, entry)? {
                Ok(result) => {
                    undone_count += 1;
                    results.push(result);
                }
                Err(reason) => {
                    skipped_count += 1;
                    results.push(skipped_result(entry, Some(reason)));
                }
            }
            continue;
        }

        let current_path = match guard.resolve_existing(&entry.to) {
            Ok(path) => path,
            Err(error) => {
                if guard.resolve_existing(&entry.from).is_ok() {
                    store
                        .append(&undo_entry(entry, JournalStatus::Undone))
                        .map_err(UndoError::Journal)?;
                    undone_count += 1;
                    results.push(UndoResult {
                        from: entry.to.clone(),
                        to: entry.from.clone(),
                        status: UndoStatus::Undone,
                        reason: Some(
                            "destination missing and original path exists; recorded recovered undo"
                                .to_string(),
                        ),
                    });
                    continue;
                }

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

        store
            .append(&undo_entry(entry, JournalStatus::UndoPlanned))
            .map_err(UndoError::Journal)?;
        fs::rename(&current_path, &original_path).map_err(|error| UndoError::Move {
            from: current_path,
            to: original_path,
            message: error.to_string(),
        })?;
        store
            .append(&undo_entry(entry, JournalStatus::Undone))
            .map_err(UndoError::Journal)?;

        undone_count += 1;
        results.push(UndoResult {
            from: entry.to.clone(),
            to: entry.from.clone(),
            status: UndoStatus::Undone,
            reason: None,
        });
    }

    if let Some(operation_id) = only_operation_id {
        if results.is_empty() {
            return Err(UndoError::OperationNotUndoable {
                operation_id: operation_id.to_string(),
            });
        }
    }

    Ok(UndoReport {
        root: guard.root().display().to_string(),
        journal_path: store.journal_path_display(),
        undone_count,
        skipped_count,
        results,
    })
}

fn undo_create_file(
    guard: &PathGuard,
    store: &JournalStore,
    entry: &JournalEntry,
) -> Result<Result<UndoResult, String>, UndoError> {
    let target = guard.root().join(&entry.to);
    if !target.exists() {
        store
            .append(&undo_entry(entry, JournalStatus::Undone))
            .map_err(UndoError::Journal)?;
        return Ok(Ok(UndoResult {
            from: entry.to.clone(),
            to: entry.from.clone(),
            status: UndoStatus::Undone,
            reason: Some("created file is already missing; recorded recovered undo".to_string()),
        }));
    }
    if !target.is_file() {
        return Ok(Err(
            "created path is no longer a file; refusing undo".to_string()
        ));
    }
    if fs::metadata(&target)
        .map(|metadata| metadata.len())
        .unwrap_or(1)
        != 0
    {
        return Ok(Err(
            "created file is no longer empty; refusing to delete user data".to_string(),
        ));
    }

    store
        .append(&undo_entry(entry, JournalStatus::UndoPlanned))
        .map_err(UndoError::Journal)?;
    fs::remove_file(&target).map_err(|error| UndoError::Move {
        from: target.clone(),
        to: target.clone(),
        message: error.to_string(),
    })?;
    store
        .append(&undo_entry(entry, JournalStatus::Undone))
        .map_err(UndoError::Journal)?;

    Ok(Ok(UndoResult {
        from: entry.to.clone(),
        to: entry.from.clone(),
        status: UndoStatus::Undone,
        reason: None,
    }))
}

fn undo_create_dir(
    guard: &PathGuard,
    store: &JournalStore,
    entry: &JournalEntry,
) -> Result<Result<UndoResult, String>, UndoError> {
    let target = guard.root().join(&entry.to);
    if !target.exists() {
        store
            .append(&undo_entry(entry, JournalStatus::Undone))
            .map_err(UndoError::Journal)?;
        return Ok(Ok(UndoResult {
            from: entry.to.clone(),
            to: entry.from.clone(),
            status: UndoStatus::Undone,
            reason: Some(
                "created directory is already missing; recorded recovered undo".to_string(),
            ),
        }));
    }
    if !target.is_dir() {
        return Ok(Err(
            "created path is no longer a directory; refusing undo".to_string()
        ));
    }
    if fs::read_dir(&target)
        .map(|mut entries| entries.next().is_some())
        .unwrap_or(true)
    {
        return Ok(Err(
            "created directory is not empty; refusing to delete user data".to_string(),
        ));
    }

    store
        .append(&undo_entry(entry, JournalStatus::UndoPlanned))
        .map_err(UndoError::Journal)?;
    fs::remove_dir(&target).map_err(|error| UndoError::Move {
        from: target.clone(),
        to: target.clone(),
        message: error.to_string(),
    })?;
    store
        .append(&undo_entry(entry, JournalStatus::Undone))
        .map_err(UndoError::Journal)?;

    Ok(Ok(UndoResult {
        from: entry.to.clone(),
        to: entry.from.clone(),
        status: UndoStatus::Undone,
        reason: None,
    }))
}

fn undo_readme_write(
    guard: &PathGuard,
    store: &JournalStore,
    entry: &JournalEntry,
) -> Result<Result<UndoResult, String>, UndoError> {
    if entry.to != "README.md" {
        return Ok(Err(
            "README write journal target is not README.md; refusing undo".to_string(),
        ));
    }

    let target = guard.root().join("README.md");
    let backup = guard.root().join(&entry.from);
    let absent = backup.with_extension("absent");

    store
        .append(&undo_entry(entry, JournalStatus::UndoPlanned))
        .map_err(UndoError::Journal)?;

    if backup.exists() {
        fs::copy(&backup, &target).map_err(|error| UndoError::Move {
            from: backup.clone(),
            to: target.clone(),
            message: error.to_string(),
        })?;
    } else if absent.exists() {
        if target.exists() {
            fs::remove_file(&target).map_err(|error| UndoError::Move {
                from: target.clone(),
                to: absent.clone(),
                message: error.to_string(),
            })?;
        }
    } else {
        return Ok(Err(
            "README write backup is missing; refusing undo".to_string()
        ));
    }

    store
        .append(&undo_entry(entry, JournalStatus::Undone))
        .map_err(UndoError::Journal)?;

    Ok(Ok(UndoResult {
        from: entry.to.clone(),
        to: entry.from.clone(),
        status: UndoStatus::Undone,
        reason: None,
    }))
}

fn undo_entry(entry: &JournalEntry, status: JournalStatus) -> JournalEntry {
    JournalEntry {
        operation_id: entry.operation_id.clone(),
        status,
        action: entry.action.clone(),
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
            UndoError::Journal(error) => write!(formatter, "{error}"),
            UndoError::CreateParentDir { path, message } => {
                write!(
                    formatter,
                    "cannot create original parent {}: {message}",
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
            UndoError::OperationNotUndoable { operation_id } => {
                write!(formatter, "operation is not undoable: {operation_id}")
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
    use crate::proposal::{proposal_id, Proposal, ProposalAction, ProposalReport, ProposalStatus};

    use super::undo_root;

    #[test]
    fn restores_executed_move() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("inbox")).expect("create inbox");
        fs::write(root.join("inbox").join("note.md"), "# note").expect("write note");
        execute_root(&root).expect("execute");

        let report = undo_root(&root).expect("undo");
        let history = crate::journal::read_operation_history(&root).expect("history");

        assert_eq!(report.undone_count, 1);
        assert_eq!(report.skipped_count, 0);
        assert!(root.join("inbox").join("note.md").exists());
        assert!(!root.join("documents").join("note.md").exists());
        // The operation ends in the undone state and is no longer undoable.
        assert_eq!(
            history.operations[0].latest_status,
            crate::journal::JournalStatus::Undone
        );
        assert!(!history.operations[0].can_undo);
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

    #[test]
    fn records_recovered_undo_when_file_was_already_restored() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("inbox")).expect("create inbox");
        fs::write(root.join("inbox").join("note.md"), "# note").expect("write note");
        execute_root(&root).expect("execute");
        fs::rename(
            root.join("documents").join("note.md"),
            root.join("inbox").join("note.md"),
        )
        .expect("simulate restored file before undone journal");

        let report = undo_root(&root).expect("undo");
        let history = crate::journal::read_operation_history(&root).expect("history");

        assert_eq!(report.undone_count, 1);
        assert_eq!(report.skipped_count, 0);
        assert_eq!(
            history.operations[0].latest_status,
            crate::journal::JournalStatus::Undone
        );
    }

    #[test]
    fn refuses_to_undo_created_directory_when_it_is_not_empty() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("archive")).expect("archive parent");
        let action = ProposalAction::CreateDir;
        let proposal = ProposalReport {
            root: root
                .canonicalize()
                .expect("canonical")
                .display()
                .to_string(),
            proposals: vec![Proposal {
                proposal_id: proposal_id(&action, "", "archive/reports"),
                action,
                from: String::new(),
                to: "archive/reports".to_string(),
                content: None,
                source_size_bytes: 0,
                source_modified_unix_ms: None,
                reason: "USER_REQUESTED_CREATE_DIR".to_string(),
                status: ProposalStatus::Ready,
            }],
        };
        crate::execute::execute_proposals(&root, proposal).expect("execute create dir");
        fs::write(root.join("archive/reports/keep.txt"), "user data").expect("user data");

        let report = undo_root(&root).expect("undo create dir");

        assert_eq!(report.undone_count, 0);
        assert_eq!(report.skipped_count, 1);
        assert!(root.join("archive/reports/keep.txt").exists());
    }

    #[test]
    fn refuses_to_undo_created_file_when_it_is_not_empty() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("notes")).expect("notes parent");
        let action = ProposalAction::CreateFile;
        let proposal = ProposalReport {
            root: root
                .canonicalize()
                .expect("canonical")
                .display()
                .to_string(),
            proposals: vec![Proposal {
                proposal_id: proposal_id(&action, "", "notes/todo.txt"),
                action,
                from: String::new(),
                to: "notes/todo.txt".to_string(),
                content: None,
                source_size_bytes: 0,
                source_modified_unix_ms: None,
                reason: "USER_REQUESTED_CREATE_FILE".to_string(),
                status: ProposalStatus::Ready,
            }],
        };
        crate::execute::execute_proposals(&root, proposal).expect("execute create file");
        fs::write(root.join("notes/todo.txt"), "user data").expect("user data");

        let report = undo_root(&root).expect("undo create file");

        assert_eq!(report.undone_count, 0);
        assert_eq!(report.skipped_count, 1);
        assert_eq!(
            fs::read_to_string(root.join("notes/todo.txt")).expect("file"),
            "user data"
        );
    }
}
