pub mod commands;
pub mod storage;

#[cfg(feature = "tauri-commands")]
pub fn run() {
    tauri::Builder::default()
        .manage(storage::managed_roots::ManagedRootStore::default())
        .invoke_handler(tauri::generate_handler![
            commands::file_engine::register_managed_root,
            commands::file_engine::list_managed_roots,
            commands::file_engine::analyze_root,
            commands::file_engine::propose_file_changes,
            commands::file_engine::precheck_file_changes,
            commands::file_engine::execute_file_changes,
            commands::file_engine::undo_last_file_operation,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Tauri desktop app");
}
