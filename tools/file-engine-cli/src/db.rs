use std::error::Error;
use std::fmt;
use std::fs;
use std::future::Future;
use std::panic::resume_unwind;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::thread;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions};
use sqlx::Row;

use crate::journal::STATE_DIR;

pub const DB_FILE: &str = "mousekeeper.db";

/// The engine's public API is synchronous, but sqlx is async. Rather than turn every
/// analyze/propose/execute/undo signature async (and force tokio on the CLI and the Tauri
/// sync commands), we keep one shared runtime here and `block_on` sqlx calls at the storage
/// boundary. Callers never see a future.
fn runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .expect("build tokio runtime for sqlite")
    })
}

pub fn block_on<F>(future: F) -> F::Output
where
    F: Future + Send,
    F::Output: Send,
{
    match tokio::runtime::Handle::try_current() {
        Ok(handle)
            if matches!(
                handle.runtime_flavor(),
                tokio::runtime::RuntimeFlavor::MultiThread
            ) =>
        {
            // Desktop background work already runs on Tauri's Tokio executor. Tell that
            // executor this worker is blocking before entering the file engine's dedicated
            // SQLite runtime; calling Runtime::block_on directly here panics.
            tokio::task::block_in_place(|| runtime().block_on(future))
        }
        Ok(_) => thread::scope(|scope| {
            // A current-thread executor cannot use block_in_place. A scoped thread keeps
            // borrowed SQLx futures valid while ensuring block_on runs outside that executor.
            scope
                .spawn(move || runtime().block_on(future))
                .join()
                .unwrap_or_else(|panic| resume_unwind(panic))
        }),
        Err(_) => runtime().block_on(future),
    }
}

#[derive(Debug)]
pub enum DbError {
    CreateStateDir { path: PathBuf, message: String },
    Open { path: PathBuf, message: String },
    Migrate { message: String },
    Query { message: String },
}

pub fn db_path_for_root(root: &Path) -> PathBuf {
    root.join(STATE_DIR).join(DB_FILE)
}

/// Opens (creating if needed) a SQLite database at an arbitrary path with WAL journaling,
/// creating any missing parent directories. No schema is applied — the caller runs whichever
/// migrations that database needs. Used for the app-level managed-roots database, which lives
/// outside any single managed root.
pub fn open_pool_at(db_path: &Path) -> Result<SqlitePool, DbError> {
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent).map_err(|error| DbError::CreateStateDir {
            path: parent.to_path_buf(),
            message: error.to_string(),
        })?;
    }

    block_on(async {
        let options = SqliteConnectOptions::new()
            .filename(db_path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal);

        SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .map_err(|error| DbError::Open {
                path: db_path.to_path_buf(),
                message: error.to_string(),
            })
    })
}

/// Opens (creating if needed) the per-root SQLite database and applies the file-engine schema.
/// Each root carries its own `.mousekeeper/mousekeeper.db`, matching how the journal already
/// lives inside the managed root rather than in a shared app-level file.
pub fn open_root_db(root: &Path) -> Result<SqlitePool, DbError> {
    let db_path = db_path_for_root(root);
    let pool = open_pool_at(&db_path)?;
    block_on(async { migrate(&pool).await })?;
    Ok(pool)
}

/// Idempotent schema setup. Every table is `CREATE TABLE IF NOT EXISTS`, so opening an
/// existing database is a no-op and opening a fresh one brings it up to the current shape.
async fn migrate(pool: &SqlitePool) -> Result<(), DbError> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS file_index (
            relative_path     TEXT PRIMARY KEY,
            size_bytes        INTEGER NOT NULL,
            modified_unix_ms  INTEGER,
            extension         TEXT,
            file_id           TEXT
        )",
    )
    .execute(pool)
    .await
    .map_err(|error| DbError::Migrate {
        message: error.to_string(),
    })?;
    add_column_if_missing(
        pool,
        "PRAGMA table_info(file_index)",
        "file_id",
        "ALTER TABLE file_index ADD COLUMN file_id TEXT",
    )
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS file_search_entries (
            relative_path         TEXT PRIMARY KEY,
            parent_relative_path  TEXT NOT NULL,
            name                  TEXT NOT NULL,
            normalized_name       TEXT NOT NULL,
            entry_type            TEXT NOT NULL CHECK(entry_type IN ('file', 'directory')),
            extension             TEXT,
            size_bytes            INTEGER,
            modified_unix_ms      INTEGER
        )",
    )
    .execute(pool)
    .await
    .map_err(|error| DbError::Migrate {
        message: error.to_string(),
    })?;
    add_column_if_missing(
        pool,
        "PRAGMA table_info(file_search_entries)",
        "extension",
        "ALTER TABLE file_search_entries ADD COLUMN extension TEXT",
    )
    .await?;
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS file_search_entries_parent_name_idx
         ON file_search_entries(parent_relative_path, normalized_name, relative_path)",
    )
    .execute(pool)
    .await
    .map_err(|error| DbError::Migrate {
        message: error.to_string(),
    })?;
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS file_index_meta (
            singleton   INTEGER PRIMARY KEY CHECK(singleton = 1),
            generation  INTEGER NOT NULL CHECK(generation >= 0),
            initialized INTEGER NOT NULL DEFAULT 0 CHECK(initialized IN (0, 1))
        )",
    )
    .execute(pool)
    .await
    .map_err(|error| DbError::Migrate {
        message: error.to_string(),
    })?;
    add_column_if_missing(
        pool,
        "PRAGMA table_info(file_index_meta)",
        "initialized",
        "ALTER TABLE file_index_meta
         ADD COLUMN initialized INTEGER NOT NULL DEFAULT 0 CHECK(initialized IN (0, 1))",
    )
    .await?;
    sqlx::query(
        "INSERT OR IGNORE INTO file_index_meta(singleton, generation, initialized)
         VALUES (1, 0, 0)",
    )
    .execute(pool)
    .await
    .map_err(|error| DbError::Migrate {
        message: error.to_string(),
    })?;

    // Append-only event log: one row per journal event, `seq` preserving insertion order the
    // way line order did in the old journal.jsonl. Planned/executed/undo_planned/undone events
    // for one file share an operation_id.
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS operation_journal (
            seq              INTEGER PRIMARY KEY AUTOINCREMENT,
            operation_id     TEXT NOT NULL,
            status           TEXT NOT NULL,
            action           TEXT NOT NULL,
            from_path        TEXT NOT NULL,
            to_path          TEXT NOT NULL,
            created_unix_ms  INTEGER NOT NULL
        )",
    )
    .execute(pool)
    .await
    .map_err(|error| DbError::Migrate {
        message: error.to_string(),
    })?;

    Ok(())
}

