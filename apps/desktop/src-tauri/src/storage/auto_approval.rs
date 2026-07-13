use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use file_engine_cli::auto_approval::AutoApprovalPolicy;
use file_engine_cli::db::{block_on, open_pool_at};
use file_engine_cli::proposal::ProposalAction;
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct AutoApprovalPolicyRecord {
    pub root_id: String,
    pub enabled: bool,
    pub allowed_actions: Vec<ProposalAction>,
    pub max_files_per_run: usize,
    pub expires_unix_ms: Option<i64>,
    pub updated_unix_ms: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct AutoApprovalPolicyPatch {
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub allowed_actions: Option<Vec<ProposalAction>>,
    #[serde(default)]
    pub max_files_per_run: Option<usize>,
    #[serde(default)]
    pub expires_unix_ms: Option<i64>,
}

#[derive(Debug, Default)]
pub struct AutoApprovalStore {
    policies: Mutex<BTreeMap<String, AutoApprovalPolicyRecord>>,
    pool: Mutex<Option<SqlitePool>>,
}

impl AutoApprovalStore {
    pub fn load_from_db(&self, path: impl AsRef<Path>) -> Result<(), String> {
        let pool = open_pool_at(path.as_ref()).map_err(|error| error.to_string())?;
        let loaded = block_on(async {
            migrate(&pool).await?;
            read_policies(&pool).await
        })?;

        {
            let mut stored_pool = self
                .pool
                .lock()
                .map_err(|_| "auto approval pool lock poisoned".to_string())?;
            *stored_pool = Some(pool);
        }

        let mut policies = self
            .policies
            .lock()
            .map_err(|_| "auto approval store lock poisoned".to_string())?;
        *policies = loaded
            .into_iter()
            .map(|policy| (policy.root_id.clone(), policy))
            .collect();

        Ok(())
    }

    pub fn get_or_default(&self, root_id: &str) -> Result<AutoApprovalPolicyRecord, String> {
        let policies = self
            .policies
            .lock()
            .map_err(|_| "auto approval store lock poisoned".to_string())?;

        Ok(policies
            .get(root_id)
            .cloned()
            .unwrap_or_else(|| AutoApprovalPolicyRecord::default_for_root(root_id.to_string())))
    }

    pub fn patch(
        &self,
        root_id: &str,
        patch: AutoApprovalPolicyPatch,
    ) -> Result<AutoApprovalPolicyRecord, String> {
        let updated = {
            let mut policies = self
                .policies
                .lock()
                .map_err(|_| "auto approval store lock poisoned".to_string())?;
            let policy = policies
                .entry(root_id.to_string())
                .or_insert_with(|| AutoApprovalPolicyRecord::default_for_root(root_id.to_string()));

            if let Some(enabled) = patch.enabled {
                policy.enabled = enabled;
            }
            if let Some(allowed_actions) = patch.allowed_actions {
                if allowed_actions.is_empty() {
                    return Err("auto approval policy must allow at least one action".to_string());
                }
                policy.allowed_actions = allowed_actions;
            }
            if let Some(max_files_per_run) = patch.max_files_per_run {
                if max_files_per_run == 0 {
                    return Err(
                        "auto approval max_files_per_run must be greater than zero".to_string()
                    );
                }
                policy.max_files_per_run = max_files_per_run;
            }
            if patch.expires_unix_ms.is_some() {
                policy.expires_unix_ms = patch.expires_unix_ms;
            }
            policy.updated_unix_ms = unix_ms();
            policy.clone()
        };

        self.persist(&updated)?;
        Ok(updated)
    }

    /// Drops the auto-approval policy for a root from memory and disk. Called when a folder is
    /// unregistered so a later folder with the same generated id cannot inherit a stale policy.
    pub fn remove(&self, root_id: &str) -> Result<bool, String> {
        let removed = {
            let mut policies = self
                .policies
                .lock()
                .map_err(|_| "auto approval store lock poisoned".to_string())?;
            policies.remove(root_id).is_some()
        };

        self.delete_persisted(root_id)?;
        Ok(removed)
    }

    fn delete_persisted(&self, root_id: &str) -> Result<(), String> {
        let pool = self
            .pool
            .lock()
            .map_err(|_| "auto approval pool lock poisoned".to_string())?
            .clone();
        let Some(pool) = pool else {
            return Ok(());
        };

        let root_id = root_id.to_string();
        block_on(async move {
            sqlx::query("DELETE FROM auto_approval_policies WHERE root_id = ?1")
                .bind(root_id)
                .execute(&pool)
                .await
                .map(|_| ())
                .map_err(|error| format!("cannot delete auto approval policy: {error}"))
        })
    }

    fn persist(&self, policy: &AutoApprovalPolicyRecord) -> Result<(), String> {
        let pool = self
            .pool
            .lock()
            .map_err(|_| "auto approval pool lock poisoned".to_string())?
            .clone();
        let Some(pool) = pool else {
            return Ok(());
        };

        let allowed_actions_json = serde_json::to_string(&policy.allowed_actions)
            .map_err(|error| format!("cannot serialize auto approval actions: {error}"))?;
        let root_id = policy.root_id.clone();
        let enabled = bool_int(policy.enabled);
        let max_files_per_run = i64::try_from(policy.max_files_per_run).unwrap_or(i64::MAX);
        let expires_unix_ms = policy.expires_unix_ms;
        let updated_unix_ms = policy.updated_unix_ms;

        block_on(async move {
            sqlx::query(
                "INSERT INTO auto_approval_policies
                    (root_id, enabled, allowed_actions_json, max_files_per_run, expires_unix_ms, updated_unix_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(root_id) DO UPDATE SET
                    enabled = excluded.enabled,
                    allowed_actions_json = excluded.allowed_actions_json,
                    max_files_per_run = excluded.max_files_per_run,
                    expires_unix_ms = excluded.expires_unix_ms,
                    updated_unix_ms = excluded.updated_unix_ms",
            )
            .bind(root_id)
            .bind(enabled)
            .bind(allowed_actions_json)
            .bind(max_files_per_run)
            .bind(expires_unix_ms)
            .bind(updated_unix_ms)
            .execute(&pool)
            .await
            .map_err(|error| format!("cannot persist auto approval policy: {error}"))?;

            Ok(())
        })
    }
}

impl AutoApprovalPolicyRecord {
    pub fn default_for_root(root_id: String) -> Self {
        Self {
            root_id,
            enabled: false,
            allowed_actions: vec![ProposalAction::Trash],
            max_files_per_run: 20,
            expires_unix_ms: None,
            updated_unix_ms: unix_ms(),
        }
    }

    pub fn to_engine_policy(&self) -> AutoApprovalPolicy {
        AutoApprovalPolicy {
            enabled: self.enabled && !self.is_expired(),
            allowed_actions: self.allowed_actions.clone(),
            max_files_per_run: self.max_files_per_run,
        }
    }

    fn is_expired(&self) -> bool {
        self.expires_unix_ms
            .is_some_and(|expires_unix_ms| expires_unix_ms <= unix_ms())
    }
}

async fn migrate(pool: &SqlitePool) -> Result<(), String> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS auto_approval_policies (
            root_id TEXT PRIMARY KEY NOT NULL,
            enabled INTEGER NOT NULL,
            allowed_actions_json TEXT NOT NULL,
            max_files_per_run INTEGER NOT NULL,
            expires_unix_ms INTEGER,
            updated_unix_ms INTEGER NOT NULL
        )",
    )
    .execute(pool)
    .await
    .map_err(|error| format!("cannot migrate auto approval policies: {error}"))?;

    Ok(())
}

