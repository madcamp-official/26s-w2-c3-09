use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use file_engine_cli::db::{block_on, open_pool_at};
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ManagedRoot {
    pub root_id: String,
    pub root: String,
    pub display_name: String,
    pub enabled: bool,
    pub watch_on_startup: bool,
    pub last_seen_status: ManagedRootStatus,
    pub last_error: Option<String>,
    pub registered_unix_ms: i64,
    pub updated_unix_ms: i64,
    pub room_id: Option<String>,
    pub detached_room_id: Option<String>,
    pub room_binding_status: RoomBindingStatus,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RoomBindingStatus {
    Unbound,
    Active,
    Detached,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ManagedRootStatus {
    Ready,
    Missing,
    Error,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ManagedRootStatePatch {
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub watch_on_startup: Option<bool>,
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
        validate_no_overlaps(&loaded)?;

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
            ensure_not_overlapping(&roots, &root)?;
            roots.insert(root.root_id.clone(), root.clone());
        }

        self.persist(&root)?;
        Ok(root)
    }

    pub fn update_state(
        &self,
        root_id: &str,
        patch: ManagedRootStatePatch,
    ) -> Result<ManagedRoot, String> {
        let updated = {
            let mut roots = self
                .roots
                .lock()
                .map_err(|_| "managed root store lock poisoned".to_string())?;
            let root = roots
                .get_mut(root_id)
                .ok_or_else(|| format!("managed root is not registered: {root_id}"))?;

            if let Some(enabled) = patch.enabled {
                root.enabled = enabled;
            }
            if let Some(watch_on_startup) = patch.watch_on_startup {
                root.watch_on_startup = watch_on_startup;
            }
            root.updated_unix_ms = unix_ms();
            root.clone()
        };

        self.persist(&updated)?;
        Ok(updated)
    }

    pub fn update_health(
        &self,
        root_id: &str,
        status: ManagedRootStatus,
        last_error: Option<String>,
    ) -> Result<ManagedRoot, String> {
        let updated = {
            let mut roots = self
                .roots
                .lock()
                .map_err(|_| "managed root store lock poisoned".to_string())?;
            let root = roots
                .get_mut(root_id)
                .ok_or_else(|| format!("managed root is not registered: {root_id}"))?;

            root.last_seen_status = status;
            root.last_error = last_error;
            root.updated_unix_ms = unix_ms();
            root.clone()
        };

        self.persist(&updated)?;
        Ok(updated)
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

    pub fn find_by_room(&self, room_id: &str) -> Result<Option<ManagedRoot>, String> {
        let roots = self
            .roots
            .lock()
            .map_err(|_| "managed root store lock poisoned".to_string())?;
        Ok(roots
            .values()
            .find(|root| {
                root.room_id.as_deref() == Some(room_id)
                    || (root.room_binding_status == RoomBindingStatus::Detached
                        && root.detached_room_id.as_deref() == Some(room_id))
            })
            .cloned())
    }

    pub fn bind_room(&self, root_id: &str, room_id: String) -> Result<ManagedRoot, String> {
        if room_id.trim().is_empty() {
            return Err("room binding id cannot be empty".to_string());
        }
        let updated = {
            let mut roots = self
                .roots
                .lock()
                .map_err(|_| "managed root store lock poisoned".to_string())?;
            if roots
                .values()
                .any(|root| root.root_id != root_id && root.room_id.as_deref() == Some(&room_id))
            {
                return Err(format!(
                    "room is already bound to another managed root: {room_id}"
                ));
            }
            let root = roots
                .get_mut(root_id)
                .ok_or_else(|| format!("managed root is not registered: {root_id}"))?;
            root.room_id = Some(room_id);
            root.detached_room_id = None;
            root.room_binding_status = RoomBindingStatus::Active;
            root.updated_unix_ms = unix_ms();
            root.clone()
        };
        self.persist(&updated)?;
        Ok(updated)
    }

    /// Verifies that a remote room still owns this root. Legacy unbound rows are adopted once,
    /// while an explicitly detached row can only be rebound through a new pairing/sync action.
    pub fn ensure_active_room_binding(
        &self,
        root_id: &str,
        room_id: &str,
    ) -> Result<ManagedRoot, String> {
        let root = self.get(root_id)?;
        match (&root.room_binding_status, root.room_id.as_deref()) {
            (RoomBindingStatus::Active, Some(bound_room)) if bound_room == room_id => Ok(root),
            (RoomBindingStatus::Unbound, None) => self.bind_room(root_id, room_id.to_string()),
            (RoomBindingStatus::Detached, _) => Err(format!(
                "managed root is detached from mobile access: {root_id}"
            )),
            (_, Some(bound_room)) => Err(format!(
                "managed root belongs to a different room: root={root_id}, expected={bound_room}, actual={room_id}"
            )),
            _ => Err(format!(
                "managed root has an invalid room binding state: {root_id}"
            )),
        }
    }

    pub fn detach_room(&self, room_id: &str) -> Result<Option<ManagedRoot>, String> {
        let updated = {
            let mut roots = self
                .roots
                .lock()
                .map_err(|_| "managed root store lock poisoned".to_string())?;
            let Some(root) = roots.values_mut().find(|root| {
                root.room_id.as_deref() == Some(room_id)
                    || (root.room_binding_status == RoomBindingStatus::Detached
                        && root.detached_room_id.as_deref() == Some(room_id))
            }) else {
                return Ok(None);
            };
            if let Some(active_room_id) = root.room_id.take() {
                root.detached_room_id = Some(active_room_id);
            }
            root.room_binding_status = RoomBindingStatus::Detached;
            root.updated_unix_ms = unix_ms();
            root.clone()
        };
        self.persist(&updated)?;
        Ok(Some(updated))
    }

    pub fn detach_all_rooms(&self) -> Result<Vec<ManagedRoot>, String> {
        let updated = {
            let mut roots = self
                .roots
                .lock()
                .map_err(|_| "managed root store lock poisoned".to_string())?;
            roots
                .values_mut()
                .filter(|root| {
                    root.room_id.is_some() && root.room_binding_status == RoomBindingStatus::Active
                })
                .map(|root| {
                    root.detached_room_id = root.room_id.take();
                    root.room_binding_status = RoomBindingStatus::Detached;
                    root.updated_unix_ms = unix_ms();
                    root.clone()
                })
                .collect::<Vec<_>>()
        };
        for root in &updated {
            self.persist(root)?;
        }
        Ok(updated)
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

fn validate_no_overlaps(roots: &[ManagedRoot]) -> Result<(), String> {
    for (index, candidate) in roots.iter().enumerate() {
        let existing = roots
            .iter()
            .enumerate()
            .filter(|(other_index, _)| *other_index != index)
            .map(|(_, root)| root);
        ensure_not_overlapping_iter(existing, candidate)?;
    }

    Ok(())
}

fn ensure_not_overlapping(
    roots: &BTreeMap<String, ManagedRoot>,
    candidate: &ManagedRoot,
) -> Result<(), String> {
    ensure_not_overlapping_iter(roots.values(), candidate)
}

fn ensure_not_overlapping_iter<'a>(
    roots: impl Iterator<Item = &'a ManagedRoot>,
    candidate: &ManagedRoot,
) -> Result<(), String> {
    let candidate_path = Path::new(&candidate.root);

    for existing in roots {
        if existing.root_id == candidate.root_id {
            continue;
        }

        let existing_path = Path::new(&existing.root);
        if existing_path == candidate_path {
            return Err(format!(
                "managed root is already registered: {}",
                candidate_path.display()
            ));
        }

        if existing_path.starts_with(candidate_path) {
            return Err(format!(
                "managed root overlaps an existing registered child root: candidate={}, existing={}",
                candidate_path.display(),
                existing_path.display()
            ));
        }

        if candidate_path.starts_with(existing_path) {
            return Err(format!(
                "managed root overlaps an existing registered parent root: candidate={}, existing={}",
                candidate_path.display(),
                existing_path.display()
            ));
        }
    }

    Ok(())
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

    add_column_if_missing(
        pool,
        "enabled",
        "ALTER TABLE managed_roots ADD COLUMN enabled INTEGER NOT NULL DEFAULT 1",
    )
    .await?;
    add_column_if_missing(
        pool,
        "room_id",
        "ALTER TABLE managed_roots ADD COLUMN room_id TEXT",
    )
    .await?;
    add_column_if_missing(
        pool,
        "detached_room_id",
        "ALTER TABLE managed_roots ADD COLUMN detached_room_id TEXT",
    )
    .await?;
    add_column_if_missing(
        pool,
        "room_binding_status",
        "ALTER TABLE managed_roots ADD COLUMN room_binding_status TEXT NOT NULL DEFAULT 'unbound'",
    )
    .await?;
    add_column_if_missing(
        pool,
        "watch_on_startup",
        "ALTER TABLE managed_roots ADD COLUMN watch_on_startup INTEGER NOT NULL DEFAULT 1",
    )
    .await?;
    add_column_if_missing(
        pool,
        "last_seen_status",
        "ALTER TABLE managed_roots ADD COLUMN last_seen_status TEXT NOT NULL DEFAULT 'ready'",
    )
    .await?;
    add_column_if_missing(
        pool,
        "last_error",
        "ALTER TABLE managed_roots ADD COLUMN last_error TEXT",
    )
    .await?;
    add_column_if_missing(
        pool,
        "registered_unix_ms",
        "ALTER TABLE managed_roots ADD COLUMN registered_unix_ms INTEGER NOT NULL DEFAULT 0",
    )
    .await?;
    add_column_if_missing(
        pool,
        "updated_unix_ms",
        "ALTER TABLE managed_roots ADD COLUMN updated_unix_ms INTEGER NOT NULL DEFAULT 0",
    )
    .await?;
    sqlx::query(
        "UPDATE managed_roots
         SET detached_room_id = COALESCE(detached_room_id, room_id), room_id = NULL
         WHERE room_binding_status = 'detached' AND room_id IS NOT NULL",
    )
    .execute(pool)
    .await
    .map_err(|error| format!("cannot normalize detached room tombstones: {error}"))?;

    Ok(())
}

async fn read_roots(pool: &SqlitePool) -> Result<Vec<ManagedRoot>, String> {
    let rows = sqlx::query(
        "SELECT root_id, canonical_path, display_name, enabled, watch_on_startup,
                last_seen_status, last_error, registered_unix_ms, updated_unix_ms,
                room_id, detached_room_id, room_binding_status
         FROM managed_roots",
    )
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
            enabled: int_bool(row.try_get("enabled").unwrap_or(1)),
            watch_on_startup: int_bool(row.try_get("watch_on_startup").unwrap_or(1)),
            last_seen_status: ManagedRootStatus::from_db_str(
                row.try_get::<String, _>("last_seen_status")
                    .unwrap_or_else(|_| "ready".to_string())
                    .as_str(),
            ),
            last_error: row.try_get("last_error").unwrap_or(None),
            registered_unix_ms: row.try_get("registered_unix_ms").unwrap_or(0),
            updated_unix_ms: row.try_get("updated_unix_ms").unwrap_or(0),
            room_id: row.try_get("room_id").unwrap_or(None),
            detached_room_id: row.try_get("detached_room_id").unwrap_or(None),
            room_binding_status: RoomBindingStatus::from_db_str(
                row.try_get::<String, _>("room_binding_status")
                    .unwrap_or_else(|_| "unbound".to_string())
                    .as_str(),
            ),
        });
    }

    Ok(roots)
}

