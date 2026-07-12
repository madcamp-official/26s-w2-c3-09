use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Mutex;

use file_engine_cli::db::{block_on, open_pool_at};
use sqlx::{Row, SqlitePool};

const SYNC_STREAM: &str = "user-events";

#[derive(Debug, Default)]
pub struct AgentSyncStore {
    cursors: Mutex<BTreeMap<String, u64>>,
    pool: Mutex<Option<SqlitePool>>,
}

impl AgentSyncStore {
    pub fn load_from_db(&self, path: impl AsRef<Path>) -> Result<(), String> {
        let pool = open_pool_at(path.as_ref()).map_err(|error| error.to_string())?;
        let loaded = block_on(async {
            sqlx::query(
                "CREATE TABLE IF NOT EXISTS sync_cursors (
                    device_id TEXT NOT NULL,
                    stream TEXT NOT NULL,
                    last_sequence INTEGER NOT NULL CHECK(last_sequence >= 0),
                    PRIMARY KEY(device_id, stream)
                )",
            )
            .execute(&pool)
            .await
            .map_err(|error| error.to_string())?;
            let rows =
                sqlx::query("SELECT device_id, last_sequence FROM sync_cursors WHERE stream = ?")
                    .bind(SYNC_STREAM)
                    .fetch_all(&pool)
                    .await
                    .map_err(|error| error.to_string())?;
            rows.into_iter()
                .map(|row| {
                    let device_id = row.try_get::<String, _>("device_id")?;
                    let sequence = row.try_get::<i64, _>("last_sequence")?;
                    let sequence = u64::try_from(sequence).map_err(|_| {
                        sqlx::Error::Protocol("negative sync cursor in database".to_string())
                    })?;
                    Ok((device_id, sequence))
                })
                .collect::<Result<BTreeMap<_, _>, sqlx::Error>>()
                .map_err(|error| error.to_string())
        })?;

        *self.pool.lock().map_err(|_| "sync pool lock poisoned")? = Some(pool);
        *self
            .cursors
            .lock()
            .map_err(|_| "sync cursor lock poisoned")? = loaded;
        Ok(())
    }

    pub fn cursor(&self, device_id: &str) -> Result<u64, String> {
        Ok(*self
            .cursors
            .lock()
            .map_err(|_| "sync cursor lock poisoned")?
            .get(device_id)
            .unwrap_or(&0))
    }

    pub async fn advance(&self, device_id: &str, sequence: u64) -> Result<u64, String> {
        let current = self.cursor(device_id)?;
        if sequence < current {
            return Err(format!(
                "sync cursor cannot move backwards: current={current}, requested={sequence}"
            ));
        }
        let sequence_i64 = i64::try_from(sequence)
            .map_err(|_| "sync cursor exceeds SQLite integer range".to_string())?;
        let pool = self
            .pool
            .lock()
            .map_err(|_| "sync pool lock poisoned")?
            .clone();
        if let Some(pool) = pool {
            sqlx::query(
                "INSERT INTO sync_cursors(device_id, stream, last_sequence)
                 VALUES (?, ?, ?)
                 ON CONFLICT(device_id, stream) DO UPDATE SET last_sequence = excluded.last_sequence
                 WHERE excluded.last_sequence >= sync_cursors.last_sequence",
            )
            .bind(device_id)
            .bind(SYNC_STREAM)
            .bind(sequence_i64)
            .execute(&pool)
            .await
            .map_err(|error| error.to_string())?;
        }
        self.cursors
            .lock()
            .map_err(|_| "sync cursor lock poisoned")?
            .insert(device_id.to_string(), sequence);
        Ok(sequence)
    }

    pub async fn clear_device(&self, device_id: &str) -> Result<(), String> {
        let pool = self
            .pool
            .lock()
            .map_err(|_| "sync pool lock poisoned")?
            .clone();
        if let Some(pool) = pool {
            sqlx::query("DELETE FROM sync_cursors WHERE device_id = ?")
                .bind(device_id)
                .execute(&pool)
                .await
                .map_err(|error| error.to_string())?;
        }
        self.cursors
            .lock()
            .map_err(|_| "sync cursor lock poisoned")?
            .remove(device_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use file_engine_cli::db::block_on;

    use super::AgentSyncStore;

    #[test]
    fn cursor_persists_and_never_moves_backwards() {
        let temp = tempfile::tempdir().expect("temp dir");
        let db = temp.path().join("agent-sync.db");
        let store = AgentSyncStore::default();
        store.load_from_db(&db).expect("load store");

        block_on(store.advance("device-1", 12)).expect("advance cursor");
        assert_eq!(store.cursor("device-1").expect("cursor"), 12);
        assert!(block_on(store.advance("device-1", 11)).is_err());

        let restored = AgentSyncStore::default();
        restored.load_from_db(&db).expect("restore store");
        assert_eq!(restored.cursor("device-1").expect("restored cursor"), 12);
        block_on(restored.clear_device("device-1")).expect("clear cursor");
        assert_eq!(restored.cursor("device-1").expect("cleared cursor"), 0);
    }
}
