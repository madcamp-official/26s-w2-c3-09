use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Mutex;

use file_engine_cli::db::{block_on, open_pool_at};
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ManagedRoot {
    pub root_id: String,
    pub root: String,
    pub display_name: String,
}

/// In-memory registry of managed roots, durably backed by an app-level SQLite database once
/// `load_from_db` is called. The map stays authoritative for fast reads; the database mirrors
/// it so registrations survive restarts. A store with no database configured still works
/// entirely in memory (used in tests).
#[derive(Debug, Default)]
pub struct ManagedRootStore {
    roots: Mutex<BTreeMap<String, ManagedRoot>>,
    pool: Mutex<Option<SqlitePool>>,
}

impl ManagedRootStore {
    /// Opens (creating if needed) the managed-roots database at `path`, then loads its rows
    /// into memory. Replaces the previous `managed-roots.json` persistence.
    pub fn load_from_db(&self, path: impl AsRef<Path>) -> Result<(), String> {
        let pool = open_pool_at(path.as_ref()).map_err(|error| error.to_string())?;
        let loaded = block_on(async {
            migrate(&pool).await?;
            read_roots(&pool).await
        })?;

        {
            let mut stored_pool = self
                .pool
                .lock()
                .map_err(|_| "managed root pool lock poisoned".to_string())?;
            *stored_pool = Some(pool);
        }

        let mut stored_roots = self
            .roots
            .lock()
            .map_err(|_| "managed root store lock poisoned".to_string())?;
        *stored_roots = loaded
            .into_iter()
            .map(|root| (root.root_id.clone(), root))
            .collect();

        Ok(())
    }

    pub fn upsert(&self, root: ManagedRoot) -> Result<ManagedRoot, String> {
        {
            let mut roots = self
                .roots
                .lock()
                .map_err(|_| "managed root store lock poisoned".to_string())?;
            roots.insert(root.root_id.clone(), root.clone());
        }

        self.persist(&root)?;
        Ok(root)
    }

    pub fn list(&self) -> Result<Vec<ManagedRoot>, String> {
        let roots = self
            .roots
            .lock()
            .map_err(|_| "managed root store lock poisoned".to_string())?;

        Ok(roots.values().cloned().collect())
    }

    pub fn get(&self, root_id: &str) -> Result<ManagedRoot, String> {
        let roots = self
            .roots
            .lock()
            .map_err(|_| "managed root store lock poisoned".to_string())?;

        roots
            .get(root_id)
            .cloned()
            .ok_or_else(|| format!("managed root is not registered: {root_id}"))
    }

    pub fn contains_root(&self, root: &str) -> Result<bool, String> {
        let roots = self
            .roots
            .lock()
            .map_err(|_| "managed root store lock poisoned".to_string())?;

        Ok(roots.values().any(|managed| managed.root == root))
    }

    fn persist(&self, root: &ManagedRoot) -> Result<(), String> {
        let pool = self
            .pool
            .lock()
            .map_err(|_| "managed root pool lock poisoned".to_string())?
            .clone();

        if let Some(pool) = pool {
            block_on(async { upsert_root(&pool, root).await })?;
        }

        Ok(())
    }
}

async fn migrate(pool: &SqlitePool) -> Result<(), String> {
    // `canonical_path` is UNIQUE so the same folder cannot be registered under two ids, the
    // overlap invariant the plan calls for at the storage layer.
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS managed_roots (
            root_id         TEXT PRIMARY KEY,
            canonical_path  TEXT NOT NULL UNIQUE,
            display_name    TEXT NOT NULL
        )",
    )
    .execute(pool)
    .await
    .map_err(|error| format!("cannot create managed_roots table: {error}"))?;

    Ok(())
}

async fn read_roots(pool: &SqlitePool) -> Result<Vec<ManagedRoot>, String> {
    let rows = sqlx::query("SELECT root_id, canonical_path, display_name FROM managed_roots")
        .fetch_all(pool)
        .await
        .map_err(|error| format!("cannot read managed roots: {error}"))?;

    let mut roots = Vec::with_capacity(rows.len());
    for row in rows {
        roots.push(ManagedRoot {
            root_id: row
                .try_get("root_id")
                .map_err(|error| format!("cannot read managed root id: {error}"))?,
            root: row
                .try_get("canonical_path")
                .map_err(|error| format!("cannot read managed root path: {error}"))?,
            display_name: row
                .try_get("display_name")
                .map_err(|error| format!("cannot read managed root name: {error}"))?,
        });
    }

    Ok(roots)
}

async fn upsert_root(pool: &SqlitePool, root: &ManagedRoot) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO managed_roots (root_id, canonical_path, display_name)
         VALUES (?, ?, ?)
         ON CONFLICT(root_id) DO UPDATE SET
             canonical_path = excluded.canonical_path,
             display_name = excluded.display_name",
    )
    .bind(&root.root_id)
    .bind(&root.root)
    .bind(&root.display_name)
    .execute(pool)
    .await
    .map(|_| ())
    .map_err(|error| format!("cannot persist managed root: {error}"))
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{ManagedRoot, ManagedRootStore};

    #[test]
    fn upsert_keeps_one_entry_per_root_id() {
        let store = ManagedRootStore::default();

        store
            .upsert(ManagedRoot {
                root_id: "root:cafe".to_string(),
                root: "C:/work".to_string(),
                display_name: "work".to_string(),
            })
            .expect("insert root");
        store
            .upsert(ManagedRoot {
                root_id: "root:cafe".to_string(),
                root: "C:/work".to_string(),
                display_name: "renamed".to_string(),
            })
            .expect("update root");

        let roots = store.list().expect("list roots");

        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].display_name, "renamed");
    }

    #[test]
    fn upsert_persists_roots_after_loading_database() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("managed-roots.db");
        let store = ManagedRootStore::default();

        store.load_from_db(&path).expect("configure database");
        store
            .upsert(ManagedRoot {
                root_id: "root:cafe".to_string(),
                root: "C:/work".to_string(),
                display_name: "work".to_string(),
            })
            .expect("insert root");

        // A fresh store loading the same database sees the persisted root.
        let reloaded = ManagedRootStore::default();
        reloaded.load_from_db(&path).expect("reload database");
        let roots = reloaded.list().expect("list roots");

        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].root, "C:/work");
    }

    #[test]
    fn load_from_database_restores_roots() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("managed-roots.db");

        let store = ManagedRootStore::default();
        store.load_from_db(&path).expect("configure database");
        store
            .upsert(ManagedRoot {
                root_id: "root:cafe".to_string(),
                root: "C:/work".to_string(),
                display_name: "work".to_string(),
            })
            .expect("insert root");

        let reloaded = ManagedRootStore::default();
        reloaded.load_from_db(&path).expect("load roots");

        assert!(reloaded.contains_root("C:/work").expect("contains root"));
        assert_eq!(
            reloaded.get("root:cafe").expect("get root").display_name,
            "work"
        );
    }
}
