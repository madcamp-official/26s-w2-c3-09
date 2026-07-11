use std::cmp::Ordering;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::Serialize;

use crate::fs_safety::is_link_or_reparse_point;
use crate::path_guard::{PathGuard, PathGuardError};

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct BrowseReport {
    pub root: String,
    pub path: String,
    pub entries: Vec<BrowseEntry>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct BrowseEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size_bytes: Option<u64>,
    pub modified_unix_ms: Option<u128>,
}

#[derive(Debug)]
pub enum BrowseError {
    Guard(PathGuardError),
    NotADirectory { path: PathBuf },
    ReadDir { path: PathBuf, message: String },
    Metadata { path: PathBuf, message: String },
    EscapedEntry { path: PathBuf },
    Serialize(String),
}

/// Lists one directory level under a managed root. Unlike `analyze_root`, this does not
/// recurse, so it stays cheap on large real-world folders and matches how a file browser
/// UI navigates: fetch the current folder, then fetch again when the user opens a child.
pub fn browse_root(
    root: impl AsRef<Path>,
    relative_path: Option<&str>,
) -> Result<BrowseReport, BrowseError> {
    let guard = PathGuard::new(root).map_err(BrowseError::Guard)?;
    let relative_path = normalize_relative(relative_path.unwrap_or(""));

    let target_dir = if relative_path.is_empty() {
        guard.root().to_path_buf()
    } else {
        let resolved = guard
            .resolve_existing(&relative_path)
            .map_err(BrowseError::Guard)?;
        if !resolved.is_dir() {
            return Err(BrowseError::NotADirectory { path: resolved });
        }
        resolved
    };

    let mut entries = Vec::new();
    let read_dir = fs::read_dir(&target_dir).map_err(|error| BrowseError::ReadDir {
        path: target_dir.clone(),
        message: error.to_string(),
    })?;

    for entry in read_dir {
        let entry = entry.map_err(|error| BrowseError::ReadDir {
            path: target_dir.clone(),
            message: error.to_string(),
        })?;
        let entry_path = entry.path();
        if is_housemouse_state_dir(&entry_path) {
            continue;
        }

        let metadata =
            fs::symlink_metadata(&entry_path).map_err(|error| BrowseError::Metadata {
                path: entry_path.clone(),
                message: error.to_string(),
            })?;
        let file_type = metadata.file_type();

        if is_link_or_reparse_point(&metadata, file_type) {
            continue;
        }

        let is_dir = metadata.is_dir();
        let name = entry_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string();

        entries.push(BrowseEntry {
            name,
            path: relative_entry_path(guard.root(), &entry_path)?,
            is_dir,
            size_bytes: if is_dir { None } else { Some(metadata.len()) },
            modified_unix_ms: modified_unix_ms(&metadata),
        });
    }

    entries.sort_by(|left, right| match (left.is_dir, right.is_dir) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        _ => left.name.cmp(&right.name),
    });

    Ok(BrowseReport {
        root: guard.root().display().to_string(),
        path: relative_path,
        entries,
    })
}

fn normalize_relative(path: &str) -> String {
    path.split(['/', '\\'])
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("/")
}

fn relative_entry_path(root: &Path, path: &Path) -> Result<String, BrowseError> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| BrowseError::EscapedEntry {
            path: path.to_path_buf(),
        })?;

    Ok(relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/"))
}

fn modified_unix_ms(metadata: &fs::Metadata) -> Option<u128> {
    metadata
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_millis())
}

fn is_housemouse_state_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == crate::journal::STATE_DIR)
}

impl fmt::Display for BrowseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BrowseError::Guard(error) => write!(formatter, "{error}"),
            BrowseError::NotADirectory { path } => {
                write!(formatter, "path is not a directory: {}", path.display())
            }
            BrowseError::ReadDir { path, message } => {
                write!(
                    formatter,
                    "cannot read directory {}: {message}",
                    path.display()
                )
            }
            BrowseError::Metadata { path, message } => {
                write!(
                    formatter,
                    "cannot read metadata {}: {message}",
                    path.display()
                )
            }
            BrowseError::EscapedEntry { path } => {
                write!(formatter, "entry escaped managed root: {}", path.display())
            }
            BrowseError::Serialize(message) => {
                write!(formatter, "cannot serialize report: {message}")
            }
        }
    }
}

