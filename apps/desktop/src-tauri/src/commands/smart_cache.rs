use crate::storage::smart_cache::{
    SmartCacheCandidate, SmartCacheFilePreference, SmartCacheFilePreferencePatch, SmartCacheStore,
    SmartCacheUsageEvent, SmartCacheUsageEventDraft,
};

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub fn record_smart_cache_usage_event(
    draft: SmartCacheUsageEventDraft,
    store: tauri::State<'_, SmartCacheStore>,
) -> Result<SmartCacheUsageEvent, String> {
    store.record_usage_event(draft)
}

#[cfg(not(feature = "tauri-commands"))]
pub fn record_smart_cache_usage_event(
    draft: SmartCacheUsageEventDraft,
    store: &SmartCacheStore,
) -> Result<SmartCacheUsageEvent, String> {
    store.record_usage_event(draft)
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub fn update_smart_cache_file_preference(
    root_id: String,
    relative_path: String,
    patch: SmartCacheFilePreferencePatch,
    store: tauri::State<'_, SmartCacheStore>,
) -> Result<SmartCacheFilePreference, String> {
    store.update_file_preference(root_id, relative_path, patch)
}

#[cfg(not(feature = "tauri-commands"))]
pub fn update_smart_cache_file_preference(
    root_id: String,
    relative_path: String,
    patch: SmartCacheFilePreferencePatch,
    store: &SmartCacheStore,
) -> Result<SmartCacheFilePreference, String> {
    store.update_file_preference(root_id, relative_path, patch)
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub fn list_smart_cache_candidates(
    root_id: String,
    limit: Option<i64>,
    store: tauri::State<'_, SmartCacheStore>,
) -> Result<Vec<SmartCacheCandidate>, String> {
    store.list_candidates(root_id, limit.unwrap_or(25))
}

#[cfg(not(feature = "tauri-commands"))]
pub fn list_smart_cache_candidates(
    root_id: String,
    limit: Option<i64>,
    store: &SmartCacheStore,
) -> Result<Vec<SmartCacheCandidate>, String> {
    store.list_candidates(root_id, limit.unwrap_or(25))
}