async fn read_policies(pool: &SqlitePool) -> Result<Vec<AutoApprovalPolicyRecord>, String> {
    let rows = sqlx::query(
        "SELECT root_id, enabled, allowed_actions_json, max_files_per_run, expires_unix_ms, updated_unix_ms
         FROM auto_approval_policies
         ORDER BY root_id",
    )
    .fetch_all(pool)
    .await
    .map_err(|error| format!("cannot read auto approval policies: {error}"))?;

    rows.into_iter()
        .map(|row| {
            let root_id = row
                .try_get::<String, _>("root_id")
                .map_err(|error| format!("cannot read auto approval root_id: {error}"))?;
            let allowed_actions_json = row
                .try_get::<String, _>("allowed_actions_json")
                .map_err(|error| format!("cannot read auto approval actions: {error}"))?;
            let allowed_actions =
                serde_json::from_str::<Vec<ProposalAction>>(&allowed_actions_json)
                    .map_err(|error| format!("cannot parse auto approval actions: {error}"))?;
            if allowed_actions.is_empty() {
                return Err(format!(
                    "auto approval policy has no allowed actions: {root_id}"
                ));
            }
            let max_files_per_run = row
                .try_get::<i64, _>("max_files_per_run")
                .map_err(|error| format!("cannot read auto approval limit: {error}"))?;
            if max_files_per_run <= 0 {
                return Err(format!(
                    "auto approval max_files_per_run must be positive: {root_id}"
                ));
            }

            Ok(AutoApprovalPolicyRecord {
                root_id,
                enabled: int_bool(
                    row.try_get::<i64, _>("enabled")
                        .map_err(|error| format!("cannot read auto approval enabled: {error}"))?,
                ),
                allowed_actions,
                max_files_per_run: usize::try_from(max_files_per_run).unwrap_or(usize::MAX),
                expires_unix_ms: row
                    .try_get::<Option<i64>, _>("expires_unix_ms")
                    .map_err(|error| format!("cannot read auto approval expiry: {error}"))?,
                updated_unix_ms: row
                    .try_get::<i64, _>("updated_unix_ms")
                    .map_err(|error| format!("cannot read auto approval update time: {error}"))?,
            })
        })
        .collect()
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

    use file_engine_cli::proposal::ProposalAction;

    use super::{AutoApprovalPolicyPatch, AutoApprovalStore};

    #[test]
    fn default_policy_is_disabled_but_trash_scoped() {
        let store = AutoApprovalStore::default();

        let policy = store.get_or_default("root:cafe").expect("policy");

        assert!(!policy.enabled);
        assert_eq!(policy.allowed_actions, vec![ProposalAction::Trash]);
        assert_eq!(policy.max_files_per_run, 20);
    }

    #[test]
    fn patch_persists_policy_after_reload() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("auto-approval.db");
        let store = AutoApprovalStore::default();
        store.load_from_db(&path).expect("load db");

        store
            .patch(
                "root:cafe",
                AutoApprovalPolicyPatch {
                    enabled: Some(true),
                    allowed_actions: Some(vec![ProposalAction::Trash, ProposalAction::Move]),
                    max_files_per_run: Some(3),
                    expires_unix_ms: None,
                },
            )
            .expect("patch policy");

        let reloaded = AutoApprovalStore::default();
        reloaded.load_from_db(&path).expect("reload db");
        let policy = reloaded.get_or_default("root:cafe").expect("policy");

        assert!(policy.enabled);
        assert_eq!(
            policy.allowed_actions,
            vec![ProposalAction::Trash, ProposalAction::Move]
        );
        assert_eq!(policy.max_files_per_run, 3);
    }
}