async fn upsert_root(pool: &SqlitePool, root: &ManagedRoot) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO managed_roots (
             root_id, canonical_path, display_name, enabled, watch_on_startup,
             last_seen_status, last_error, registered_unix_ms, updated_unix_ms,
             room_id, detached_room_id, room_binding_status
         )
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(root_id) DO UPDATE SET
             canonical_path = excluded.canonical_path,
             display_name = excluded.display_name,
             enabled = excluded.enabled,
             watch_on_startup = excluded.watch_on_startup,
             last_seen_status = excluded.last_seen_status,
             last_error = excluded.last_error,
             registered_unix_ms = excluded.registered_unix_ms,
             updated_unix_ms = excluded.updated_unix_ms,
             room_id = excluded.room_id,
             detached_room_id = excluded.detached_room_id,
             room_binding_status = excluded.room_binding_status",
    )
    .bind(&root.root_id)
    .bind(&root.root)
    .bind(&root.display_name)
    .bind(bool_int(root.enabled))
    .bind(bool_int(root.watch_on_startup))
    .bind(root.last_seen_status.as_db_str())
    .bind(&root.last_error)
    .bind(root.registered_unix_ms)
    .bind(root.updated_unix_ms)
    .bind(&root.room_id)
    .bind(&root.detached_room_id)
    .bind(root.room_binding_status.as_db_str())
    .execute(pool)
    .await
    .map(|_| ())
    .map_err(|error| format!("cannot persist managed root: {error}"))
}

