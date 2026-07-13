use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use file_engine_cli::db::{block_on, open_pool_at};
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};

const CACHE_CANDIDATE_LIMIT_MAX: i64 = 200;

#[derive(Debug, Default)]
pub struct SmartCacheStore {
    pool: Mutex<Option<SqlitePool>>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SmartCacheUsageEventKind {
    Browse,
    Preview,
    Download,
    ProposalCandidate,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SmartCacheUsageEventDraft {
    pub root_id: String,
    pub relative_path: String,
    pub kind: SmartCacheUsageEventKind,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SmartCacheUsageEvent {
    pub id: i64,
    pub root_id: String,
    pub relative_path: String,
    pub kind: SmartCacheUsageEventKind,
    pub created_unix_ms: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SmartCacheFilePreferencePatch {
    #[serde(default)]
    pub pinned: Option<bool>,
    #[serde(default)]
    pub excluded: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SmartCacheFilePreference {
    pub root_id: String,
    pub relative_path: String,
    pub pinned: bool,
    pub excluded: bool,
    pub updated_unix_ms: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SmartCacheCandidate {
    pub root_id: String,
    pub relative_path: String,
    pub score: i64,
    pub event_count: i64,
    pub last_used_unix_ms: i64,
    pub pinned: bool,
}

impl SmartCacheStore {
    pub fn load_from_db(&self, path: impl AsRef<Path>) -> Result<(), String> {
        let pool = open_pool_at(path.as_ref()).map_err(|error| error.to_string())?;
        block_on(migrate(&pool))?;
        *self
            .pool
            .lock()
            .map_err(|_| "smart cache pool lock poisoned".to_string())? = Some(pool);
        Ok(())
    }

    pub fn record_usage_event(
        &self,
        draft: SmartCacheUsageEventDraft,
    ) -> Result<SmartCacheUsageEvent, String> {
        self.record_usage_event_at(draft, unix_ms())
    }

    pub fn update_file_preference(
        &self,
        root_id: String,
        relative_path: String,
        patch: SmartCacheFilePreferencePatch,
    ) -> Result<SmartCacheFilePreference, String> {
        validate_cache_path(&relative_path)?;
        let pool = self.required_pool()?;
        let now = unix_ms();
        block_on(async move {
            sqlx::query(
                "INSERT INTO smart_cache_file_preferences
                    (root_id, relative_path, pinned, excluded, updated_unix_ms)
                 VALUES (?1, ?2, COALESCE(?3, 0), COALESCE(?4, 0), ?5)
                 ON CONFLICT(root_id, relative_path) DO UPDATE SET
                    pinned = COALESCE(?3, pinned),
                    excluded = COALESCE(?4, excluded),
                    updated_unix_ms = ?5",
            )
            .bind(&root_id)
            .bind(&relative_path)
            .bind(patch.pinned.map(bool_to_i64))
            .bind(patch.excluded.map(bool_to_i64))
            .bind(now)
            .execute(&pool)
            .await
            .map_err(|error| format!("cannot update smart cache preference: {error}"))?;

            let row = sqlx::query(
                "SELECT root_id, relative_path, pinned, excluded, updated_unix_ms
                 FROM smart_cache_file_preferences
                 WHERE root_id = ?1 AND relative_path = ?2",
            )
            .bind(&root_id)
            .bind(&relative_path)
            .fetch_one(&pool)
            .await
            .map_err(|error| format!("cannot read smart cache preference: {error}"))?;
            preference_from_row(row)
        })
    }

    pub fn list_candidates(
        &self,
        root_id: String,
        limit: i64,
    ) -> Result<Vec<SmartCacheCandidate>, String> {
        let pool = self.required_pool()?;
        let limit = limit.clamp(1, CACHE_CANDIDATE_LIMIT_MAX);
        block_on(async move {
            let rows = sqlx::query(
                "WITH usage AS (
                    SELECT
                        root_id,
                        relative_path,
                        COUNT(*) AS event_count,
                        MAX(created_unix_ms) AS last_used_unix_ms,
                        SUM(CASE kind
                            WHEN 'browse' THEN 1
                            WHEN 'preview' THEN 2
                            WHEN 'download' THEN 8
                            WHEN 'proposal_candidate' THEN 3
                            ELSE 0
                        END) AS usage_score
                    FROM smart_cache_usage_events
                    WHERE root_id = ?1
                    GROUP BY root_id, relative_path
                 )
                 SELECT
                    usage.root_id,
                    usage.relative_path,
                    usage.event_count,
                    usage.last_used_unix_ms,
                    COALESCE(pref.pinned, 0) AS pinned,
                    COALESCE(pref.excluded, 0) AS excluded,
                    usage.usage_score + CASE COALESCE(pref.pinned, 0)
                        WHEN 1 THEN 1000
                        ELSE 0
                    END AS score
                 FROM usage
                 LEFT JOIN smart_cache_file_preferences pref
                    ON pref.root_id = usage.root_id
                    AND pref.relative_path = usage.relative_path
                 WHERE COALESCE(pref.excluded, 0) = 0
                 ORDER BY score DESC, last_used_unix_ms DESC, usage.relative_path ASC
                 LIMIT ?2",
            )
            .bind(&root_id)
            .bind(limit)
            .fetch_all(&pool)
            .await
            .map_err(|error| format!("cannot list smart cache candidates: {error}"))?;

            rows.into_iter()
                .map(|row| {
                    Ok(SmartCacheCandidate {
                        root_id: row.try_get::<String, _>("root_id").map_err(err)?,
                        relative_path: row.try_get::<String, _>("relative_path").map_err(err)?,
                        score: row.try_get::<i64, _>("score").map_err(err)?,
                        event_count: row.try_get::<i64, _>("event_count").map_err(err)?,
                        last_used_unix_ms: row
                            .try_get::<i64, _>("last_used_unix_ms")
                            .map_err(err)?,
                        pinned: row.try_get::<i64, _>("pinned").map_err(err)? == 1,
                    })
                })
                .collect::<Result<Vec<_>, String>>()
        })
    }

    fn record_usage_event_at(
        &self,
        draft: SmartCacheUsageEventDraft,
        created_unix_ms: i64,
    ) -> Result<SmartCacheUsageEvent, String> {
        validate_cache_path(&draft.relative_path)?;
        let pool = self.required_pool()?;
        let kind = kind_to_str(&draft.kind).to_string();
        block_on(async move {
            let result = sqlx::query(
                "INSERT INTO smart_cache_usage_events
                    (root_id, relative_path, kind, created_unix_ms)
                 VALUES (?1, ?2, ?3, ?4)",
            )
            .bind(&draft.root_id)
            .bind(&draft.relative_path)
            .bind(&kind)
            .bind(created_unix_ms)
            .execute(&pool)
            .await
            .map_err(|error| format!("cannot record smart cache usage event: {error}"))?;
            Ok(SmartCacheUsageEvent {
                id: result.last_insert_rowid(),
                root_id: draft.root_id,
                relative_path: draft.relative_path,
                kind: draft.kind,
                created_unix_ms,
            })
        })
    }

    fn required_pool(&self) -> Result<SqlitePool, String> {
        self.pool()?
            .ok_or_else(|| "smart cache database is not initialized".to_string())
    }

    fn pool(&self) -> Result<Option<SqlitePool>, String> {
        Ok(self
            .pool
            .lock()
            .map_err(|_| "smart cache pool lock poisoned".to_string())?
            .clone())
    }
}

async fn migrate(pool: &SqlitePool) -> Result<(), String> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS smart_cache_usage_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            root_id TEXT NOT NULL,
            relative_path TEXT NOT NULL,
            kind TEXT NOT NULL,
            created_unix_ms INTEGER NOT NULL
        )",
    )
    .execute(pool)
    .await
    .map_err(|error| format!("cannot migrate smart cache usage events: {error}"))?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_smart_cache_usage_root_path
         ON smart_cache_usage_events(root_id, relative_path, created_unix_ms)",
    )
    .execute(pool)
    .await
    .map_err(|error| format!("cannot index smart cache usage events: {error}"))?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS smart_cache_file_preferences (
            root_id TEXT NOT NULL,
            relative_path TEXT NOT NULL,
            pinned INTEGER NOT NULL DEFAULT 0,
            excluded INTEGER NOT NULL DEFAULT 0,
            updated_unix_ms INTEGER NOT NULL,
            PRIMARY KEY(root_id, relative_path)
        )",
    )
    .execute(pool)
    .await
    .map_err(|error| format!("cannot migrate smart cache preferences: {error}"))?;

