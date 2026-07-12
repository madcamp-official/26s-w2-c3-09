use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use file_engine_cli::db::{block_on, open_pool_at};
use sqlx::{Row, SqlitePool};

/// Durable store for desktop→server mutations. Instead of firing an HTTP request inline (and
/// losing it if the network drops or the process exits after a local file change already
/// happened), the caller writes the mutation here first. A background flush loop then delivers
/// pending rows to the server with retries. Every stored mutation carries the server idempotency
/// key it will be sent with, so a redelivery after a crash is safe on the server side.
#[derive(Debug, Default)]
pub struct OutboxStore {
    pool: Mutex<Option<SqlitePool>>,
}

/// A pending mutation ready to be delivered to the server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutboxItem {
    pub id: i64,
    pub kind: String,
    pub idempotency_key: String,
    pub payload_json: String,
    pub attempt_count: i64,
}

impl OutboxStore {
    pub fn load_from_db(&self, path: impl AsRef<Path>) -> Result<(), String> {
        let pool = open_pool_at(path.as_ref()).map_err(|error| error.to_string())?;
        block_on(migrate(&pool))?;
        *self
            .pool
            .lock()
            .map_err(|_| "outbox pool lock poisoned".to_string())? = Some(pool);
        Ok(())
    }

    /// Records a mutation for later delivery. Deduplicates on `(kind, idempotency_key)`: if a row
    /// with the same key already exists in any state (pending, sent, or dead) this is a no-op, so
    /// re-processing the same decision or command never queues a duplicate server call.
    pub fn enqueue(
        &self,
        kind: &str,
        idempotency_key: &str,
        payload_json: &str,
    ) -> Result<(), String> {
        let pool = self.pool()?;
        let Some(pool) = pool else {
            return Err("outbox database is not initialized".to_string());
        };
        let now = unix_ms();
        block_on(async move {
            sqlx::query(
                "INSERT INTO desktop_outbox
                    (kind, idempotency_key, payload_json, state, attempt_count, created_unix_ms, updated_unix_ms)
                 VALUES (?1, ?2, ?3, 'pending', 0, ?4, ?4)
                 ON CONFLICT(kind, idempotency_key) DO NOTHING",
            )
            .bind(kind)
            .bind(idempotency_key)
            .bind(payload_json)
            .bind(now)
            .execute(&pool)
            .await
            .map_err(|error| format!("cannot enqueue outbox item: {error}"))
        })?;
        Ok(())
    }

    /// Reads the oldest pending mutations, in enqueue order, for delivery.
    pub fn pending_batch(&self, limit: i64) -> Result<Vec<OutboxItem>, String> {
        let pool = self.pool()?;
        let Some(pool) = pool else {
            return Ok(Vec::new());
        };
        block_on(async move {
            let rows = sqlx::query(
                "SELECT id, kind, idempotency_key, payload_json, attempt_count
                 FROM desktop_outbox
                 WHERE state = 'pending'
                 ORDER BY id ASC
                 LIMIT ?1",
            )
            .bind(limit)
            .fetch_all(&pool)
            .await
            .map_err(|error| format!("cannot read outbox batch: {error}"))?;

            rows.into_iter()
                .map(|row| {
                    Ok(OutboxItem {
                        id: row.try_get::<i64, _>("id").map_err(err)?,
                        kind: row.try_get::<String, _>("kind").map_err(err)?,
                        idempotency_key: row
                            .try_get::<String, _>("idempotency_key")
                            .map_err(err)?,
                        payload_json: row.try_get::<String, _>("payload_json").map_err(err)?,
                        attempt_count: row.try_get::<i64, _>("attempt_count").map_err(err)?,
                    })
                })
                .collect::<Result<Vec<_>, String>>()
        })
    }

    /// Marks a mutation as delivered.
    pub fn mark_sent(&self, id: i64) -> Result<(), String> {
        self.update_state(id, "sent", None)
    }

    /// Records a transient delivery failure. The row stays `pending` so the next flush retries it.
    /// Only the error code is stored — never a server message, token, or path.
    pub fn mark_retry(&self, id: i64, error_code: &str) -> Result<(), String> {
        self.update_state(id, "pending", Some(error_code))
    }

    /// Records a terminal delivery failure (e.g. the server rejected the payload). The row moves to
    /// `failed` and is never retried, so a permanently-bad mutation cannot loop forever.
    pub fn mark_failed(&self, id: i64, error_code: &str) -> Result<(), String> {
        self.update_state(id, "failed", Some(error_code))
    }

    fn update_state(&self, id: i64, state: &str, error_code: Option<&str>) -> Result<(), String> {
        let pool = self.pool()?;
        let Some(pool) = pool else {
            return Err("outbox database is not initialized".to_string());
        };
        let now = unix_ms();
        // A send attempt bumps the counter unless it succeeded.
        let increment = if state == "sent" { 0 } else { 1 };
        let state = state.to_string();
        let error_code = error_code.map(|code| code.to_string());
        block_on(async move {
            sqlx::query(
                "UPDATE desktop_outbox
                 SET state = ?2,
                     attempt_count = attempt_count + ?3,
                     last_error = COALESCE(?4, last_error),
                     updated_unix_ms = ?5
                 WHERE id = ?1",
            )
            .bind(id)
            .bind(state)
            .bind(increment)
            .bind(error_code)
            .bind(now)
            .execute(&pool)
            .await
            .map_err(|error| format!("cannot update outbox item: {error}"))
        })?;
        Ok(())
    }