async fn add_column_if_missing(
    pool: &SqlitePool,
    column: &str,
    alter_sql: &'static str,
) -> Result<(), String> {
    let rows = sqlx::query("PRAGMA table_info(managed_roots)")
        .fetch_all(pool)
        .await
        .map_err(|error| format!("cannot inspect managed_roots table: {error}"))?;
    let exists = rows.iter().any(|row| {
        row.try_get::<String, _>("name")
            .is_ok_and(|name| name == column)
    });

    if !exists {
        sqlx::query(alter_sql)
            .execute(pool)
            .await
            .map_err(|error| format!("cannot add managed_roots.{column}: {error}"))?;
    }

    Ok(())
}

impl ManagedRoot {
    pub fn new(root_id: String, root: String, display_name: String) -> Self {
        let now = unix_ms();
        Self {
            root_id,
            root,
            display_name,
            enabled: true,
            watch_on_startup: true,
            last_seen_status: ManagedRootStatus::Ready,
            last_error: None,
            registered_unix_ms: now,
            updated_unix_ms: now,
            room_id: None,
            detached_room_id: None,
            room_binding_status: RoomBindingStatus::Unbound,
        }
    }
}

impl ManagedRootStatus {
    fn as_db_str(&self) -> &'static str {
        match self {
            ManagedRootStatus::Ready => "ready",
            ManagedRootStatus::Missing => "missing",
            ManagedRootStatus::Error => "error",
        }
    }

    fn from_db_str(value: &str) -> Self {
        match value {
            "missing" => ManagedRootStatus::Missing,
            "error" => ManagedRootStatus::Error,
            _ => ManagedRootStatus::Ready,
        }
    }
}