    Ok(())
}

fn validate_cache_path(relative_path: &str) -> Result<(), String> {
    if relative_path.trim().is_empty()
        || relative_path.starts_with('/')
        || relative_path.starts_with('\\')
        || relative_path.contains(':')
        || relative_path
            .split(['/', '\\'])
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err("smart cache path must be a safe relative file path".to_string());
    }

    let lower = relative_path.replace('\\', "/").to_ascii_lowercase();
    if lower.contains("/.housemouse/")
        || lower.starts_with(".housemouse/")
        || lower.contains("/.housemouse_trash/")
        || lower.starts_with(".housemouse_trash/")
        || lower.ends_with(".tmp")
        || lower.ends_with(".lock")
        || lower.ends_with(".key")
        || lower.ends_with(".pem")
        || lower.ends_with(".p12")
        || lower.ends_with(".pfx")
        || lower.contains("credential")
        || lower.contains("secret")
        || lower.contains("token")
    {
        return Err("smart cache path is excluded by local cache safety policy".to_string());
    }

    Ok(())
}

fn kind_to_str(kind: &SmartCacheUsageEventKind) -> &'static str {
    match kind {
        SmartCacheUsageEventKind::Browse => "browse",
        SmartCacheUsageEventKind::Preview => "preview",
        SmartCacheUsageEventKind::Download => "download",
        SmartCacheUsageEventKind::ProposalCandidate => "proposal_candidate",
    }
}

