use crate::storage::managed_roots::ManagedRootStore;
use crate::storage::watchers::WatcherStore;
use crate::watcher_lifecycle::start_root_watcher;

#[tauri::command]
pub fn start_watching_root(
    root_id: String,
    app: tauri::AppHandle,
    store: tauri::State<'_, ManagedRootStore>,
    watchers: tauri::State<'_, WatcherStore>,
) -> Result<(), String> {
    start_root_watcher(root_id, app, &store, &watchers)
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