async fn add_column_if_missing(
    pool: &SqlitePool,
    table_info_sql: &'static str,
    column: &str,
    alter_sql: &'static str,
) -> Result<(), DbError> {
    let rows = sqlx::query(table_info_sql)
        .fetch_all(pool)
        .await
        .map_err(|error| DbError::Migrate {
            message: error.to_string(),
        })?;
    if rows.iter().any(|row| {
        row.try_get::<String, _>("name")
            .is_ok_and(|name| name == column)
    }) {
        return Ok(());
    }

    sqlx::query(alter_sql)
        .execute(pool)
        .await
        .map(|_| ())
        .map_err(|error| DbError::Migrate {
            message: error.to_string(),
        })
}

impl fmt::Display for DbError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DbError::CreateStateDir { path, message } => {
                write!(
                    formatter,
                    "cannot create state directory {}: {message}",
                    path.display()
                )
            }
            DbError::Open { path, message } => {
                write!(
                    formatter,
                    "cannot open database {}: {message}",
                    path.display()
                )
            }
            DbError::Migrate { message } => write!(formatter, "cannot apply schema: {message}"),
            DbError::Query { message } => write!(formatter, "database query failed: {message}"),
        }
    }
}

impl Error for DbError {}

#[cfg(test)]
mod tests {
    use sqlx::Row;
    use tempfile::tempdir;

    use super::{block_on, db_path_for_root, open_pool_at, open_root_db};

    #[test]
    fn opens_database_and_round_trips_a_row() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path();

        let pool = open_root_db(root).expect("open db");

        block_on(async {
            sqlx::query("INSERT INTO file_index (relative_path, size_bytes) VALUES (?, ?)")
                .bind("inbox/note.md")
                .bind(12_i64)
                .execute(&pool)
                .await
                .expect("insert row");

            let size: i64 =
                sqlx::query_scalar("SELECT size_bytes FROM file_index WHERE relative_path = ?")
                    .bind("inbox/note.md")
                    .fetch_one(&pool)
                    .await
                    .expect("read row");

            assert_eq!(size, 12);
        });
    }

    #[test]
    fn blocks_safely_inside_a_multithread_runtime() {
        let outer = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("outer runtime");

        let value = outer.block_on(async { block_on(async { 42_u8 }) });

        assert_eq!(value, 42);
    }

    #[test]
    fn blocks_safely_inside_a_current_thread_runtime() {
        let outer = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("outer runtime");

        let value = outer.block_on(async { block_on(async { 42_u8 }) });

        assert_eq!(value, 42);
    }

    #[test]
    fn reopening_an_existing_database_preserves_rows() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path();

        let pool = open_root_db(root).expect("open db");
        block_on(async {
            sqlx::query("INSERT INTO file_index (relative_path, size_bytes) VALUES (?, ?)")
                .bind("a.txt")
                .bind(1_i64)
                .execute(&pool)
                .await
                .expect("insert row");
        });
        drop(pool);

        let pool = open_root_db(root).expect("reopen db");
        let count: i64 = block_on(async {
            sqlx::query_scalar("SELECT COUNT(*) FROM file_index")
                .fetch_one(&pool)
                .await
                .expect("count rows")
        });

        assert_eq!(count, 1);
    }

    #[test]
    fn migration_adds_file_id_to_existing_file_index() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path();
        let db_path = db_path_for_root(root);
        let pool = open_pool_at(&db_path).expect("open old db");
        block_on(async {
            sqlx::query(
                "CREATE TABLE file_index (
                    relative_path     TEXT PRIMARY KEY,
                    size_bytes        INTEGER NOT NULL,
                    modified_unix_ms  INTEGER,
                    extension         TEXT
                )",
            )
            .execute(&pool)
            .await
            .expect("create old file_index");
            sqlx::query(
                "INSERT INTO file_index (relative_path, size_bytes, modified_unix_ms, extension)
                 VALUES (?, ?, ?, ?)",
            )
            .bind("legacy.txt")
            .bind(7_i64)
            .bind(123_i64)
            .bind("txt")
            .execute(&pool)
            .await
            .expect("insert legacy row");
        });
        drop(pool);

        let pool = open_root_db(root).expect("migrate db");
        let row = block_on(async {
            sqlx::query("SELECT size_bytes, file_id FROM file_index WHERE relative_path = ?")
                .bind("legacy.txt")
                .fetch_one(&pool)
                .await
                .expect("legacy row remains readable")
        });
        let size: i64 = row.try_get("size_bytes").expect("size");
        let file_id: Option<String> = row.try_get("file_id").expect("file_id column");

        assert_eq!(size, 7);
        assert!(file_id.is_none());
    }
}
