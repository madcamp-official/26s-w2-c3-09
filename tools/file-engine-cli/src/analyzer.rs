use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::Serialize;

use crate::fs_safety::is_link_or_reparse_point;
use crate::path_guard::{PathGuard, PathGuardError};

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct AnalyzeReport {
    pub root: String,
    pub files: Vec<FileEntry>,
    pub skipped_entries: Vec<SkippedEntry>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct FileEntry {
    pub path: String,
    pub size_bytes: u64,
    pub modified_unix_ms: Option<u128>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct SkippedEntry {
    pub path: String,
    pub reason: String,
}

#[derive(Debug)]
pub enum AnalyzeError {
    Guard(PathGuardError),
    ReadDir { path: PathBuf, message: String },
    Metadata { path: PathBuf, message: String },
    EscapedEntry { path: PathBuf },
    Serialize(String),
}

pub fn analyze_root(root: impl AsRef<Path>) -> Result<AnalyzeReport, AnalyzeError> {
    let guard = PathGuard::new(root).map_err(AnalyzeError::Guard)?;
    let mut files = Vec::new();
    let mut skipped_entries = Vec::new();

    collect_files(guard.root(), guard.root(), &mut files, &mut skipped_entries)?;
    files.sort_by(|left, right| left.path.cmp(&right.path));
    skipped_entries.sort_by(|left, right| left.path.cmp(&right.path));

    Ok(AnalyzeReport {
        root: guard.root().display().to_string(),
        files,
        skipped_entries,
    })
}

fn collect_files(
    root: &Path,
    current_dir: &Path,
    files: &mut Vec<FileEntry>,
    skipped_entries: &mut Vec<SkippedEntry>,
) -> Result<(), AnalyzeError> {
    let entries = match fs::read_dir(current_dir) {
        Ok(entries) => entries,
        Err(error) if current_dir == root => {
            return Err(AnalyzeError::ReadDir {
                path: current_dir.to_path_buf(),
                message: error.to_string(),
            });
        }
        Err(error) => {
            skipped_entries.push(skipped_entry(root, current_dir, error.to_string()));
            return Ok(());
        }
    };

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                skipped_entries.push(skipped_entry(root, current_dir, error.to_string()));
                continue;
            }
        };
        let path = entry.path();
        if is_mousekeeper_internal_dir(&path) {
            continue;
        }

        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) => {
                skipped_entries.push(skipped_entry(root, &path, error.to_string()));
                continue;
            }
        };
        let file_type = metadata.file_type();

        if is_link_or_reparse_point(&metadata, file_type) {
            skipped_entries.push(skipped_entry(
                root,
                &path,
                "skipped symlink, junction, or reparse point".to_string(),
            ));
            continue;
        }

        if metadata.is_dir() {
            collect_files(root, &path, files, skipped_entries)?;
            continue;
        }

        if metadata.is_file() {
            match relative_path(root, &path) {
                Ok(relative_path) => files.push(FileEntry {
                    path: relative_path,
                    size_bytes: metadata.len(),
                    modified_unix_ms: modified_unix_ms(&metadata),
                    file_id: crate::file_identity::file_id_for_path(&path),
                }),
                Err(error) => skipped_entries.push(skipped_entry(root, &path, error.to_string())),
            }
        }
    }

    Ok(())
}

fn skipped_entry(root: &Path, path: &Path, reason: String) -> SkippedEntry {
    SkippedEntry {
        path: relative_path_lossy(root, path),
        reason,
    }
}

fn relative_path(root: &Path, path: &Path) -> Result<String, AnalyzeError> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| AnalyzeError::EscapedEntry {
            path: path.to_path_buf(),
        })?;

    Ok(relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/"))
}

