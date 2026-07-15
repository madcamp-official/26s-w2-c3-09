use crate::storage::managed_roots::ManagedRootStore;
use crate::storage::watchers::WatcherStore;
use crate::watcher_lifecycle::start_root_watcher;

#[tauri::command]
pub fn start_watching_root(
    root_id: String,
    window: tauri::Window,
    app: tauri::AppHandle,
    store: tauri::State<'_, ManagedRootStore>,
    watchers: tauri::State<'_, WatcherStore>,
) -> Result<(), String> {
    crate::commands::permissions::require_file_manager_window(&window)?;
    start_root_watcher(root_id, app, &store, &watchers)
}

#[tauri::command]
pub fn stop_watching_root(
    root_id: String,
    window: tauri::Window,
    watchers: tauri::State<'_, WatcherStore>,
) -> Result<bool, String> {
    crate::commands::permissions::require_file_manager_window(&window)?;
    watchers.stop(&root_id)
}

#[tauri::command]
pub fn is_watching_root(
    root_id: String,
    window: tauri::Window,
    watchers: tauri::State<'_, WatcherStore>,
) -> Result<bool, String> {
    crate::commands::permissions::require_file_manager_window(&window)?;
    watchers.is_watching(&root_id)
}
