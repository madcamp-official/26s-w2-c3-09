pub mod agent;
pub mod background;
pub mod cleanliness;
pub mod command_processor;
pub mod commands;
pub mod execution_processor;
pub mod file_browse_processor;
pub mod file_transfer_processor;
pub mod outbox_processor;
pub mod overlay;
pub mod smart_cache_processor;
pub mod storage;
#[cfg(feature = "tauri-commands")]
pub mod tray;
pub mod watcher;
pub mod watcher_lifecycle;

#[cfg(feature = "tauri-commands")]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(agent::AgentRuntime::default())
        .manage(storage::agent_sync::AgentSyncStore::default())
        .manage(background::BackgroundRuntime::default())
        .manage(overlay::OverlayRuntime::default())
        .manage(storage::auto_approval::AutoApprovalStore::default())
        .manage(storage::managed_roots::ManagedRootStore::default())
        .manage(storage::outbox::OutboxStore::default())
        .manage(storage::smart_cache::SmartCacheStore::default())
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
            let agent_sync = app.state::<storage::agent_sync::AgentSyncStore>();
            let agent_sync_path = app
                .path()
                .app_data_dir()
                .map_err(|error| format!("cannot resolve app data directory: {error}"))?
                .join("agent-sync.db");
            agent_sync
                .load_from_db(agent_sync_path)
                .map_err(|error| format!("cannot load desktop agent sync cursor: {error}"))?;
            let outbox = app.state::<storage::outbox::OutboxStore>();
            let outbox_path = app
                .path()
                .app_data_dir()
                .map_err(|error| format!("cannot resolve app data directory: {error}"))?
                .join("desktop-outbox.db");
            outbox
                .load_from_db(outbox_path)
                .map_err(|error| format!("cannot load desktop outbox: {error}"))?;
            let smart_cache = app.state::<storage::smart_cache::SmartCacheStore>();
            let smart_cache_path = app
                .path()
                .app_data_dir()
                .map_err(|error| format!("cannot resolve app data directory: {error}"))?
                .join("smart-cache.db");
            smart_cache
                .load_from_db(smart_cache_path)
                .map_err(|error| format!("cannot load smart cache metadata: {error}"))?;
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
            app.state::<background::BackgroundRuntime>()
                .start(app.handle().clone())?;
            if let Err(error) = tray::install_tray(app) {
                eprintln!("failed to install tray skeleton: {error}");
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::background::get_background_runtime_status,
            commands::background::start_background_runtime,
            commands::background::pause_background_runtime,
            commands::file_engine::register_managed_root,
            commands::file_engine::list_managed_roots,
            commands::file_engine::update_managed_root_state,
            commands::file_engine::prepare_demo_root,
            commands::file_engine::analyze_root,
            commands::file_engine::browse_root_tree,
            commands::file_engine::reindex_managed_root,
            commands::file_engine::search_managed_root,
            commands::file_engine::propose_file_changes,
            commands::file_engine::calculate_cleanliness_snapshot,
            commands::file_engine::validate_rule_draft,
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
            commands::agent::start_agent_pairing,
            commands::agent::poll_agent_pairing,
            commands::agent::send_agent_heartbeat,
            commands::agent::poll_agent_commands,
            commands::agent::process_agent_commands,
            commands::agent::process_agent_decisions,
            commands::agent::process_agent_file_browse_requests,
            commands::agent::process_agent_file_transfers,
            commands::agent::flush_agent_outbox,
            commands::agent::ensure_agent_room,
            commands::agent::submit_cleanliness_snapshot,
            commands::agent::replay_agent_events,
            commands::agent::update_agent_command_status,
            commands::agent::forget_agent_device,
            commands::overlay::get_overlay_status,
            commands::overlay::emit_character_event,
            commands::overlay::show_overlay,
            commands::overlay::hide_overlay,
            commands::smart_cache::record_smart_cache_usage_event,
            commands::smart_cache::update_smart_cache_file_preference,
            commands::smart_cache::list_smart_cache_candidates,
            commands::smart_cache::process_smart_cache_for_room,
            commands::watcher::start_watching_root,
            commands::watcher::stop_watching_root,
            commands::watcher::is_watching_root,
        ])
        .build(tauri::generate_context!())
        .expect("failed to build Tauri desktop app")
        .run(|app, event| {
            if let tauri::RunEvent::WindowEvent {
                label,
                event: tauri::WindowEvent::CloseRequested { api, .. },
                ..
            } = event
            {
                if label == "main" {
                    api.prevent_close();
                    tray::hide_main_window(app);
                }
            }
        });
}