fn relative_path_lossy(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
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

impl fmt::Display for AnalyzeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AnalyzeError::Guard(error) => write!(formatter, "{error}"),
            AnalyzeError::ReadDir { path, message } => {
                write!(
                    formatter,
                    "cannot read directory {}: {message}",
                    path.display()
                )
            }
            AnalyzeError::Metadata { path, message } => {
                write!(
                    formatter,
                    "cannot read metadata {}: {message}",
                    path.display()
                )
            }
            AnalyzeError::EscapedEntry { path } => {
                write!(formatter, "entry escaped managed root: {}", path.display())
            }
            AnalyzeError::Serialize(message) => {
                write!(formatter, "cannot serialize report: {message}")
            }
        }
    }
}

impl Error for AnalyzeError {}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::analyze_root;

    #[test]
    fn lists_files_relative_to_root_in_stable_order() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("b")).expect("create b");
        fs::create_dir_all(root.join("a")).expect("create a");
        fs::write(root.join("b").join("two.txt"), "22").expect("write two");
        fs::write(root.join("a").join("one.txt"), "1").expect("write one");

        let report = analyze_root(&root).expect("analyze");

        let paths = report
            .files
            .iter()
            .map(|file| file.path.as_str())
            .collect::<Vec<_>>();

        assert_eq!(paths, vec!["a/one.txt", "b/two.txt"]);
        assert_eq!(report.files[0].size_bytes, 1);
        assert_eq!(report.files[1].size_bytes, 2);
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
                eprintln!("skipping symlink safety test; symlink privilege is unavailable");
                return;
            }
            panic!("create symlink: {error}");
        }

        let report = analyze_root(&root).expect("analyze");

        assert_eq!(report.files.len(), 1);
        assert_eq!(report.files[0].path, "real.txt");
        assert_eq!(report.skipped_entries.len(), 1);
    }

    #[test]
    fn ignores_mousekeeper_trash_dir() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join(crate::journal::TRASH_DIR)).expect("create trash dir");
        fs::write(root.join("real.txt"), "real").expect("write real");
        fs::write(
            root.join(crate::journal::TRASH_DIR).join("hidden.txt"),
            "hidden",
        )
        .expect("write trash file");

        let report = analyze_root(&root).expect("analyze");

        let paths = report
            .files
            .iter()
            .map(|file| file.path.as_str())
            .collect::<Vec<_>>();
        assert_eq!(paths, vec!["real.txt"]);
    }

    #[test]
    fn analyzes_downloads_like_names() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("Downloads");
        fs::create_dir_all(&root).expect("create downloads");
        fs::write(root.join("보고서 (최종).pdf"), "pdf").expect("write korean pdf");
        fs::write(root.join("installer.tmp"), "tmp").expect("write temp file");
        fs::write(root.join("shortcut.lnk"), "shortcut").expect("write shortcut file");

        let report = analyze_root(&root).expect("analyze downloads-like root");
        let paths = report
            .files
            .iter()
            .map(|file| file.path.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            paths,
            vec!["installer.tmp", "shortcut.lnk", "보고서 (최종).pdf"]
        );
        assert!(report.skipped_entries.is_empty());
    }

    #[cfg(windows)]
    #[test]
    fn ignores_windows_reparse_directory_escape() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        let outside = temp.path().join("outside");
        fs::create_dir_all(&root).expect("create root");
        fs::create_dir_all(&outside).expect("create outside");
        fs::write(root.join("inside.txt"), "inside").expect("write inside");
        fs::write(outside.join("secret.txt"), "secret").expect("write outside");

        let link = root.join("outside-link");
        if let Err(error) = std::os::windows::fs::symlink_dir(&outside, &link) {
            eprintln!("skipping reparse safety test; cannot create directory symlink: {error}");
            return;
        }

        let report = analyze_root(&root).expect("analyze");

        let paths = report
            .files
            .iter()
            .map(|file| file.path.as_str())
            .collect::<Vec<_>>();
        assert_eq!(paths, vec!["inside.txt"]);
        assert_eq!(report.skipped_entries.len(), 1);
    }
}
