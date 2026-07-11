pub mod agent;
pub mod commands;
pub mod overlay;
pub mod storage;
pub mod watcher;
pub mod watcher_lifecycle;

#[cfg(feature = "tauri-commands")]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(agent::AgentRuntime::default())
        .manage(overlay::OverlayRuntime::default())
        .manage(storage::auto_approval::AutoApprovalStore::default())
        .manage(storage::managed_roots::ManagedRootStore::default())
        .manage(storage::watchers::WatcherStore::default())
        .setup(|app| {
            use tauri::Manager;

            let store = app.state::<storage::managed_roots::ManagedRootStore>();
            let storage_path = app
                .path()
                .app_data_dir()
                .map_err(|error| format!("cannot resolve app data directory: {error}"))?
                .join("managed-roots.db");

            store
                .load_from_db(storage_path)
                .map_err(|error| format!("cannot load managed roots: {error}"))?;
            let auto_approval = app.state::<storage::auto_approval::AutoApprovalStore>();
            let auto_approval_path = app
                .path()
                .app_data_dir()
                .map_err(|error| format!("cannot resolve app data directory: {error}"))?
                .join("auto-approval.db");

            auto_approval
                .load_from_db(auto_approval_path)
                .map_err(|error| format!("cannot load auto approval policies: {error}"))?;
            let watchers = app.state::<storage::watchers::WatcherStore>();
            let restore_results = watcher_lifecycle::restore_startup_watchers(
                app.handle().clone(),
                &store,
                &watchers,
            )?;
            for result in restore_results {
                if let Some(error) = result.error {
                    eprintln!(
                        "failed to restore watcher for {} on startup: {}",
                        result.root_id, error
                    );
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::file_engine::register_managed_root,
            commands::file_engine::list_managed_roots,
            commands::file_engine::update_managed_root_state,
            commands::file_engine::prepare_demo_root,
            commands::file_engine::analyze_root,
            commands::file_engine::browse_root_tree,
            commands::file_engine::reindex_managed_root,
            commands::file_engine::search_managed_root,
            commands::file_engine::propose_file_changes,
            commands::file_engine::get_auto_approval_policy,
            commands::file_engine::update_auto_approval_policy,
            commands::file_engine::auto_approve_file_changes,
            commands::file_engine::precheck_file_changes,
            commands::file_engine::execute_file_changes,
            commands::file_engine::trash_file,
            commands::file_engine::create_file,
            commands::file_engine::rename_file,
            commands::file_engine::undo_last_file_operation,
            commands::file_engine::undo_operation,
            commands::file_engine::list_operation_history,
            commands::file_engine::recover_journal,
            commands::agent::get_agent_connection_status,
            commands::agent::poll_agent_commands,
            commands::agent::send_agent_event,
            commands::overlay::get_overlay_status,
            commands::overlay::emit_character_event,
            commands::watcher::start_watching_root,
            commands::watcher::stop_watching_root,
            commands::watcher::is_watching_root,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Tauri desktop app");
}