impl RoomBindingStatus {
    fn as_db_str(&self) -> &'static str {
        match self {
            RoomBindingStatus::Unbound => "unbound",
            RoomBindingStatus::Active => "active",
            RoomBindingStatus::Detached => "detached",
        }
    }

    fn from_db_str(value: &str) -> Self {
        match value {
            "active" => RoomBindingStatus::Active,
            "detached" => RoomBindingStatus::Detached,
            _ => RoomBindingStatus::Unbound,
        }
    }
}

fn bool_int(value: bool) -> i64 {
    if value {
        1
    } else {
        0
    }
}

fn int_bool(value: i64) -> bool {
    value != 0
}

fn unix_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{ManagedRoot, ManagedRootStatePatch, ManagedRootStore};

    fn root(id: &str, path: &str, name: &str) -> ManagedRoot {
        ManagedRoot::new(id.to_string(), path.to_string(), name.to_string())
    }

    #[test]
    fn upsert_keeps_one_entry_per_root_id() {
        let store = ManagedRootStore::default();

        store
            .upsert(root("root:cafe", "C:/work", "work"))
            .expect("insert root");
        store
            .upsert(root("root:cafe", "C:/work", "renamed"))
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
            .upsert(root("root:cafe", "C:/work", "work"))
            .expect("insert root");

        // A fresh store loading the same database sees the persisted root.
        let reloaded = ManagedRootStore::default();
        reloaded.load_from_db(&path).expect("reload database");
        let roots = reloaded.list().expect("list roots");

        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].root, "C:/work");
        assert!(roots[0].enabled);
        assert!(roots[0].watch_on_startup);
    }

    #[test]
    fn load_from_database_restores_roots() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("managed-roots.db");

        let store = ManagedRootStore::default();
        store.load_from_db(&path).expect("configure database");
        store
            .upsert(root("root:cafe", "C:/work", "work"))
            .expect("insert root");

        let reloaded = ManagedRootStore::default();
        reloaded.load_from_db(&path).expect("load roots");

        assert!(reloaded.contains_root("C:/work").expect("contains root"));
        assert_eq!(
            reloaded.get("root:cafe").expect("get root").display_name,
            "work"
        );
    }

    #[test]
    fn update_state_persists_runtime_flags() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("managed-roots.db");
        let store = ManagedRootStore::default();
        store.load_from_db(&path).expect("configure database");
        store
            .upsert(root("root:cafe", "C:/work", "work"))
            .expect("insert root");

        let updated = store
            .update_state(
                "root:cafe",
                ManagedRootStatePatch {
                    enabled: Some(false),
                    watch_on_startup: Some(false),
                },
            )
            .expect("update state");

        assert!(!updated.enabled);
        assert!(!updated.watch_on_startup);

        let reloaded = ManagedRootStore::default();
        reloaded.load_from_db(&path).expect("reload database");
        let root = reloaded.get("root:cafe").expect("get root");
        assert!(!root.enabled);
        assert!(!root.watch_on_startup);
    }

    #[test]
    fn update_health_persists_last_status_and_error() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("managed-roots.db");
        let store = ManagedRootStore::default();
        store.load_from_db(&path).expect("configure database");
        store
            .upsert(root("root:cafe", "C:/work", "work"))
            .expect("insert root");

        store
            .update_health(
                "root:cafe",
                super::ManagedRootStatus::Error,
                Some("watcher failed".to_string()),
            )
            .expect("update health");

        let reloaded = ManagedRootStore::default();
        reloaded.load_from_db(&path).expect("reload database");
        let root = reloaded.get("root:cafe").expect("get root");
        assert_eq!(root.last_seen_status, super::ManagedRootStatus::Error);
        assert_eq!(root.last_error, Some("watcher failed".to_string()));
    }

    #[test]
    fn room_binding_and_detached_tombstone_survive_restart() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("managed-roots.db");
        let store = ManagedRootStore::default();
        store.load_from_db(&path).expect("configure database");
        store
            .upsert(root("root:cafe", "C:/work", "work"))
            .expect("insert root");

        let bound = store
            .bind_room("root:cafe", "room-1".to_string())
            .expect("bind room");
        assert_eq!(bound.room_id.as_deref(), Some("room-1"));
        assert_eq!(bound.room_binding_status, super::RoomBindingStatus::Active);
        let detached = store
            .detach_room("room-1")
            .expect("detach")
            .expect("bound root");
        assert_eq!(detached.room_id, None);
        assert_eq!(detached.detached_room_id.as_deref(), Some("room-1"));
        assert_eq!(
            detached.room_binding_status,
            super::RoomBindingStatus::Detached
        );

        let reloaded = ManagedRootStore::default();
        reloaded.load_from_db(&path).expect("reload");
        let root = reloaded.get("root:cafe").expect("root");
        assert_eq!(root.room_id, None);
        assert_eq!(root.detached_room_id.as_deref(), Some("room-1"));
        assert_eq!(root.room_binding_status, super::RoomBindingStatus::Detached);
        assert!(reloaded
            .ensure_active_room_binding("root:cafe", "room-1")
            .expect_err("detached root stays blocked")
            .contains("detached"));
    }

    #[test]
    fn upsert_rejects_child_root_overlap() {
        let store = ManagedRootStore::default();

        store
            .upsert(root("root:parent", "C:/work", "work"))
            .expect("insert parent root");

        let error = store
            .upsert(root("root:child", "C:/work/project", "project"))
            .expect_err("reject child root");

        assert!(error.contains("parent root"));
    }

    #[test]
    fn upsert_rejects_parent_root_overlap() {
        let store = ManagedRootStore::default();

        store
            .upsert(root("root:child", "C:/work/project", "project"))
            .expect("insert child root");

        let error = store
            .upsert(root("root:parent", "C:/work", "work"))
            .expect_err("reject parent root");

        assert!(error.contains("child root"));
    }

    #[test]
    fn load_from_database_rejects_overlapping_roots() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("managed-roots.db");
        let store = ManagedRootStore::default();
        store.load_from_db(&path).expect("configure database");
        store
            .upsert(root("root:parent", "C:/work", "work"))
            .expect("insert parent root");

        let pool = file_engine_cli::db::open_pool_at(&path).expect("open db");
        file_engine_cli::db::block_on(async {
            sqlx::query(
                "INSERT INTO managed_roots (root_id, canonical_path, display_name)
                 VALUES ('root:child', 'C:/work/project', 'project')",
            )
            .execute(&pool)
            .await
            .expect("insert overlapping row");
        });

        let reloaded = ManagedRootStore::default();
        let error = reloaded
            .load_from_db(&path)
            .expect_err("reject overlapping database state");

        assert!(error.contains("overlaps"));
    }
}
