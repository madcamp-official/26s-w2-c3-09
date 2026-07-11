pub mod commands;
pub mod storage;

#[cfg(feature = "tauri-commands")]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(storage::managed_roots::ManagedRootStore::default())
        .setup(|app| {
            use tauri::Manager;

            let store = app.state::<storage::managed_roots::ManagedRootStore>();
            let storage_path = app
                .path()
                .app_data_dir()
                .map_err(|error| format!("cannot resolve app data directory: {error}"))?
                .join("managed-roots.json");

            store
                .load_from_file(storage_path)
                .map_err(|error| format!("cannot load managed roots: {error}"))?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::file_engine::register_managed_root,
            commands::file_engine::list_managed_roots,
            commands::file_engine::analyze_root,
            commands::file_engine::browse_root_tree,
            commands::file_engine::propose_file_changes,
            commands::file_engine::precheck_file_changes,
            commands::file_engine::execute_file_changes,
            commands::file_engine::undo_last_file_operation,
            commands::file_engine::undo_operation,
            commands::file_engine::list_operation_history,
            commands::file_engine::recover_journal,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Tauri desktop app");
}
