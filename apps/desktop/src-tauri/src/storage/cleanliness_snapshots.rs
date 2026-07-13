use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Mutex;

use file_engine_cli::db::{block_on, open_pool_at};
use sqlx::{Row, SqlitePool};

use crate::cleanliness::CleanlinessSnapshot;

/// Durable local source for the exact snapshots shown by the dashboard. Keeping the serialized
/// object avoids a second calculation when the UI starts after startup reconciliation completed.
#[derive(Debug, Default)]
pub struct CleanlinessSnapshotStore {
    snapshots: Mutex<BTreeMap<String, CleanlinessSnapshot>>,
    pool: Mutex<Option<SqlitePool>>,
}

impl CleanlinessSnapshotStore {
    pub fn load_from_db(&self, path: impl AsRef<Path>) -> Result<(), String> {
        let pool = open_pool_at(path.as_ref()).map_err(|error| error.to_string())?;
        let loaded = block_on(async {
            sqlx::query(
                "CREATE TABLE IF NOT EXISTS cleanliness_snapshots (
                    root_id        TEXT PRIMARY KEY,
                    calculated_at  TEXT NOT NULL,
                    snapshot_json  TEXT NOT NULL
                )",
            )
            .execute(&pool)
            .await
            .map_err(|error| format!("cannot migrate cleanliness snapshots: {error}"))?;
            let rows = sqlx::query(
                "SELECT root_id, snapshot_json FROM cleanliness_snapshots ORDER BY root_id",
            )
            .fetch_all(&pool)
            .await
            .map_err(|error| format!("cannot read cleanliness snapshots: {error}"))?;
            rows.into_iter()
                .map(|row| {
                    let root_id: String = row
                        .try_get("root_id")
                        .map_err(|error| format!("cannot read cleanliness root id: {error}"))?;
                    let json: String = row
                        .try_get("snapshot_json")
                        .map_err(|error| format!("cannot read cleanliness snapshot: {error}"))?;
                    let snapshot = serde_json::from_str(&json)
                        .map_err(|error| format!("cannot decode cleanliness snapshot: {error}"))?;
                    Ok((root_id, snapshot))
                })
                .collect::<Result<BTreeMap<_, _>, String>>()
        })?;
        *self
            .pool
            .lock()
            .map_err(|_| "cleanliness snapshot pool lock poisoned".to_string())? = Some(pool);
        *self
            .snapshots
            .lock()
            .map_err(|_| "cleanliness snapshot store lock poisoned".to_string())? = loaded;
        Ok(())
    }

    /// Saves only a strictly newer calculation. RFC3339 timestamps emitted by the engine have a
    /// fixed UTC millisecond shape, so lexical order is chronological order.
    pub fn save_latest(
        &self,
        root_id: &str,
        snapshot: &CleanlinessSnapshot,
    ) -> Result<bool, String> {
        if root_id.is_empty() {
            return Err("cleanliness snapshot root id cannot be empty".to_string());
        }
        let mut snapshots = self
            .snapshots
            .lock()
            .map_err(|_| "cleanliness snapshot store lock poisoned".to_string())?;
        if snapshots
            .get(root_id)
            .is_some_and(|current| current.calculated_at >= snapshot.calculated_at)
        {
            return Ok(false);
        }
        let pool = self
            .pool
            .lock()
            .map_err(|_| "cleanliness snapshot pool lock poisoned".to_string())?
            .clone()
            .ok_or_else(|| "cleanliness snapshot database is not initialized".to_string())?;
        let json = serde_json::to_string(snapshot)
            .map_err(|error| format!("cannot encode cleanliness snapshot: {error}"))?;
        let root_id_owned = root_id.to_string();
        let calculated_at = snapshot.calculated_at.clone();
        let changed = block_on(async move {
            sqlx::query(
                "INSERT INTO cleanliness_snapshots (root_id, calculated_at, snapshot_json)
                 VALUES (?, ?, ?)
                 ON CONFLICT(root_id) DO UPDATE SET
                    calculated_at = excluded.calculated_at,
                    snapshot_json = excluded.snapshot_json
                 WHERE excluded.calculated_at > cleanliness_snapshots.calculated_at",
            )
            .bind(root_id_owned)
            .bind(calculated_at)
            .bind(json)
            .execute(&pool)
            .await
            .map(|result| result.rows_affected() > 0)
            .map_err(|error| format!("cannot persist cleanliness snapshot: {error}"))
        })?;
        if changed {
            snapshots.insert(root_id.to_string(), snapshot.clone());
        }
        Ok(changed)
    }

    pub fn get(&self, root_id: &str) -> Result<Option<CleanlinessSnapshot>, String> {
        Ok(self
            .snapshots
            .lock()
            .map_err(|_| "cleanliness snapshot store lock poisoned".to_string())?
            .get(root_id)
            .cloned())
    }
}

#[cfg(test)]
mod tests {
    use super::CleanlinessSnapshotStore;
    use crate::cleanliness::{
        CleanlinessMetrics, CleanlinessSnapshot, CLEANLINESS_FORMULA_VERSION,
    };

    fn snapshot(calculated_at: &str, score: u8) -> CleanlinessSnapshot {
        CleanlinessSnapshot {
            formula_version: CLEANLINESS_FORMULA_VERSION.to_string(),
            score,
            metrics: CleanlinessMetrics {
                total_file_count: 1,
                managed_file_count: 1,
                unorganized_file_count: 0,
                deductions: Vec::new(),
            },
            calculated_at: calculated_at.to_string(),
        }
    }

    #[test]
    fn latest_snapshot_never_moves_backwards_and_survives_restart() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("cleanliness.db");
        let store = CleanlinessSnapshotStore::default();
        store.load_from_db(&path).expect("load");
        let newer = snapshot("2026-07-13T00:00:00.002Z", 90);
        let older = snapshot("2026-07-13T00:00:00.001Z", 10);
        assert!(store.save_latest("root-1", &newer).expect("newer"));
        assert!(!store.save_latest("root-1", &older).expect("older"));
        assert_eq!(store.get("root-1").expect("get"), Some(newer.clone()));

        let restored = CleanlinessSnapshotStore::default();
        restored.load_from_db(&path).expect("restore");
        assert_eq!(restored.get("root-1").expect("get"), Some(newer));
    }
}
