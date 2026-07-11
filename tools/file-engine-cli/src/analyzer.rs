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
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct FileEntry {
    pub path: String,
    pub size_bytes: u64,
    pub modified_unix_ms: Option<u128>,
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

    collect_files(guard.root(), guard.root(), &mut files)?;
    files.sort_by(|left, right| left.path.cmp(&right.path));

    Ok(AnalyzeReport {
        root: guard.root().display().to_string(),
        files,
    })
}

fn collect_files(
    root: &Path,
    current_dir: &Path,
    files: &mut Vec<FileEntry>,
) -> Result<(), AnalyzeError> {
    let entries = fs::read_dir(current_dir).map_err(|error| AnalyzeError::ReadDir {
        path: current_dir.to_path_buf(),
        message: error.to_string(),
    })?;

    for entry in entries {
        let entry = entry.map_err(|error| AnalyzeError::ReadDir {
            path: current_dir.to_path_buf(),
            message: error.to_string(),
        })?;
        let path = entry.path();
        if is_housemouse_state_dir(&path) {
            continue;
        }

        let metadata = fs::symlink_metadata(&path).map_err(|error| AnalyzeError::Metadata {
            path: path.clone(),
            message: error.to_string(),
        })?;
        let file_type = metadata.file_type();

        if is_link_or_reparse_point(&metadata, file_type) {
            continue;
        }

        if metadata.is_dir() {
            collect_files(root, &path, files)?;
            continue;
        }

        if metadata.is_file() {
            files.push(FileEntry {
                path: relative_path(root, &path)?,
                size_bytes: metadata.len(),
                modified_unix_ms: modified_unix_ms(&metadata),
            });
        }
    }

    Ok(())
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
        .is_some_and(|name| name == ".housemouse")
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

        let report = analyze_root(&root).expect("analyze");

        assert_eq!(report.files.len(), 1);
        assert_eq!(report.files[0].path, "real.txt");
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
    }
}
