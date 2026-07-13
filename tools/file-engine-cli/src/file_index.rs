use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::Serialize;
use sqlx::{Row, Sqlite, SqlitePool, Transaction};

use crate::analyzer::{analyze_root, AnalyzeError, SkippedEntry};
use crate::db::{block_on, open_root_db, DbError};
use crate::fs_safety::is_link_or_reparse_point;
use crate::journal::{STATE_DIR, TRASH_DIR};
use crate::path_guard::{PathGuard, PathGuardError};

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct FileIndexReport {
    pub root: String,
    pub generation: u64,
    pub files: Vec<IndexedFile>,
    pub skipped_entries: Vec<SkippedEntry>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct IndexedFile {
    pub relative_path: String,
    pub size_bytes: u64,
    pub modified_unix_ms: Option<u128>,
    pub extension: Option<String>,
    pub file_id: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IndexedSearchScope {
    CurrentDirectory,
    ManagedRoot,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct IndexedSearchEntry {
    pub name: String,
    pub relative_path: String,
    pub is_dir: bool,
    pub size_bytes: Option<u64>,
    pub modified_unix_ms: Option<u128>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct IndexedSearchPage {
    pub generation: u64,
    pub entries: Vec<IndexedSearchEntry>,
    pub next_offset: Option<usize>,
}

#[derive(Debug)]
pub enum FileIndexError {
    Guard(PathGuardError),
    Analyze(AnalyzeError),
    Db(DbError),
    Metadata { path: PathBuf, message: String },
    NotAFile { path: PathBuf },
    InvalidSearch(String),
    GenerationChanged { expected: u64, actual: u64 },
}

/// Rebuilds the SQLite `file_index` for a root from a fresh scan. Runs in one transaction so
/// the index never reflects a half-written state, and replaces the whole table rather than
/// diffing — the cheap, always-correct baseline that a watcher can later refine with
/// incremental `upsert_file` / `remove_file` calls.
pub fn reindex_root(root: impl AsRef<Path>) -> Result<FileIndexReport, FileIndexError> {
    let guard = PathGuard::new(root).map_err(FileIndexError::Guard)?;
    let analysis = analyze_root(guard.root()).map_err(FileIndexError::Analyze)?;
    let search_entries = collect_search_entries(guard.root())?;
    let pool = open_root_db(guard.root()).map_err(FileIndexError::Db)?;
    let files = analysis
        .files
        .iter()
        .map(|file| IndexedFile {
            relative_path: file.path.clone(),
            size_bytes: file.size_bytes,
            modified_unix_ms: file.modified_unix_ms,
            extension: extension_of(&file.path),
            file_id: file_id_for_relative_path(&guard, &file.path),
        })
        .collect::<Vec<_>>();

    let generation = block_on(async {
        let mut tx = pool.begin().await.map_err(query_error)?;
        sqlx::query("DELETE FROM file_index")
            .execute(&mut *tx)
            .await
            .map_err(query_error)?;

        for file in &files {
            sqlx::query(
                "INSERT INTO file_index (relative_path, size_bytes, modified_unix_ms, extension, file_id)
                 VALUES (?, ?, ?, ?, ?)",
            )
            .bind(&file.relative_path)
            .bind(file.size_bytes as i64)
            .bind(file.modified_unix_ms.map(|value| value as i64))
            .bind(&file.extension)
            .bind(&file.file_id)
            .execute(&mut *tx)
            .await
            .map_err(query_error)?;
        }

        sqlx::query("DELETE FROM file_search_entries")
            .execute(&mut *tx)
            .await
            .map_err(query_error)?;
        for entry in &search_entries {
            insert_search_entry(&mut tx, entry).await?;
        }

        sqlx::query("UPDATE file_index_meta SET initialized = 1 WHERE singleton = 1")
            .execute(&mut *tx)
            .await
            .map_err(query_error)?;

        let generation = bump_generation(&mut tx).await?;

        tx.commit().await.map_err(query_error)?;
        Ok(generation)
    })
    .map_err(FileIndexError::Db)?;

    Ok(FileIndexReport {
        root: analysis.root,
        generation,
        files,
        skipped_entries: analysis.skipped_entries,
    })
}

/// Reads one existing file under the managed root and writes its current metadata into the
/// index. Directory events should fall back to a full reindex because one directory change may
/// represent many child file changes.
pub fn upsert_existing_file(
    root: impl AsRef<Path>,
    relative_path: &str,
) -> Result<(), FileIndexError> {
    let guard = PathGuard::new(root).map_err(FileIndexError::Guard)?;
    let path = guard
        .resolve_existing(relative_path)
        .map_err(FileIndexError::Guard)?;
    let metadata = fs::symlink_metadata(&path).map_err(|error| FileIndexError::Metadata {
        path: path.clone(),
        message: error.to_string(),
    })?;

    if !metadata.is_file() {
        return Err(FileIndexError::NotAFile { path });
    }

    upsert_file(
        guard.root(),
        relative_path,
        metadata.len(),
        modified_unix_ms(&metadata),
    )
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
    let file_id = file_id_for_relative_path(&guard, relative_path);
    let search_entry =
        indexed_search_entry(relative_path, false, Some(size_bytes), modified_unix_ms).ok_or_else(
            || FileIndexError::InvalidSearch("indexed file path has no name".to_string()),
        )?;

    block_on(async {
        let mut tx = pool.begin().await.map_err(query_error)?;
        sqlx::query(
            "INSERT INTO file_index (relative_path, size_bytes, modified_unix_ms, extension, file_id)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT(relative_path) DO UPDATE SET
                 size_bytes = excluded.size_bytes,
                 modified_unix_ms = excluded.modified_unix_ms,
                 extension = excluded.extension,
                 file_id = excluded.file_id",
        )
        .bind(relative_path)
        .bind(size_bytes as i64)
        .bind(modified_unix_ms.map(|value| value as i64))
        .bind(extension_of(relative_path))
        .bind(&file_id)
        .execute(&mut *tx)
        .await
        .map_err(query_error)?;
        sqlx::query("DELETE FROM file_search_entries WHERE relative_path = ?")
            .bind(relative_path)
            .execute(&mut *tx)
            .await
            .map_err(query_error)?;
        insert_search_entry(&mut tx, &search_entry).await?;
        bump_generation(&mut tx).await?;
        tx.commit().await.map_err(query_error)
    })
    .map_err(FileIndexError::Db)
}

/// Removes a single file from the index, e.g. on a watcher delete event.
pub fn remove_file(root: impl AsRef<Path>, relative_path: &str) -> Result<(), FileIndexError> {
    remove_path(root, relative_path)
}

/// Removes either one indexed file or an indexed subtree. The subtree case is needed for
/// delete/rename events where the removed path may have been a directory and no longer exists
/// on disk for metadata inspection.
pub fn remove_path(root: impl AsRef<Path>, relative_path: &str) -> Result<(), FileIndexError> {
    let guard = PathGuard::new(root).map_err(FileIndexError::Guard)?;
    let pool = open_root_db(guard.root()).map_err(FileIndexError::Db)?;
    let prefix = format!("{}/%", escape_like(relative_path.trim_end_matches('/')));

    block_on(async {
        let mut tx = pool.begin().await.map_err(query_error)?;
        sqlx::query(
            "DELETE FROM file_index
             WHERE relative_path = ? OR relative_path LIKE ? ESCAPE '\\'",
        )
        .bind(relative_path)
        .bind(&prefix)
        .execute(&mut *tx)
        .await
        .map_err(query_error)?;
        sqlx::query(
            "DELETE FROM file_search_entries
             WHERE relative_path = ? OR relative_path LIKE ? ESCAPE '\\'",
        )
        .bind(relative_path)
        .bind(prefix)
        .execute(&mut *tx)
        .await
        .map_err(query_error)?;
        bump_generation(&mut tx).await?;
        tx.commit().await.map_err(query_error)
    })
    .map_err(FileIndexError::Db)
}

/// Clears only the disposable browse/search index. The operation journal lives in the same
/// database and is intentionally untouched so room disconnect never removes undo history.
pub fn clear_index(root: impl AsRef<Path>) -> Result<(), FileIndexError> {
    let guard = PathGuard::new(root).map_err(FileIndexError::Guard)?;
    let pool = open_root_db(guard.root()).map_err(FileIndexError::Db)?;
    block_on(async {
        let mut tx = pool.begin().await.map_err(query_error)?;
        sqlx::query("DELETE FROM file_index")
            .execute(&mut *tx)
            .await
            .map_err(query_error)?;
        sqlx::query("DELETE FROM file_search_entries")
            .execute(&mut *tx)
            .await
            .map_err(query_error)?;
        sqlx::query("UPDATE file_index_meta SET initialized = 0 WHERE singleton = 1")
            .execute(&mut *tx)
            .await
            .map_err(query_error)?;
        bump_generation(&mut tx).await?;
        tx.commit().await.map_err(query_error)
    })
    .map_err(FileIndexError::Db)
}

pub fn index_generation(root: impl AsRef<Path>) -> Result<u64, FileIndexError> {
    let guard = PathGuard::new(root).map_err(FileIndexError::Guard)?;
    let pool = open_root_db(guard.root()).map_err(FileIndexError::Db)?;
    block_on(async { read_generation(&pool).await }).map_err(FileIndexError::Db)
}

pub fn index_is_initialized(root: impl AsRef<Path>) -> Result<bool, FileIndexError> {
    let guard = PathGuard::new(root).map_err(FileIndexError::Guard)?;
    let pool = open_root_db(guard.root()).map_err(FileIndexError::Db)?;
    block_on(async {
        let initialized: i64 =
            sqlx::query_scalar("SELECT initialized FROM file_index_meta WHERE singleton = 1")
                .fetch_one(&pool)
                .await
                .map_err(query_error)?;
        Ok(initialized != 0)
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
    let generation =
        block_on(async { read_generation(&pool).await }).map_err(FileIndexError::Db)?;

    Ok(FileIndexReport {
        root: guard.root().display().to_string(),
        generation,
        files,
        skipped_entries: Vec::new(),
    })
}

async fn fetch_indexed(
    pool: &SqlitePool,
    name_query: Option<&str>,
) -> Result<Vec<IndexedFile>, DbError> {
    let rows = match name_query {
        Some(query) => sqlx::query(
            "SELECT fi.relative_path, fi.size_bytes, fi.modified_unix_ms, fi.extension, fi.file_id
             FROM file_index fi
             JOIN file_search_entries se ON se.relative_path = fi.relative_path
             WHERE instr(se.normalized_name, ?) > 0
             ORDER BY se.normalized_name, fi.relative_path",
        )
        .bind(normalize_name(query))
        .fetch_all(pool)
        .await
        .map_err(query_error)?,
        None => sqlx::query(
            "SELECT relative_path, size_bytes, modified_unix_ms, extension, file_id
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
        let file_id: Option<String> = row.try_get("file_id").map_err(query_error)?;

        files.push(IndexedFile {
            relative_path,
            size_bytes: size_bytes as u64,
            modified_unix_ms: modified.map(|value| value as u128),
            extension,
            file_id,
        });
    }

    Ok(files)
}

pub fn search_index_page(
    root: impl AsRef<Path>,
    query: &str,
    scope: IndexedSearchScope,
    relative_directory: &str,
    expected_generation: Option<u64>,
    offset: usize,
    limit: usize,
) -> Result<IndexedSearchPage, FileIndexError> {
    let query = query.trim();
    if !(2..=100).contains(&query.chars().count()) {
        return Err(FileIndexError::InvalidSearch(
            "file name query must contain between 2 and 100 Unicode code points".to_string(),
        ));
    }
    if !(1..=200).contains(&limit) {
        return Err(FileIndexError::InvalidSearch(
            "search page limit must be between 1 and 200".to_string(),
        ));
    }
    let guard = PathGuard::new(root).map_err(FileIndexError::Guard)?;
    let relative_directory = normalize_relative_directory(relative_directory)?;
    if scope == IndexedSearchScope::CurrentDirectory {
        validate_no_link_path(&guard, &relative_directory, true)?;
    }
    let pool = open_root_db(guard.root()).map_err(FileIndexError::Db)?;
    let normalized_query = normalize_name(query);

    block_on(async {
        let mut tx = pool
            .begin()
            .await
            .map_err(query_error)
            .map_err(FileIndexError::Db)?;
        let actual_i64: i64 =
            sqlx::query_scalar("SELECT generation FROM file_index_meta WHERE singleton = 1")
                .fetch_one(&mut *tx)
                .await
                .map_err(query_error)
                .map_err(FileIndexError::Db)?;
        let actual = u64::try_from(actual_i64).map_err(|_| {
            FileIndexError::InvalidSearch("file index generation is invalid".to_string())
        })?;
        if let Some(expected) = expected_generation {
            if expected != actual {
                return Err(FileIndexError::GenerationChanged { expected, actual });
            }
        }

        let mut entries = Vec::with_capacity(limit);
        let mut scanned_offset = offset;
        let mut next_offset = None;
        const FETCH_BATCH: usize = 256;
        'pages: loop {
            let rows = match scope {
                IndexedSearchScope::CurrentDirectory => {
                    sqlx::query(
                        "SELECT name, relative_path, entry_type, size_bytes, modified_unix_ms
                     FROM file_search_entries
                     WHERE parent_relative_path = ? AND instr(normalized_name, ?) > 0
                     ORDER BY normalized_name, relative_path
                     LIMIT ? OFFSET ?",
                    )
                    .bind(&relative_directory)
                    .bind(&normalized_query)
                    .bind(i64::try_from(FETCH_BATCH).unwrap_or(i64::MAX))
                    .bind(i64::try_from(scanned_offset).unwrap_or(i64::MAX))
                    .fetch_all(&mut *tx)
                    .await
                }
                IndexedSearchScope::ManagedRoot => {
                    sqlx::query(
                        "SELECT name, relative_path, entry_type, size_bytes, modified_unix_ms
                     FROM file_search_entries
                     WHERE instr(normalized_name, ?) > 0
                     ORDER BY normalized_name, relative_path
                     LIMIT ? OFFSET ?",
                    )
                    .bind(&normalized_query)
                    .bind(i64::try_from(FETCH_BATCH).unwrap_or(i64::MAX))
                    .bind(i64::try_from(scanned_offset).unwrap_or(i64::MAX))
                    .fetch_all(&mut *tx)
                    .await
                }
            }
            .map_err(query_error)
            .map_err(FileIndexError::Db)?;

            if rows.is_empty() {
                break;
            }
            let row_count = rows.len();
            for row in rows {
                let entry = indexed_search_entry_from_row(&row)?;
                if validate_indexed_search_hit(&guard, &entry) {
                    if entries.len() == limit {
                        next_offset = Some(scanned_offset);
                        break 'pages;
                    }
                    entries.push(entry);
                }
                scanned_offset = scanned_offset.saturating_add(1);
            }
            if row_count < FETCH_BATCH {
                break;
            }
        }
        tx.commit()
            .await
            .map_err(query_error)
            .map_err(FileIndexError::Db)?;
        Ok(IndexedSearchPage {
            generation: actual,
            entries,
            next_offset,
        })
    })
}

fn collect_search_entries(root: &Path) -> Result<Vec<IndexedSearchEntry>, FileIndexError> {
    fn visit(
        root: &Path,
        directory: &Path,
        entries: &mut Vec<IndexedSearchEntry>,
    ) -> Result<(), FileIndexError> {
        let read_dir = fs::read_dir(directory).map_err(|error| FileIndexError::Metadata {
            path: directory.to_path_buf(),
            message: error.to_string(),
        })?;
        for item in read_dir {
            let Ok(item) = item else { continue };
            let path = item.path();
            if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name == STATE_DIR || name == TRASH_DIR)
            {
                continue;
            }
            let Ok(metadata) = fs::symlink_metadata(&path) else {
                continue;
            };
            if is_link_or_reparse_point(&metadata, metadata.file_type()) {
                continue;
            }
            let Ok(relative) = path.strip_prefix(root) else {
                continue;
            };
            let relative = relative
                .components()
                .map(|component| component.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            if metadata.is_dir() {
                if let Some(entry) =
                    indexed_search_entry(&relative, true, None, modified_unix_ms(&metadata))
                {
                    entries.push(entry);
                }
                visit(root, &path, entries)?;
            } else if metadata.is_file() {
                if let Some(entry) = indexed_search_entry(
                    &relative,
                    false,
                    Some(metadata.len()),
                    modified_unix_ms(&metadata),
                ) {
                    entries.push(entry);
                }
            }
        }
        Ok(())
    }

    let mut entries = Vec::new();
    visit(root, root, &mut entries)?;
    entries.sort_by(|left, right| {
        normalize_name(&left.name)
            .cmp(&normalize_name(&right.name))
            .then_with(|| left.relative_path.cmp(&right.relative_path))
    });
    Ok(entries)
}

fn indexed_search_entry(
    relative_path: &str,
    is_dir: bool,
    size_bytes: Option<u64>,
    modified_unix_ms: Option<u128>,
) -> Option<IndexedSearchEntry> {
    let normalized_path = relative_path
        .split(['/', '\\'])
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("/");
    let name = Path::new(&normalized_path)
        .file_name()?
        .to_string_lossy()
        .to_string();
    Some(IndexedSearchEntry {
        name,
        relative_path: normalized_path,
        is_dir,
        size_bytes: if is_dir { None } else { size_bytes },
        modified_unix_ms,
    })
}

async fn insert_search_entry(
    tx: &mut Transaction<'_, Sqlite>,
    entry: &IndexedSearchEntry,
) -> Result<(), DbError> {
    let parent = Path::new(&entry.relative_path)
        .parent()
        .map(|path| {
            path.components()
                .map(|component| component.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/")
        })
        .unwrap_or_default();
    sqlx::query(
        "INSERT INTO file_search_entries (
             relative_path, parent_relative_path, name, normalized_name,
             entry_type, size_bytes, modified_unix_ms
         ) VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&entry.relative_path)
    .bind(parent)
    .bind(&entry.name)
    .bind(normalize_name(&entry.name))
    .bind(if entry.is_dir { "directory" } else { "file" })
    .bind(entry.size_bytes.map(|value| value as i64))
    .bind(entry.modified_unix_ms.map(|value| value as i64))
    .execute(&mut **tx)
    .await
    .map(|_| ())
    .map_err(query_error)
}

async fn bump_generation(tx: &mut Transaction<'_, Sqlite>) -> Result<u64, DbError> {
    sqlx::query("UPDATE file_index_meta SET generation = generation + 1 WHERE singleton = 1")
        .execute(&mut **tx)
        .await
        .map_err(query_error)?;
    let generation: i64 =
        sqlx::query_scalar("SELECT generation FROM file_index_meta WHERE singleton = 1")
            .fetch_one(&mut **tx)
            .await
            .map_err(query_error)?;
    u64::try_from(generation).map_err(|_| DbError::Query {
        message: "negative file index generation".to_string(),
    })
}

async fn read_generation(pool: &SqlitePool) -> Result<u64, DbError> {
    let generation: i64 =
        sqlx::query_scalar("SELECT generation FROM file_index_meta WHERE singleton = 1")
            .fetch_one(pool)
            .await
            .map_err(query_error)?;
    u64::try_from(generation).map_err(|_| DbError::Query {
        message: "negative file index generation".to_string(),
    })
}

fn indexed_search_entry_from_row(
    row: &sqlx::sqlite::SqliteRow,
) -> Result<IndexedSearchEntry, FileIndexError> {
    let entry_type: String = row
        .try_get("entry_type")
        .map_err(query_error)
        .map_err(FileIndexError::Db)?;
    let size_bytes: Option<i64> = row
        .try_get("size_bytes")
        .map_err(query_error)
        .map_err(FileIndexError::Db)?;
    let modified: Option<i64> = row
        .try_get("modified_unix_ms")
        .map_err(query_error)
        .map_err(FileIndexError::Db)?;
    Ok(IndexedSearchEntry {
        name: row
            .try_get("name")
            .map_err(query_error)
            .map_err(FileIndexError::Db)?,
        relative_path: row
            .try_get("relative_path")
            .map_err(query_error)
            .map_err(FileIndexError::Db)?,
        is_dir: entry_type == "directory",
        size_bytes: size_bytes.map(|value| value as u64),
        modified_unix_ms: modified.map(|value| value as u128),
    })
}

fn normalize_relative_directory(path: &str) -> Result<String, FileIndexError> {
    let path = Path::new(path);
    if path.components().any(|component| {
        matches!(
            component,
            Component::Prefix(_) | Component::RootDir | Component::ParentDir
        )
    }) {
        return Err(FileIndexError::InvalidSearch(
            "search directory must stay relative to the managed root".to_string(),
        ));
    }
    Ok(path
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy()),
            Component::CurDir => None,
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/"))
}

fn validate_no_link_path(
    guard: &PathGuard,
    relative_path: &str,
    require_directory: bool,
) -> Result<(), FileIndexError> {
    if relative_path.is_empty() {
        return Ok(());
    }
    let mut current = guard.root().to_path_buf();
    for component in Path::new(relative_path).components() {
        let Component::Normal(component) = component else {
            return Err(FileIndexError::InvalidSearch(
                "indexed path has an unsafe component".to_string(),
            ));
        };
        current.push(component);
        let metadata =
            fs::symlink_metadata(&current).map_err(|error| FileIndexError::Metadata {
                path: current.clone(),
                message: error.to_string(),
            })?;
        if is_link_or_reparse_point(&metadata, metadata.file_type()) {
            return Err(FileIndexError::InvalidSearch(
                "indexed path crosses a symlink, junction, or reparse point".to_string(),
            ));
        }
    }
    let resolved = guard
        .resolve_existing(relative_path)
        .map_err(FileIndexError::Guard)?;
    if require_directory && !resolved.is_dir() {
        return Err(FileIndexError::InvalidSearch(
            "search scope is not a directory".to_string(),
        ));
    }
    Ok(())
}

fn validate_indexed_search_hit(guard: &PathGuard, entry: &IndexedSearchEntry) -> bool {
    if entry.relative_path.split('/').any(|segment| {
        segment == STATE_DIR || segment == TRASH_DIR || segment.is_empty() || segment == ".."
    }) {
        return false;
    }
    if validate_no_link_path(guard, &entry.relative_path, entry.is_dir).is_err() {
        return false;
    }
    let path = guard.root().join(&entry.relative_path);
    fs::symlink_metadata(path).is_ok_and(|metadata| {
        !is_link_or_reparse_point(&metadata, metadata.file_type())
            && if entry.is_dir {
                metadata.is_dir()
            } else {
                metadata.is_file()
            }
    })
}

fn normalize_name(value: &str) -> String {
    value.to_lowercase()
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

fn file_id_for_relative_path(guard: &PathGuard, relative_path: &str) -> Option<String> {
    guard
        .resolve_existing(relative_path)
        .ok()
        .and_then(|path| crate::file_identity::file_id_for_path(&path))
}

fn modified_unix_ms(metadata: &fs::Metadata) -> Option<u128> {
    metadata
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_millis())
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
            FileIndexError::Metadata { path, message } => {
                write!(
                    formatter,
                    "cannot read metadata {}: {message}",
                    path.display()
                )
            }
            FileIndexError::NotAFile { path } => {
                write!(formatter, "index entry is not a file: {}", path.display())
            }
            FileIndexError::InvalidSearch(message) => {
                write!(formatter, "invalid indexed search: {message}")
            }
            FileIndexError::GenerationChanged { expected, actual } => write!(
                formatter,
                "file index generation changed (expected {expected}, actual {actual})"
            ),
        }
    }
}

impl Error for FileIndexError {}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::{
        clear_index, list_index, reindex_root, remove_file, remove_path, search_index,
        search_index_page, upsert_existing_file, upsert_file, FileIndexError, IndexedSearchScope,
    };

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
        #[cfg(any(unix, windows))]
        assert!(
            report.files.iter().all(|file| file.file_id.is_some()),
            "supported platforms should persist an OS-backed file id"
        );
        assert!(report.skipped_entries.is_empty());
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
    fn upsert_existing_file_reads_current_metadata() {
        let (_temp, root) = make_root();
        reindex_root(&root).expect("reindex");
        fs::write(root.join("inbox").join("fresh.txt"), "fresh").expect("write fresh");

        upsert_existing_file(&root, "inbox/fresh.txt").expect("upsert existing");

        let listed = list_index(&root).expect("list");
        let fresh = listed
            .files
            .iter()
            .find(|file| file.relative_path == "inbox/fresh.txt")
            .expect("fresh indexed");
        assert_eq!(fresh.size_bytes, 5);
        assert_eq!(fresh.extension.as_deref(), Some("txt"));
        #[cfg(any(unix, windows))]
        assert!(fresh.file_id.is_some());
    }

    #[test]
    fn remove_path_clears_indexed_subtrees() {
        let (_temp, root) = make_root();
        reindex_root(&root).expect("reindex");

        remove_path(&root, "inbox").expect("remove subtree");

        let listed = list_index(&root).expect("list");
        assert!(listed.files.is_empty());
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

    #[test]
    fn paged_search_supports_both_scopes_and_includes_directories() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("current").join("NeedleFolder")).expect("current dir");
        fs::create_dir_all(root.join("other")).expect("other dir");
        fs::write(root.join("current").join("needle-local.txt"), "x").expect("local");
        fs::write(root.join("other").join("NEEDLE-remote.txt"), "x").expect("remote");
        reindex_root(&root).expect("reindex");

        let current = search_index_page(
            &root,
            "needle",
            IndexedSearchScope::CurrentDirectory,
            "current",
            None,
            0,
            200,
        )
        .expect("current search");
        let current_paths = current
            .entries
            .iter()
            .map(|entry| entry.relative_path.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            current_paths,
            vec!["current/needle-local.txt", "current/NeedleFolder"]
        );
        assert!(current.entries.iter().any(|entry| entry.is_dir));

        let whole_root = search_index_page(
            &root,
            "needle",
            IndexedSearchScope::ManagedRoot,
            "",
            None,
            0,
            200,
        )
        .expect("root search");
        assert_eq!(whole_root.entries.len(), 3);
        assert!(whole_root
            .entries
            .iter()
            .any(|entry| entry.relative_path == "other/NEEDLE-remote.txt"));
    }

    #[test]
    fn search_matches_basename_not_parent_path() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("needle-parent")).expect("parent");
        fs::write(root.join("needle-parent").join("plain.txt"), "x").expect("plain");
        reindex_root(&root).expect("reindex");

        let page = search_index_page(
            &root,
            "needle",
            IndexedSearchScope::ManagedRoot,
            "",
            None,
            0,
            200,
        )
        .expect("search");
        let paths = page
            .entries
            .iter()
            .map(|entry| entry.relative_path.as_str())
            .collect::<Vec<_>>();
        assert_eq!(paths, vec!["needle-parent"]);
    }

    #[test]
    fn paged_search_rejects_queries_outside_unicode_code_point_limits() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(&root).expect("root");

        for query in ["한", "🙂"] {
            let error = search_index_page(
                &root,
                query,
                IndexedSearchScope::ManagedRoot,
                "",
                None,
                0,
                200,
            )
            .expect_err("single Unicode code point must be rejected");
            assert!(matches!(error, FileIndexError::InvalidSearch(_)));
        }

        let too_long = "가".repeat(101);
        let error = search_index_page(
            &root,
            &too_long,
            IndexedSearchScope::ManagedRoot,
            "",
            None,
            0,
            200,
        )
        .expect_err("more than one hundred Unicode code points must be rejected");
        assert!(matches!(error, FileIndexError::InvalidSearch(_)));
    }

    #[test]
    fn generation_bound_cursor_is_invalidated_after_index_change() {
        let (_temp, root) = make_root();
        let indexed = reindex_root(&root).expect("reindex");
        let first = search_index_page(
            &root,
            "ot",
            IndexedSearchScope::ManagedRoot,
            "",
            Some(indexed.generation),
            0,
            1,
        )
        .expect("first page");
        assert!(first.next_offset.is_some());

        fs::write(root.join("inbox").join("new.txt"), "new").expect("new file");
        upsert_existing_file(&root, "inbox/new.txt").expect("upsert");
        let error = search_index_page(
            &root,
            "ot",
            IndexedSearchScope::ManagedRoot,
            "",
            Some(first.generation),
            first.next_offset.unwrap_or_default(),
            1,
        )
        .expect_err("stale generation");
        assert!(matches!(error, FileIndexError::GenerationChanged { .. }));
    }

    #[test]
    fn search_paginates_two_hundred_and_filters_stale_hits() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(&root).expect("root");
        for index in 0..205 {
            fs::write(root.join(format!("match-{index:03}.txt")), "x").expect("file");
        }
        let indexed = reindex_root(&root).expect("reindex");
        fs::remove_file(root.join("match-000.txt")).expect("remove after indexing");

        let first = search_index_page(
            &root,
            "match",
            IndexedSearchScope::ManagedRoot,
            "",
            Some(indexed.generation),
            0,
            200,
        )
        .expect("first");
        assert_eq!(first.entries.len(), 200);
        assert!(first
            .entries
            .iter()
            .all(|entry| entry.relative_path != "match-000.txt"));
        let second = search_index_page(
            &root,
            "match",
            IndexedSearchScope::ManagedRoot,
            "",
            Some(first.generation),
            first.next_offset.expect("next cursor"),
            200,
        )
        .expect("second");
        assert_eq!(second.entries.len(), 4);
        assert!(second.next_offset.is_none());
    }

    #[test]
    fn clearing_disposable_index_preserves_operation_journal() {
        use crate::journal::{JournalAction, JournalEntry, JournalStatus, JournalStore};

        let (_temp, root) = make_root();
        reindex_root(&root).expect("reindex");
        let journal = JournalStore::open(&root).expect("journal");
        journal
            .append(&JournalEntry {
                operation_id: "op-preserved".to_string(),
                status: JournalStatus::Planned,
                action: JournalAction::Move,
                from: "inbox/note.md".to_string(),
                to: "Documents/note.md".to_string(),
                created_unix_ms: 1,
            })
            .expect("append");

        clear_index(&root).expect("clear index");

        assert!(list_index(&root).expect("index").files.is_empty());
        let entries = JournalStore::open(&root)
            .expect("journal after clear")
            .read_all()
            .expect("read journal");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].operation_id, "op-preserved");
    }

    #[test]
    fn reindex_keeps_downloads_like_names_searchable() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("Downloads");
        fs::create_dir_all(&root).expect("create downloads");
        fs::write(root.join("보고서 (최종).pdf"), "pdf").expect("write korean pdf");
        fs::write(root.join("installer.tmp"), "tmp").expect("write temp file");

        let indexed = reindex_root(&root).expect("reindex");
        assert_eq!(indexed.files.len(), 2);
        assert!(indexed.skipped_entries.is_empty());

        let hits = search_index(&root, "보고서").expect("search korean name");
        assert_eq!(hits.files.len(), 1);
        assert_eq!(hits.files[0].relative_path, "보고서 (최종).pdf");
    }
}
