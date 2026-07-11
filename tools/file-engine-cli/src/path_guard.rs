use std::error::Error;
use std::fmt;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone)]
pub struct PathGuard {
    root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathGuardError {
    RootNotDirectory(PathBuf),
    AbsoluteInput(PathBuf),
    ParentTraversal(PathBuf),
    MissingPath(PathBuf),
    EscapesRoot { root: PathBuf, resolved: PathBuf },
    Io(String),
}

impl PathGuard {
    pub fn new(root: impl AsRef<Path>) -> Result<Self, PathGuardError> {
        let root = canonicalize_existing(root.as_ref())?;

        if !root.is_dir() {
            return Err(PathGuardError::RootNotDirectory(root));
        }

        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn resolve_existing(
        &self,
        relative_path: impl AsRef<Path>,
    ) -> Result<PathBuf, PathGuardError> {
        let relative_path = relative_path.as_ref();
        reject_unsafe_components(relative_path)?;

        // Canonicalization resolves symlinks/junctions before the root-boundary check.
        let candidate = self.root.join(relative_path);
        let resolved = canonicalize_existing(&candidate)
            .map_err(|_| PathGuardError::MissingPath(candidate.clone()))?;

        if !resolved.starts_with(&self.root) {
            return Err(PathGuardError::EscapesRoot {
                root: self.root.clone(),
                resolved,
            });
        }

        Ok(resolved)
    }

    pub fn resolve_for_create(
        &self,
        relative_path: impl AsRef<Path>,
    ) -> Result<PathBuf, PathGuardError> {
        let relative_path = relative_path.as_ref();
        reject_unsafe_components(relative_path)?;

        let candidate = self.root.join(relative_path);
        let parent = candidate
            .parent()
            .ok_or_else(|| PathGuardError::MissingPath(candidate.clone()))?;
        let resolved_parent = canonicalize_existing(parent)
            .map_err(|_| PathGuardError::MissingPath(parent.to_path_buf()))?;

        if !resolved_parent.starts_with(&self.root) {
            return Err(PathGuardError::EscapesRoot {
                root: self.root.clone(),
                resolved: resolved_parent,
            });
        }

        Ok(resolved_parent.join(
            candidate
                .file_name()
                .ok_or_else(|| PathGuardError::MissingPath(candidate.clone()))?,
        ))
    }
}

fn reject_unsafe_components(path: &Path) -> Result<(), PathGuardError> {
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => {
                return Err(PathGuardError::AbsoluteInput(path.to_path_buf()));
            }
            Component::ParentDir => {
                return Err(PathGuardError::ParentTraversal(path.to_path_buf()));
            }
            Component::CurDir | Component::Normal(_) => {}
        }
    }

    Ok(())
}

fn canonicalize_existing(path: &Path) -> Result<PathBuf, PathGuardError> {
    path.canonicalize()
        .map_err(|error| PathGuardError::Io(error.to_string()))
}

impl fmt::Display for PathGuardError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PathGuardError::RootNotDirectory(path) => {
                write!(
                    formatter,
                    "managed root is not a directory: {}",
                    path.display()
                )
            }
            PathGuardError::AbsoluteInput(path) => {
                write!(
                    formatter,
                    "path must be relative to the managed root: {}",
                    path.display()
                )
            }
            PathGuardError::ParentTraversal(path) => {
                write!(
                    formatter,
                    "path cannot contain parent traversal: {}",
                    path.display()
                )
            }
            PathGuardError::MissingPath(path) => {
                write!(
                    formatter,
                    "path does not exist under managed root: {}",
                    path.display()
                )
            }
            PathGuardError::EscapesRoot { root, resolved } => {
                write!(
                    formatter,
                    "resolved path escapes managed root: root={}, resolved={}",
                    root.display(),
                    resolved.display()
                )
            }
            PathGuardError::Io(message) => write!(formatter, "{message}"),
        }
    }
}

impl Error for PathGuardError {}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use tempfile::tempdir;

    use super::{PathGuard, PathGuardError};

    #[test]
    fn accepts_existing_file_inside_root() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        let nested = root.join("docs");
        fs::create_dir_all(&nested).expect("create nested dir");
        fs::write(nested.join("readme.md"), "# hello").expect("write fixture");

        let guard = PathGuard::new(&root).expect("guard");
        let resolved = guard
            .resolve_existing(Path::new("docs").join("readme.md"))
            .expect("resolve");

        assert!(resolved.starts_with(guard.root()));
        assert_eq!(
            resolved.file_name().and_then(|name| name.to_str()),
            Some("readme.md")
        );
    }

    #[test]
    fn rejects_parent_traversal_before_touching_filesystem() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(&root).expect("create root");

        let guard = PathGuard::new(&root).expect("guard");
        let error = guard
            .resolve_existing("../outside.txt")
            .expect_err("reject traversal");

        assert!(matches!(error, PathGuardError::ParentTraversal(_)));
    }

    #[test]
    fn rejects_absolute_input() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(&root).expect("create root");

        let guard = PathGuard::new(&root).expect("guard");
        let error = guard
            .resolve_existing(root.join("file.txt"))
            .expect_err("reject absolute");

        assert!(matches!(error, PathGuardError::AbsoluteInput(_)));
    }

    #[test]
    fn rejects_missing_path_inside_root() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(&root).expect("create root");

        let guard = PathGuard::new(&root).expect("guard");
        let error = guard
            .resolve_existing("missing.txt")
            .expect_err("reject missing path");

        assert!(matches!(error, PathGuardError::MissingPath(_)));
    }

    #[test]
    fn resolves_create_target_when_parent_exists() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("documents")).expect("create parent");

        let guard = PathGuard::new(&root).expect("guard");
        let target = guard
            .resolve_for_create("documents/readme.md")
            .expect("resolve target");

        assert!(target.starts_with(guard.root()));
        assert_eq!(
            target.file_name().and_then(|name| name.to_str()),
            Some("readme.md")
        );
    }

    #[test]
    fn rejects_create_target_with_missing_parent() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(&root).expect("create root");

        let guard = PathGuard::new(&root).expect("guard");
        let error = guard
            .resolve_for_create("missing/readme.md")
            .expect_err("reject missing parent");

        assert!(matches!(error, PathGuardError::MissingPath(_)));
    }
}
