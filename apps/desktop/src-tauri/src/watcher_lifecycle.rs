#[cfg(any(feature = "tauri-commands", test))]
use crate::watcher::WatchChange;

/// Event name the frontend listens for to know a watched managed root changed on disk.
/// Payload is the `root_id` so the UI only refreshes the root it is currently showing.
pub const ROOT_CHANGED_EVENT: &str = "managed-root-changed";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RootIndexReconcileReport {
    pub inspected_root_count: usize,
    pub reindexed_root_count: usize,
    pub failed_root_count: usize,
}

#[cfg(feature = "tauri-commands")]
pub fn start_root_watcher(
    root_id: String,
    app: tauri::AppHandle,
    roots: &crate::storage::managed_roots::ManagedRootStore,
    watchers: &crate::storage::watchers::WatcherStore,
) -> Result<(), String> {
    use tauri::Emitter;

    let managed = roots.get(&root_id)?;
    if !managed.enabled {
        return Err(format!("managed root is disabled: {root_id}"));
    }

    let root = managed.root;
    let event_root_id = root_id.clone();
    let index_root = root.clone();

    // Startup/reconnect performs a full reconcile before incremental watching. The snapshot
    // produced immediately afterward is the one persisted for the dashboard and queued intact.
    file_engine_cli::file_index::reindex_root(&root).map_err(|error| error.to_string())?;
    crate::cleanliness::reconcile_cleanliness_snapshot(&app, &root_id)?;

    let watcher = crate::watcher::watch_root_changes(root, move |change| {
        if apply_index_change(&index_root, change).is_err() {
            // Never emit the managed root or filesystem error text: both can contain an
            // absolute user path. The UI receives the root-scoped refresh event below.
            eprintln!("MANAGED_ROOT_INDEX_UPDATE_FAILED");
        } else if crate::cleanliness::reconcile_cleanliness_snapshot(&app, &event_root_id).is_err()
        {
            eprintln!("CLEANLINESS_RECONCILE_FAILED root_id={event_root_id}");
        }
        let _ = app.emit(ROOT_CHANGED_EVENT, &event_root_id);
    })?;

    watchers.start(root_id.clone(), watcher)?;
    let _ = roots.update_health(
        &root_id,
        crate::storage::managed_roots::ManagedRootStatus::Ready,
        None,
    )?;

    Ok(())
}

#[cfg(feature = "tauri-commands")]
pub fn restore_startup_watchers(
    app: tauri::AppHandle,
    roots: &crate::storage::managed_roots::ManagedRootStore,
    watchers: &crate::storage::watchers::WatcherStore,
) -> Result<Vec<StartupWatcherResult>, String> {
    let candidates = roots
        .list()?
        .into_iter()
        .filter(|root| {
            root.enabled
                && root.watch_on_startup
                && root.room_binding_status
                    != crate::storage::managed_roots::RoomBindingStatus::Detached
        })
        .collect::<Vec<_>>();
    let mut results = Vec::with_capacity(candidates.len());

    for managed in candidates {
        let result = start_root_watcher(managed.root_id.clone(), app.clone(), roots, watchers);
        if let Err(error) = &result {
            let _ = roots.update_health(
                &managed.root_id,
                crate::storage::managed_roots::ManagedRootStatus::Error,
                Some(error.clone()),
            );
        }
        results.push(StartupWatcherResult {
            root_id: managed.root_id,
            started: result.is_ok(),
            error: result.err(),
        });
    }

    Ok(results)
}

