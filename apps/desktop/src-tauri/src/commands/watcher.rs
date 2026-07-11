use tauri::Emitter;

use crate::storage::managed_roots::ManagedRootStore;
use crate::storage::watchers::WatcherStore;
use crate::watcher::watch_root;

/// Event name the frontend listens for to know a watched managed root changed on disk.
/// Payload is the `root_id` so the UI only refreshes the root it is currently showing.
pub const ROOT_CHANGED_EVENT: &str = "managed-root-changed";

#[tauri::command]
pub fn start_watching_root(
    root_id: String,
    app: tauri::AppHandle,
    store: tauri::State<'_, ManagedRootStore>,
    watchers: tauri::State<'_, WatcherStore>,
) -> Result<(), String> {
    let root = store.get(&root_id)?.root;
    let event_root_id = root_id.clone();
    let index_root = root.clone();

    let watcher = watch_root(root, move || {
        // Keep the SQLite file_index current so search reflects the current disk state without
        // a manual reindex. Best-effort: if it fails we still tell the UI to refresh.
        if let Err(error) = file_engine_cli::file_index::reindex_root(&index_root) {
            eprintln!("failed to reindex {index_root}: {error}");
        }
        let _ = app.emit(ROOT_CHANGED_EVENT, &event_root_id);
    })?;

    watchers.start(root_id, watcher)
}

#[tauri::command]
pub fn stop_watching_root(
    root_id: String,
    watchers: tauri::State<'_, WatcherStore>,
) -> Result<bool, String> {
    watchers.stop(&root_id)
}

#[tauri::command]
pub fn is_watching_root(
    root_id: String,
    watchers: tauri::State<'_, WatcherStore>,
) -> Result<bool, String> {
    watchers.is_watching(&root_id)
}
