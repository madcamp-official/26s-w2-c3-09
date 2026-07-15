use crate::storage::smart_cache::{
    SmartCacheCandidate, SmartCacheFilePreference, SmartCacheFilePreferencePatch, SmartCacheStore,
    SmartCacheUsageEvent, SmartCacheUsageEventDraft,
};
#[cfg(feature = "tauri-commands")]
use crate::work_limiter::WorkLimiter;
use crate::{
    agent::AgentRuntime, smart_cache_processor::SmartCacheProcessingReport,
    storage::managed_roots::ManagedRootStore, storage::outbox::OutboxStore,
};

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub fn record_smart_cache_usage_event(
    draft: SmartCacheUsageEventDraft,
    window: tauri::Window,
    store: tauri::State<'_, SmartCacheStore>,
) -> Result<SmartCacheUsageEvent, String> {
    crate::commands::permissions::require_file_manager_window(&window)?;
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
    window: tauri::Window,
    store: tauri::State<'_, SmartCacheStore>,
) -> Result<SmartCacheFilePreference, String> {
    crate::commands::permissions::require_file_manager_window(&window)?;
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
    window: tauri::Window,
    store: tauri::State<'_, SmartCacheStore>,
) -> Result<Vec<SmartCacheCandidate>, String> {
    crate::commands::permissions::require_file_manager_window(&window)?;
    store.list_candidates(root_id, limit.unwrap_or(25))
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub async fn process_smart_cache_for_room(
    room_id: String,
    limit: Option<i64>,
    window: tauri::Window,
    runtime: tauri::State<'_, AgentRuntime>,
    roots: tauri::State<'_, ManagedRootStore>,
    smart_cache: tauri::State<'_, SmartCacheStore>,
    outbox: tauri::State<'_, OutboxStore>,
    limiter: tauri::State<'_, WorkLimiter>,
) -> Result<SmartCacheProcessingReport, String> {
    crate::commands::permissions::require_file_manager_window(&window)?;
    let _permit = limiter.try_transfer()?;
    crate::smart_cache_processor::process_smart_cache_for_room(
        &runtime,
        &roots,
        &smart_cache,
        &outbox,
        room_id,
        limit.unwrap_or(25),
    )
    .await
}

#[cfg(not(feature = "tauri-commands"))]
pub async fn process_smart_cache_for_room(
    room_id: String,
    limit: Option<i64>,
    runtime: &AgentRuntime,
    roots: &ManagedRootStore,
    smart_cache: &SmartCacheStore,
    outbox: &OutboxStore,
) -> Result<SmartCacheProcessingReport, String> {
    crate::smart_cache_processor::process_smart_cache_for_room(
        runtime,
        roots,
        smart_cache,
        outbox,
        room_id,
        limit.unwrap_or(25),
    )
    .await
}

#[cfg(not(feature = "tauri-commands"))]
pub fn list_smart_cache_candidates(
    root_id: String,
    limit: Option<i64>,
    store: &SmartCacheStore,
) -> Result<Vec<SmartCacheCandidate>, String> {
    store.list_candidates(root_id, limit.unwrap_or(25))
}
