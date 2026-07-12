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
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(agent::AgentRuntime::default())
        .manage(storage::agent_sync::AgentSyncStore::default())
        .manage(overlay::OverlayRuntime::default())
        .manage(storage::auto_approval::AutoApprovalStore::default())
        .manage(storage::managed_roots::ManagedRootStore::default())
        .manage(storage::watchers::WatcherStore::default())
        .setup(|app| {
            use tauri::menu::{Menu, MenuItem};
            use tauri::tray::TrayIconBuilder;
            use tauri::Manager;

            let open = MenuItem::with_id(app, "open", "Open Housemouse", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let tray_menu = Menu::with_items(app, &[&open, &quit])?;
            let tray_icon = app
                .default_window_icon()
                .cloned()
                .ok_or("application icon is required for the system tray")?;
            TrayIconBuilder::with_id("housemouse-main")
                .icon(tray_icon)
                .tooltip("Housemouse Desktop Agent")
                .menu(&tray_menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;

            if let Some(window) = app.get_webview_window("main") {
                let window_to_hide = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = window_to_hide.hide();
                    }
                });
            }

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
            commands::agent::start_agent_pairing,
            commands::agent::poll_agent_pairing,
            commands::agent::send_agent_heartbeat,
            commands::agent::poll_agent_commands,
            commands::agent::ensure_agent_room,
            commands::agent::replay_agent_events,
            commands::agent::update_agent_command_status,
            commands::agent::forget_agent_device,
            commands::overlay::get_overlay_status,
            commands::overlay::emit_character_event,
            commands::watcher::start_watching_root,
            commands::watcher::stop_watching_root,
            commands::watcher::is_watching_root,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Tauri desktop app");
}