/// Periodically rebuilds the authoritative browse/search index for roots that are currently
/// watched. OS watcher events stay the fast path; this scheduled pass repairs missed events
/// without polling the server or exposing absolute user paths in status/log output.
#[cfg(any(feature = "tauri-commands", test))]
pub fn reconcile_watched_root_indexes(
    roots: &crate::storage::managed_roots::ManagedRootStore,
    watchers: &crate::storage::watchers::WatcherStore,
    mut after_reindex: impl FnMut(&str) -> Result<(), String>,
) -> Result<RootIndexReconcileReport, String> {
    use crate::storage::managed_roots::{ManagedRootStatus, RoomBindingStatus};

    let mut report = RootIndexReconcileReport::default();

    for managed in roots.list()? {
        if !managed.enabled
            || managed.room_binding_status == RoomBindingStatus::Detached
            || !watchers.is_watching(&managed.root_id)?
        {
            continue;
        }

        report.inspected_root_count += 1;
        let reconcile_result = file_engine_cli::file_index::reindex_root(&managed.root)
            .map(|_| ())
            .map_err(|_| ())
            .and_then(|_| after_reindex(&managed.root_id).map_err(|_| ()));

        match reconcile_result {
            Ok(()) => {
                if roots
                    .update_health(&managed.root_id, ManagedRootStatus::Ready, None)
                    .is_ok()
                {
                    report.reindexed_root_count += 1;
                } else {
                    report.failed_root_count += 1;
                }
            }
            Err(()) => {
                report.failed_root_count += 1;
                let _ = roots.update_health(
                    &managed.root_id,
                    ManagedRootStatus::Error,
                    Some("scheduled index reconcile failed".to_string()),
                );
            }
        }
    }

    Ok(report)
}

#[cfg(any(feature = "tauri-commands", test))]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupWatcherResult {
    pub root_id: String,
    pub started: bool,
    pub error: Option<String>,
}

