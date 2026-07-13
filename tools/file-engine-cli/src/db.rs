use std::error::Error;
use std::fmt;
use std::fs;
use std::future::Future;
use std::panic::resume_unwind;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::thread;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions};

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
            extension         TEXT
        )",
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
    use tempfile::tempdir;

    use super::{block_on, open_root_db};

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
}
