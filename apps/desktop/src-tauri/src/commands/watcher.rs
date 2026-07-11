use tauri::Emitter;

use crate::storage::managed_roots::ManagedRootStore;
use crate::storage::watchers::WatcherStore;
use crate::watcher::{watch_root_changes, WatchChange};

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

    let watcher = watch_root_changes(root, move |change| {
        // Keep SQLite current from the smallest trustworthy watcher update. Directory changes
        // and watcher errors fall back to reindexing so search never trusts a partial tree.
        if let Err(error) = apply_index_change(&index_root, change) {
            eprintln!("failed to update index for {index_root}: {error}");
        }
        let _ = app.emit(ROOT_CHANGED_EVENT, &event_root_id);
    })?;

    watchers.start(root_id, watcher)
}

fn apply_index_change(root: &str, change: WatchChange) -> Result<(), String> {
    match change {
        WatchChange::UpsertFile { relative_path } => {
            file_engine_cli::file_index::upsert_existing_file(root, &relative_path)
        }
        WatchChange::RemovePath { relative_path } => {
            file_engine_cli::file_index::remove_path(root, &relative_path)
        }
        WatchChange::Reindex => file_engine_cli::file_index::reindex_root(root).map(|_| ()),
    }
    .map_err(|error| error.to_string())
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