#[cfg(test)]
fn start_root_watcher_for_test(
    root_id: String,
    roots: &crate::storage::managed_roots::ManagedRootStore,
    watchers: &crate::storage::watchers::WatcherStore,
) -> Result<(), String> {
    let managed = roots.get(&root_id)?;
    if !managed.enabled {
        return Err(format!("managed root is disabled: {root_id}"));
    }
    let watcher = crate::watcher::watch_root(managed.root, || {})?;
    watchers.start(root_id, watcher)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use tempfile::tempdir;

    use crate::storage::managed_roots::{
        ManagedRoot, ManagedRootStatePatch, ManagedRootStore, RoomBindingStatus,
    };
    use crate::storage::watchers::WatcherStore;
    use crate::watcher::{watch_root, watch_root_changes};

    fn wait_until(condition: impl Fn() -> bool, timeout: Duration) -> bool {
        let start = Instant::now();
        while start.elapsed() < timeout {
            if condition() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        condition()
    }

    #[test]
    fn disabled_root_refuses_watcher_start() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(&root).expect("create root");
        let roots = ManagedRootStore::default();
        let watchers = WatcherStore::default();
        roots
            .upsert(ManagedRoot::new(
                "root-1".to_string(),
                root.display().to_string(),
                "root".to_string(),
            ))
            .expect("insert root");
        roots
            .update_state(
                "root-1",
                ManagedRootStatePatch {
                    enabled: Some(false),
                    watch_on_startup: None,
                },
            )
            .expect("disable root");

        let error = super::start_root_watcher_for_test("root-1".to_string(), &roots, &watchers)
            .expect_err("disabled root should not start");

        assert!(error.contains("disabled"));
        assert!(!watchers.is_watching("root-1").expect("is watching"));
    }

    #[test]
    fn watcher_updates_index_when_file_changes() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(&root).expect("create root");
        file_engine_cli::file_index::reindex_root(&root).expect("initial index");
        let calls = Arc::new(AtomicUsize::new(0));
        let calls_in_closure = Arc::clone(&calls);
        let index_root = root.display().to_string();
        let watcher = watch_root_changes(&root, move |change| {
            super::apply_index_change(&index_root, change).expect("apply index change");
            calls_in_closure.fetch_add(1, Ordering::SeqCst);
        })
        .expect("start watcher");
        let watchers = WatcherStore::default();
        watchers
            .start("root-1".to_string(), watcher)
            .expect("store watcher");

        fs::write(root.join("note.md"), "# note").expect("write file");

        let indexed = wait_until(
            || {
                file_engine_cli::file_index::search_index(&root, "note")
                    .map(|report| !report.files.is_empty())
                    .unwrap_or(false)
            },
            Duration::from_secs(3),
        );
        assert!(indexed, "watcher should index new file");
    }

    #[test]
    fn scheduled_reconcile_repairs_changes_that_a_watcher_missed() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(&root).expect("create root");
        file_engine_cli::file_index::reindex_root(&root).expect("initial index");
        fs::write(root.join("missed.txt"), "missed").expect("write missed file before watcher");

        let roots = ManagedRootStore::default();
        let watchers = WatcherStore::default();
        roots
            .upsert(ManagedRoot::new(
                "root-1".to_string(),
                root.display().to_string(),
                "root".to_string(),
            ))
            .expect("insert root");
        super::start_root_watcher_for_test("root-1".to_string(), &roots, &watchers)
            .expect("start watcher");

        let report = super::reconcile_watched_root_indexes(&roots, &watchers, |_| Ok(()))
            .expect("reconcile");

        assert_eq!(report.inspected_root_count, 1);
        assert_eq!(report.reindexed_root_count, 1);
        assert_eq!(report.failed_root_count, 0);
        let indexed = file_engine_cli::file_index::search_index(&root, "missed")
            .expect("search after reconcile");
        assert_eq!(indexed.files.len(), 1);
        assert_eq!(indexed.files[0].relative_path, "missed.txt");
    }

    #[test]
    fn scheduled_reconcile_skips_disabled_detached_and_unwatched_roots() {
        let temp = tempdir().expect("tempdir");
        let disabled_root = temp.path().join("disabled");
        let detached_root = temp.path().join("detached");
        let unwatched_root = temp.path().join("unwatched");
        for root in [&disabled_root, &detached_root, &unwatched_root] {
            fs::create_dir_all(root).expect("create root");
            file_engine_cli::file_index::reindex_root(root).expect("initial index");
        }

        let roots = ManagedRootStore::default();
        let watchers = WatcherStore::default();
        roots
            .upsert(ManagedRoot::new(
                "disabled".to_string(),
                disabled_root.display().to_string(),
                "disabled".to_string(),
            ))
            .expect("insert disabled root");
        roots
            .update_state(
                "disabled",
                ManagedRootStatePatch {
                    enabled: Some(false),
                    watch_on_startup: None,
                },
            )
            .expect("disable root");

        let mut detached = ManagedRoot::new(
            "detached".to_string(),
            detached_root.display().to_string(),
            "detached".to_string(),
        );
        detached.detached_room_id = Some("room-detached".to_string());
        detached.room_binding_status = RoomBindingStatus::Detached;
        roots.upsert(detached).expect("insert detached root");

        roots
            .upsert(ManagedRoot::new(
                "unwatched".to_string(),
                unwatched_root.display().to_string(),
                "unwatched".to_string(),
            ))
            .expect("insert unwatched root");

        watchers
            .start(
                "disabled".to_string(),
                watch_root(&disabled_root, || {}).expect("watch disabled root"),
            )
            .expect("store disabled watcher");
        watchers
            .start(
                "detached".to_string(),
                watch_root(&detached_root, || {}).expect("watch detached root"),
            )
            .expect("store detached watcher");

        let callback_calls = Arc::new(AtomicUsize::new(0));
        let callback_calls_in_closure = Arc::clone(&callback_calls);
        let report = super::reconcile_watched_root_indexes(&roots, &watchers, move |_| {
            callback_calls_in_closure.fetch_add(1, Ordering::SeqCst);
            Ok(())
        })
        .expect("reconcile");

        assert_eq!(report.inspected_root_count, 0);
        assert_eq!(report.reindexed_root_count, 0);
        assert_eq!(report.failed_root_count, 0);
        assert_eq!(callback_calls.load(Ordering::SeqCst), 0);
    }
}
