use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};

use serde::Serialize;
use sqlx::{Row, SqlitePool};

use crate::analyzer::{analyze_root, AnalyzeError};
use crate::db::{block_on, open_root_db, DbError};
use crate::path_guard::{PathGuard, PathGuardError};

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct FileIndexReport {
    pub root: String,
    pub files: Vec<IndexedFile>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct IndexedFile {
    pub relative_path: String,
    pub size_bytes: u64,
    pub modified_unix_ms: Option<u128>,
    pub extension: Option<String>,
}

#[derive(Debug)]
pub enum FileIndexError {
    Guard(PathGuardError),
    Analyze(AnalyzeError),
    Db(DbError),
}

/// Rebuilds the SQLite `file_index` for a root from a fresh scan. Runs in one transaction so
/// the index never reflects a half-written state, and replaces the whole table rather than
/// diffing — the cheap, always-correct baseline that a watcher can later refine with
/// incremental `upsert_file` / `remove_file` calls.
pub fn reindex_root(root: impl AsRef<Path>) -> Result<FileIndexReport, FileIndexError> {
    let guard = PathGuard::new(root).map_err(FileIndexError::Guard)?;
    let analysis = analyze_root(guard.root()).map_err(FileIndexError::Analyze)?;
    let pool = open_root_db(guard.root()).map_err(FileIndexError::Db)?;

    block_on(async {
        let mut tx = pool.begin().await.map_err(query_error)?;
        sqlx::query("DELETE FROM file_index")
            .execute(&mut *tx)
            .await
            .map_err(query_error)?;

        for file in &analysis.files {
            sqlx::query(
                "INSERT INTO file_index (relative_path, size_bytes, modified_unix_ms, extension)
                 VALUES (?, ?, ?, ?)",
            )
            .bind(&file.path)
            .bind(file.size_bytes as i64)
            .bind(file.modified_unix_ms.map(|value| value as i64))
            .bind(extension_of(&file.path))
            .execute(&mut *tx)
            .await
            .map_err(query_error)?;
        }

        tx.commit().await.map_err(query_error)
    })
    .map_err(FileIndexError::Db)?;

    let files = analysis
        .files
        .into_iter()
        .map(|file| IndexedFile {
            relative_path: file.path.clone(),
            size_bytes: file.size_bytes,
            modified_unix_ms: file.modified_unix_ms,
            extension: extension_of(&file.path),
        })
        .collect();

    Ok(FileIndexReport {
        root: analysis.root,
        files,
    })
}

/// Inserts or updates a single file. This is the hook a watcher uses on a create/modify event
/// so a change touches one row instead of forcing a full rescan.
pub fn upsert_file(
    root: impl AsRef<Path>,
    relative_path: &str,
    size_bytes: u64,
    modified_unix_ms: Option<u128>,
) -> Result<(), FileIndexError> {
    let guard = PathGuard::new(root).map_err(FileIndexError::Guard)?;
    let pool = open_root_db(guard.root()).map_err(FileIndexError::Db)?;

    block_on(async {
        sqlx::query(
            "INSERT INTO file_index (relative_path, size_bytes, modified_unix_ms, extension)
             VALUES (?, ?, ?, ?)
             ON CONFLICT(relative_path) DO UPDATE SET
                 size_bytes = excluded.size_bytes,
                 modified_unix_ms = excluded.modified_unix_ms,
                 extension = excluded.extension",
        )
        .bind(relative_path)
        .bind(size_bytes as i64)
        .bind(modified_unix_ms.map(|value| value as i64))
        .bind(extension_of(relative_path))
        .execute(&pool)
        .await
        .map(|_| ())
        .map_err(query_error)
    })
    .map_err(FileIndexError::Db)
}

/// Removes a single file from the index, e.g. on a watcher delete event.
pub fn remove_file(root: impl AsRef<Path>, relative_path: &str) -> Result<(), FileIndexError> {
    let guard = PathGuard::new(root).map_err(FileIndexError::Guard)?;
    let pool = open_root_db(guard.root()).map_err(FileIndexError::Db)?;

    block_on(async {
        sqlx::query("DELETE FROM file_index WHERE relative_path = ?")
            .bind(relative_path)
            .execute(&pool)
            .await
            .map(|_| ())
            .map_err(query_error)
    })
    .map_err(FileIndexError::Db)
}

pub fn list_index(root: impl AsRef<Path>) -> Result<FileIndexReport, FileIndexError> {
    query_index(root, None)
}

/// Case-insensitive substring search over the indexed relative paths. This is the cheap,
/// always-available lookup that a full-text search (`file_index_fts`) will later refine.
pub fn search_index(
    root: impl AsRef<Path>,
    query: &str,
) -> Result<FileIndexReport, FileIndexError> {
    query_index(root, Some(query))
}

fn query_index(
    root: impl AsRef<Path>,
    name_query: Option<&str>,
) -> Result<FileIndexReport, FileIndexError> {
    let guard = PathGuard::new(root).map_err(FileIndexError::Guard)?;
    let pool = open_root_db(guard.root()).map_err(FileIndexError::Db)?;

    let files =
        block_on(async { fetch_indexed(&pool, name_query).await }).map_err(FileIndexError::Db)?;

    Ok(FileIndexReport {
        root: guard.root().display().to_string(),
        files,
    })
}

async fn fetch_indexed(
    pool: &SqlitePool,
    name_query: Option<&str>,
) -> Result<Vec<IndexedFile>, DbError> {
    let rows = match name_query {
        Some(query) => sqlx::query(
            "SELECT relative_path, size_bytes, modified_unix_ms, extension
             FROM file_index
             WHERE relative_path LIKE ? ESCAPE '\\'
             ORDER BY relative_path",
        )
        .bind(format!("%{}%", escape_like(query)))
        .fetch_all(pool)
        .await
        .map_err(query_error)?,
        None => sqlx::query(
            "SELECT relative_path, size_bytes, modified_unix_ms, extension
             FROM file_index
             ORDER BY relative_path",
        )
        .fetch_all(pool)
        .await
        .map_err(query_error)?,
    };

    let mut files = Vec::with_capacity(rows.len());
    for row in rows {
        let relative_path: String = row.try_get("relative_path").map_err(query_error)?;
        let size_bytes: i64 = row.try_get("size_bytes").map_err(query_error)?;
        let modified: Option<i64> = row.try_get("modified_unix_ms").map_err(query_error)?;
        let extension: Option<String> = row.try_get("extension").map_err(query_error)?;

        files.push(IndexedFile {
            relative_path,
            size_bytes: size_bytes as u64,
            modified_unix_ms: modified.map(|value| value as u128),
            extension,
        });
    }

    Ok(files)
}

/// Escapes the LIKE metacharacters so a user query of `a_b` matches a literal underscore
/// rather than "any character". Pairs with `ESCAPE '\'` in the query.
fn escape_like(query: &str) -> String {
    query
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

fn extension_of(path: &str) -> Option<String> {
    PathBuf::from(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
}

fn query_error(error: sqlx::Error) -> DbError {
    DbError::Query {
        message: error.to_string(),
    }
}

impl fmt::Display for FileIndexError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FileIndexError::Guard(error) => write!(formatter, "{error}"),
            FileIndexError::Analyze(error) => write!(formatter, "{error}"),
            FileIndexError::Db(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for FileIndexError {}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::{list_index, reindex_root, remove_file, search_index, upsert_file};

    fn make_root() -> (tempfile::TempDir, std::path::PathBuf) {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("inbox")).expect("create inbox");
        fs::write(root.join("inbox").join("note.md"), "# note").expect("write note");
        fs::write(root.join("inbox").join("photo.png"), "png").expect("write photo");
        (temp, root)
    }

    #[test]
    fn reindex_populates_the_index_from_a_scan() {
        let (_temp, root) = make_root();

        let report = reindex_root(&root).expect("reindex");

        let paths: Vec<&str> = report
            .files
            .iter()
            .map(|file| file.relative_path.as_str())
            .collect();
        assert_eq!(paths, vec!["inbox/note.md", "inbox/photo.png"]);
        assert_eq!(report.files[0].extension.as_deref(), Some("md"));
    }

    #[test]
    fn reindex_is_idempotent_and_reflects_removals() {
        let (_temp, root) = make_root();
        reindex_root(&root).expect("first reindex");

        fs::remove_file(root.join("inbox").join("photo.png")).expect("delete photo");
        let report = reindex_root(&root).expect("second reindex");

        let paths: Vec<&str> = report
            .files
            .iter()
            .map(|file| file.relative_path.as_str())
            .collect();
        assert_eq!(paths, vec!["inbox/note.md"]);
    }

    #[test]
    fn upsert_and_remove_touch_single_rows() {
        let (_temp, root) = make_root();
        reindex_root(&root).expect("reindex");

        upsert_file(&root, "inbox/added.txt", 5, Some(123)).expect("upsert");
        let listed = list_index(&root).expect("list");
        assert!(listed
            .files
            .iter()
            .any(|file| file.relative_path == "inbox/added.txt"));

        // Upsert again updates rather than duplicating.
        upsert_file(&root, "inbox/added.txt", 9, Some(456)).expect("upsert update");
        let listed = list_index(&root).expect("list after update");
        let added: Vec<_> = listed
            .files
            .iter()
            .filter(|file| file.relative_path == "inbox/added.txt")
            .collect();
        assert_eq!(added.len(), 1);
        assert_eq!(added[0].size_bytes, 9);

        remove_file(&root, "inbox/added.txt").expect("remove");
        let listed = list_index(&root).expect("list after remove");
        assert!(!listed
            .files
            .iter()
            .any(|file| file.relative_path == "inbox/added.txt"));
    }

    #[test]
    fn search_matches_by_name_substring() {
        let (_temp, root) = make_root();
        reindex_root(&root).expect("reindex");

        let hits = search_index(&root, "photo").expect("search");
        let paths: Vec<&str> = hits
            .files
            .iter()
            .map(|file| file.relative_path.as_str())
            .collect();
        assert_eq!(paths, vec!["inbox/photo.png"]);

        let none = search_index(&root, "nonexistent").expect("search empty");
        assert!(none.files.is_empty());
    }

    #[test]
    fn search_treats_underscore_as_a_literal() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(&root).expect("create root");
        fs::write(root.join("a_b.txt"), "x").expect("write underscore file");
        fs::write(root.join("axb.txt"), "x").expect("write other file");
        reindex_root(&root).expect("reindex");

        let hits = search_index(&root, "a_b").expect("search");
        let paths: Vec<&str> = hits
            .files
            .iter()
            .map(|file| file.relative_path.as_str())
            .collect();
        assert_eq!(paths, vec!["a_b.txt"]);
    }
}