    fn pool(&self) -> Result<Option<SqlitePool>, String> {
        Ok(self
            .pool
            .lock()
            .map_err(|_| "outbox pool lock poisoned".to_string())?
            .clone())
    }

    #[cfg(test)]
    fn state_of(&self, id: i64) -> Result<Option<(String, i64, Option<String>)>, String> {
        let pool = self.pool()?.expect("outbox pool");
        block_on(async move {
            let row = sqlx::query(
                "SELECT state, attempt_count, last_error FROM desktop_outbox WHERE id = ?1",
            )
            .bind(id)
            .fetch_optional(&pool)
            .await
            .map_err(|error| error.to_string())?;
            Ok(row.map(|row| {
                (
                    row.try_get::<String, _>("state").expect("state"),
                    row.try_get::<i64, _>("attempt_count").expect("attempts"),
                    row.try_get::<Option<String>, _>("last_error")
                        .expect("error"),
                )
            }))
        })
    }
}

async fn migrate(pool: &SqlitePool) -> Result<(), String> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS desktop_outbox (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            kind TEXT NOT NULL,
            idempotency_key TEXT NOT NULL,
            payload_json TEXT NOT NULL,
            state TEXT NOT NULL DEFAULT 'pending',
            attempt_count INTEGER NOT NULL DEFAULT 0,
            last_error TEXT,
            created_unix_ms INTEGER NOT NULL,
            updated_unix_ms INTEGER NOT NULL,
            UNIQUE(kind, idempotency_key)
        )",
    )
    .execute(pool)
    .await
    .map_err(|error| format!("cannot migrate outbox table: {error}"))?;
    Ok(())
}

fn err(error: sqlx::Error) -> String {
    error.to_string()
}

fn unix_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::OutboxStore;

    fn store_at(path: &std::path::Path) -> OutboxStore {
        let store = OutboxStore::default();
        store.load_from_db(path).expect("load outbox");
        store
    }

    #[test]
    fn enqueue_dedupes_on_kind_and_key() {
        let temp = tempfile::tempdir().expect("temp dir");
        let store = store_at(&temp.path().join("outbox.db"));

        store
            .enqueue("execution_result", "exec-1", r#"{"a":1}"#)
            .expect("enqueue");
        // Same key re-enqueued (e.g. the decision was reprocessed) must not create a duplicate.
        store
            .enqueue("execution_result", "exec-1", r#"{"a":1}"#)
            .expect("enqueue again");

        let batch = store.pending_batch(10).expect("batch");
        assert_eq!(batch.len(), 1);
        assert_eq!(batch[0].kind, "execution_result");
        assert_eq!(batch[0].idempotency_key, "exec-1");
    }

    #[test]
    fn pending_batch_is_ordered_and_excludes_sent_and_failed() {
        let temp = tempfile::tempdir().expect("temp dir");
        let store = store_at(&temp.path().join("outbox.db"));

        store.enqueue("proposal", "cmd-1", "{}").expect("enqueue 1");
        store.enqueue("proposal", "cmd-2", "{}").expect("enqueue 2");
        store.enqueue("proposal", "cmd-3", "{}").expect("enqueue 3");

        let batch = store.pending_batch(10).expect("batch");
        assert_eq!(
            batch
                .iter()
                .map(|i| i.idempotency_key.as_str())
                .collect::<Vec<_>>(),
            vec!["cmd-1", "cmd-2", "cmd-3"]
        );

        store.mark_sent(batch[0].id).expect("sent");
        store
            .mark_failed(batch[1].id, "VALIDATION_FAILED")
            .expect("failed");

        let remaining = store.pending_batch(10).expect("batch again");
        assert_eq!(
            remaining
                .iter()
                .map(|i| i.idempotency_key.as_str())
                .collect::<Vec<_>>(),
            vec!["cmd-3"]
        );
    }

    #[test]
    fn transient_retry_keeps_item_pending_and_counts_attempts() {
        let temp = tempfile::tempdir().expect("temp dir");
        let store = store_at(&temp.path().join("outbox.db"));

        store
            .enqueue("execution_result", "exec-1", "{}")
            .expect("enqueue");
        let id = store.pending_batch(10).expect("batch")[0].id;

        store
            .mark_retry(id, "TRANSPORT_UNAVAILABLE")
            .expect("retry");
        store
            .mark_retry(id, "TRANSPORT_UNAVAILABLE")
            .expect("retry");

        let (state, attempts, last_error) = store.state_of(id).expect("state").expect("row");
        assert_eq!(state, "pending");
        assert_eq!(attempts, 2);
        assert_eq!(last_error.as_deref(), Some("TRANSPORT_UNAVAILABLE"));
        // Still eligible for delivery.
        assert_eq!(store.pending_batch(10).expect("batch").len(), 1);
    }

    #[test]
    fn queued_mutation_survives_process_restart() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("outbox.db");

        {
            let store = store_at(&path);
            store
                .enqueue("execution_result", "exec-1", r#"{"status":"SUCCEEDED"}"#)
                .expect("enqueue before crash");
        }

        // A brand new store (simulating a restart) still sees the undelivered mutation.
        let restored = store_at(&path);
        let batch = restored.pending_batch(10).expect("batch");
        assert_eq!(batch.len(), 1);
        assert_eq!(batch[0].payload_json, r#"{"status":"SUCCEEDED"}"#);
    }
}