impl Error for BrowseError {}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::{browse_root, BrowseError};

    #[test]
    fn lists_root_level_entries_with_directories_before_files() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("inbox")).expect("create inbox");
        fs::create_dir_all(root.join("documents")).expect("create documents");
        fs::write(root.join("readme.txt"), "hi").expect("write readme");

        let report = browse_root(&root, None).expect("browse");

        let names = report
            .entries
            .iter()
            .map(|entry| entry.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["documents", "inbox", "readme.txt"]);
        assert!(report.entries[0].is_dir);
        assert!(report.entries[1].is_dir);
        assert!(!report.entries[2].is_dir);
        assert_eq!(report.entries[2].size_bytes, Some(2));
        assert_eq!(report.path, "");
    }

    #[test]
    fn browses_into_subdirectory_relative_to_root() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("documents")).expect("create documents");
        fs::write(root.join("documents").join("note.md"), "# note").expect("write note");

        let report = browse_root(&root, Some("documents")).expect("browse");

        assert_eq!(report.path, "documents");
        assert_eq!(report.entries.len(), 1);
        assert_eq!(report.entries[0].name, "note.md");
        assert_eq!(report.entries[0].path, "documents/note.md");
        assert!(!report.entries[0].is_dir);
    }

    #[test]
    fn rejects_browsing_into_a_file() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(&root).expect("create root");
        fs::write(root.join("readme.txt"), "hi").expect("write readme");

        let error = browse_root(&root, Some("readme.txt")).expect_err("reject file target");

        assert!(matches!(error, BrowseError::NotADirectory { .. }));
    }

    #[test]
    fn rejects_path_that_escapes_root() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(&root).expect("create root");

        let error = browse_root(&root, Some("../outside")).expect_err("reject traversal");

        assert!(matches!(error, BrowseError::Guard(_)));
    }

    #[test]
    fn ignores_housemouse_state_dir() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join(".housemouse")).expect("create state dir");
        fs::write(root.join("readme.txt"), "hi").expect("write readme");

        let report = browse_root(&root, None).expect("browse");

        assert_eq!(report.entries.len(), 1);
        assert_eq!(report.entries[0].name, "readme.txt");
    }

    #[test]
    fn ignores_direct_symlinks() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(&root).expect("create root");
        fs::write(root.join("real.txt"), "real").expect("write real");

        #[cfg(unix)]
        std::os::unix::fs::symlink(root.join("real.txt"), root.join("link.txt"))
            .expect("create symlink");

        #[cfg(windows)]
        std::os::windows::fs::symlink_file(root.join("real.txt"), root.join("link.txt"))
            .expect("create symlink");

        let report = browse_root(&root, None).expect("browse");

        assert_eq!(report.entries.len(), 1);
        assert_eq!(report.entries[0].name, "real.txt");
    }

    #[cfg(windows)]
    #[test]
    fn ignores_windows_reparse_directory_entries() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        let outside = temp.path().join("outside");
        fs::create_dir_all(&root).expect("create root");
        fs::create_dir_all(&outside).expect("create outside");
        fs::write(root.join("real.txt"), "real").expect("write real");
        fs::write(outside.join("secret.txt"), "secret").expect("write outside");

        let link = root.join("outside-link");
        if let Err(error) = std::os::windows::fs::symlink_dir(&outside, &link) {
            eprintln!("skipping reparse browse test; cannot create directory symlink: {error}");
            return;
        }

        let report = browse_root(&root, None).expect("browse");

        assert_eq!(report.entries.len(), 1);
        assert_eq!(report.entries[0].name, "real.txt");
    }
}