fn bool_to_i64(value: bool) -> i64 {
    if value {
        1
    } else {
        0
    }
}

fn preference_from_row(row: sqlx::sqlite::SqliteRow) -> Result<SmartCacheFilePreference, String> {
    Ok(SmartCacheFilePreference {
        root_id: row.try_get::<String, _>("root_id").map_err(err)?,
        relative_path: row.try_get::<String, _>("relative_path").map_err(err)?,
        pinned: row.try_get::<i64, _>("pinned").map_err(err)? == 1,
        excluded: row.try_get::<i64, _>("excluded").map_err(err)? == 1,
        updated_unix_ms: row.try_get::<i64, _>("updated_unix_ms").map_err(err)?,
    })
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
    use super::{
        SmartCacheFilePreferencePatch, SmartCacheStore, SmartCacheUsageEventDraft,
        SmartCacheUsageEventKind,
    };

    fn store() -> (tempfile::TempDir, SmartCacheStore) {
        let temp = tempfile::tempdir().expect("temp dir");
        let store = SmartCacheStore::default();
        store
            .load_from_db(temp.path().join("smart-cache.db"))
            .expect("load smart cache store");
        (temp, store)
    }

    #[test]
    fn usage_events_rank_downloads_above_browses() {
        let (_temp, store) = store();
        store
            .record_usage_event_at(
                SmartCacheUsageEventDraft {
                    root_id: "root-1".to_string(),
                    relative_path: "docs/readme.md".to_string(),
                    kind: SmartCacheUsageEventKind::Browse,
                },
                100,
            )
            .expect("browse");
        store
            .record_usage_event_at(
                SmartCacheUsageEventDraft {
                    root_id: "root-1".to_string(),
                    relative_path: "media/demo.mp4".to_string(),
                    kind: SmartCacheUsageEventKind::Download,
                },
                90,
            )
            .expect("download");

        let candidates = store
            .list_candidates("root-1".to_string(), 10)
            .expect("candidates");

        assert_eq!(candidates[0].relative_path, "media/demo.mp4");
        assert_eq!(candidates[0].score, 8);
        assert_eq!(candidates[1].relative_path, "docs/readme.md");
    }

    #[test]
    fn pinned_files_outrank_usage_and_excluded_files_are_removed() {
        let (_temp, store) = store();
        for _ in 0..5 {
            store
                .record_usage_event(SmartCacheUsageEventDraft {
                    root_id: "root-1".to_string(),
                    relative_path: "hot/report.pdf".to_string(),
                    kind: SmartCacheUsageEventKind::Download,
                })
                .expect("hot event");
        }
        store
            .record_usage_event(SmartCacheUsageEventDraft {
                root_id: "root-1".to_string(),
                relative_path: "pinned/notes.md".to_string(),
                kind: SmartCacheUsageEventKind::Browse,
            })
            .expect("pinned event");
        store
            .record_usage_event(SmartCacheUsageEventDraft {
                root_id: "root-1".to_string(),
                relative_path: "excluded/private.pdf".to_string(),
                kind: SmartCacheUsageEventKind::Download,
            })
            .expect("excluded event");

        store
            .update_file_preference(
                "root-1".to_string(),
                "pinned/notes.md".to_string(),
                SmartCacheFilePreferencePatch {
                    pinned: Some(true),
                    excluded: None,
                },
            )
            .expect("pin");
        store
            .update_file_preference(
                "root-1".to_string(),
                "excluded/private.pdf".to_string(),
                SmartCacheFilePreferencePatch {
                    pinned: None,
                    excluded: Some(true),
                },
            )
            .expect("exclude");

        let candidates = store
            .list_candidates("root-1".to_string(), 10)
            .expect("candidates");

        assert_eq!(candidates[0].relative_path, "pinned/notes.md");
        assert!(candidates[0].pinned);
        assert!(!candidates
            .iter()
            .any(|candidate| candidate.relative_path == "excluded/private.pdf"));
    }

    #[test]
    fn unsafe_paths_are_rejected_before_becoming_cache_candidates() {
        let (_temp, store) = store();
        let error = store
            .record_usage_event(SmartCacheUsageEventDraft {
                root_id: "root-1".to_string(),
                relative_path: "../secret.pem".to_string(),
                kind: SmartCacheUsageEventKind::Download,
            })
            .expect_err("unsafe path rejected");

        assert!(error.contains("safe relative file path"));
    }

    #[test]
    fn credential_like_paths_are_excluded() {
        let (_temp, store) = store();
        let error = store
            .record_usage_event(SmartCacheUsageEventDraft {
                root_id: "root-1".to_string(),
                relative_path: "config/access-token.txt".to_string(),
                kind: SmartCacheUsageEventKind::Download,
            })
            .expect_err("credential path rejected");

        assert!(error.contains("cache safety policy"));
    }
}
