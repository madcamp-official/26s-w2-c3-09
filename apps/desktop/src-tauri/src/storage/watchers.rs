use std::collections::HashMap;
use std::sync::Mutex;

use crate::watcher::RootWatcher;

/// Holds the active watcher for each root_id currently being watched. Inserting a new watcher
/// for a root_id that already has one drops (and so stops) the previous one; removing an
/// entry stops it the same way, via `RootWatcher`'s `Drop` impl.
#[derive(Default)]
pub struct WatcherStore {
    watchers: Mutex<HashMap<String, RootWatcher>>,
}

impl WatcherStore {
    pub fn start(&self, root_id: String, watcher: RootWatcher) -> Result<(), String> {
        let mut watchers = self
            .watchers
            .lock()
            .map_err(|_| "watcher store lock poisoned".to_string())?;
        watchers.insert(root_id, watcher);
        Ok(())
    }

    pub fn stop(&self, root_id: &str) -> Result<bool, String> {
        let mut watchers = self
            .watchers
            .lock()
            .map_err(|_| "watcher store lock poisoned".to_string())?;
        Ok(watchers.remove(root_id).is_some())
    }

    pub fn is_watching(&self, root_id: &str) -> Result<bool, String> {
        let watchers = self
            .watchers
            .lock()
            .map_err(|_| "watcher store lock poisoned".to_string())?;
        Ok(watchers.contains_key(root_id))
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use tempfile::tempdir;

    use crate::watcher::watch_root;

    use super::WatcherStore;

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
    fn starting_a_second_watcher_for_the_same_root_stops_the_first() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(&root).expect("create root");

        let store = WatcherStore::default();

        let first_calls = Arc::new(AtomicUsize::new(0));
        let first_calls_in_closure = Arc::clone(&first_calls);
        let first_watcher = watch_root(&root, move || {
            first_calls_in_closure.fetch_add(1, Ordering::SeqCst);
        })
        .expect("watch root");
        store
            .start("root-1".to_string(), first_watcher)
            .expect("start first watcher");

        let second_calls = Arc::new(AtomicUsize::new(0));
        let second_calls_in_closure = Arc::clone(&second_calls);
        let second_watcher = watch_root(&root, move || {
            second_calls_in_closure.fetch_add(1, Ordering::SeqCst);
        })
        .expect("watch root");
        store
            .start("root-1".to_string(), second_watcher)
            .expect("start second watcher replacing first");

        fs::write(root.join("note.md"), "# note").expect("write note");

        let second_saw_change = wait_until(
            || second_calls.load(Ordering::SeqCst) > 0,
            Duration::from_secs(3),
        );
        assert!(second_saw_change, "replacement watcher should still fire");
        assert_eq!(
            first_calls.load(Ordering::SeqCst),
            0,
            "replaced watcher should have been stopped"
        );
    }

    #[test]
    fn stop_removes_entry_and_reports_whether_one_existed() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(&root).expect("create root");

        let store = WatcherStore::default();
        let watcher = watch_root(&root, || {}).expect("watch root");
        store.start("root-1".to_string(), watcher).expect("start");

        assert!(store.is_watching("root-1").expect("is_watching"));
        assert!(store.stop("root-1").expect("stop existing"));
        assert!(!store.is_watching("root-1").expect("is_watching after stop"));
        assert!(!store.stop("root-1").expect("stop already-stopped root"));
    }
}
