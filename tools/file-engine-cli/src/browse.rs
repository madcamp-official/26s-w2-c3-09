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
    pub skipped_entries: Vec<SkippedEntry>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct BrowseEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size_bytes: Option<u64>,
    pub modified_unix_ms: Option<u128>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct SkippedEntry {
    pub path: String,
    pub reason: String,
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
    let mut skipped_entries = Vec::new();
    let read_dir = fs::read_dir(&target_dir).map_err(|error| BrowseError::ReadDir {
        path: target_dir.clone(),
        message: error.to_string(),
    })?;

    for entry in read_dir {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                skipped_entries.push(skipped_entry(guard.root(), &target_dir, error.to_string()));
                continue;
            }
        };
        let entry_path = entry.path();
        if is_mousekeeper_internal_dir(&entry_path) {
            continue;
        }

        let metadata = match fs::symlink_metadata(&entry_path) {
            Ok(metadata) => metadata,
            Err(error) => {
                skipped_entries.push(skipped_entry(guard.root(), &entry_path, error.to_string()));
                continue;
            }
        };
        let file_type = metadata.file_type();

        if is_link_or_reparse_point(&metadata, file_type) {
            skipped_entries.push(skipped_entry(
                guard.root(),
                &entry_path,
                "skipped symlink, junction, or reparse point".to_string(),
            ));
            continue;
        }

        let is_dir = metadata.is_dir();
        let name = entry_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string();

        match relative_entry_path(guard.root(), &entry_path) {
            Ok(path) => entries.push(BrowseEntry {
                name,
                path,
                is_dir,
                size_bytes: if is_dir { None } else { Some(metadata.len()) },
                modified_unix_ms: modified_unix_ms(&metadata),
            }),
            Err(error) => {
                skipped_entries.push(skipped_entry(guard.root(), &entry_path, error.to_string()))
            }
        }
    }

    entries.sort_by(|left, right| match (left.is_dir, right.is_dir) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        _ => left.name.cmp(&right.name),
    });
    skipped_entries.sort_by(|left, right| left.path.cmp(&right.path));

    Ok(BrowseReport {
        root: guard.root().display().to_string(),
        path: relative_path,
        entries,
        skipped_entries,
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

fn skipped_entry(root: &Path, path: &Path, reason: String) -> SkippedEntry {
    SkippedEntry {
        path: path
            .strip_prefix(root)
            .unwrap_or(path)
            .components()
            .map(|component| component.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/"),
        reason,
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

fn is_mousekeeper_internal_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == crate::journal::STATE_DIR || name == crate::journal::TRASH_DIR)
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
        assert!(report.skipped_entries.is_empty());
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
    fn ignores_mousekeeper_state_dir() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join(".mousekeeper")).expect("create state dir");
        fs::write(root.join("readme.txt"), "hi").expect("write readme");

        let report = browse_root(&root, None).expect("browse");

        assert_eq!(report.entries.len(), 1);
        assert_eq!(report.entries[0].name, "readme.txt");
    }

    #[test]
    fn ignores_mousekeeper_trash_dir() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join(crate::journal::TRASH_DIR)).expect("create trash dir");
        fs::write(root.join("readme.txt"), "hi").expect("write readme");

        let report = browse_root(&root, None).expect("browse");

        assert_eq!(report.entries.len(), 1);
        assert_eq!(report.entries[0].name, "readme.txt");
    }

    #[test]
    fn browses_downloads_like_names() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("Downloads");
        fs::create_dir_all(root.join("새 폴더")).expect("create korean folder");
        fs::write(root.join("보고서 (최종).pdf"), "pdf").expect("write korean pdf");
        fs::write(root.join("installer.tmp"), "tmp").expect("write temp file");

        let report = browse_root(&root, None).expect("browse downloads-like root");
        let names = report
            .entries
            .iter()
            .map(|entry| entry.name.as_str())
            .collect::<Vec<_>>();

        assert_eq!(names, vec!["새 폴더", "installer.tmp", "보고서 (최종).pdf"]);
        assert!(report.entries[0].is_dir);
        assert!(report.skipped_entries.is_empty());
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
        if let Err(error) =
            std::os::windows::fs::symlink_file(root.join("real.txt"), root.join("link.txt"))
        {
            // Windows without Developer Mode or elevation returns ERROR_PRIVILEGE_NOT_HELD.
            if error.raw_os_error() == Some(1314) {
                eprintln!("skipping symlink browse test; symlink privilege is unavailable");
                return;
            }
            panic!("create symlink: {error}");
        }

        let report = browse_root(&root, None).expect("browse");

        assert_eq!(report.entries.len(), 1);
        assert_eq!(report.entries[0].name, "real.txt");
        assert_eq!(report.skipped_entries.len(), 1);
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
        assert_eq!(report.skipped_entries.len(), 1);
    }
}
