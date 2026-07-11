use std::path::Path;
use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};

use file_engine_cli::journal::STATE_DIR;

const DEBOUNCE: Duration = Duration::from_millis(500);
const POLL_INTERVAL: Duration = Duration::from_millis(100);

/// Watches a managed root for filesystem changes and calls `on_change` after a quiet period,
/// so a burst of edits (e.g. a large copy) collapses into one UI refresh instead of many.
/// Dropping the returned handle stops both the OS watch and the debounce thread.
pub struct RootWatcher {
    _watcher: RecommendedWatcher,
    stop_tx: Sender<()>,
}

impl Drop for RootWatcher {
    fn drop(&mut self) {
        let _ = self.stop_tx.send(());
    }
}

pub fn watch_root(
    root: impl AsRef<Path>,
    on_change: impl Fn() + Send + 'static,
) -> Result<RootWatcher, String> {
    let root = root.as_ref().to_path_buf();
    let (event_tx, event_rx) = channel::<notify::Result<Event>>();
    let (stop_tx, stop_rx) = channel::<()>();

    let mut watcher =
        RecommendedWatcher::new(event_tx, Config::default()).map_err(|error| error.to_string())?;
    watcher
        .watch(&root, RecursiveMode::Recursive)
        .map_err(|error| error.to_string())?;

    thread::spawn(move || {
        let mut pending = false;
        let mut last_relevant_event_at = Instant::now();

        loop {
            if stop_rx.try_recv().is_ok() {
                return;
            }

            match event_rx.recv_timeout(POLL_INTERVAL) {
                Ok(Ok(event)) => {
                    if is_relevant(&event) {
                        pending = true;
                        last_relevant_event_at = Instant::now();
                    }
                }
                // The watcher itself reports an error (e.g. event queue overflow): we can no
                // longer trust that we saw every change, so err on the side of refreshing.
                Ok(Err(_)) => {
                    pending = true;
                    last_relevant_event_at = Instant::now();
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }

            if pending && last_relevant_event_at.elapsed() >= DEBOUNCE {
                pending = false;
                on_change();
            }
        }
    });

    Ok(RootWatcher {
        _watcher: watcher,
        stop_tx,
    })
}

/// Ignores events where every touched path is inside `.housemouse`: journal writes and
/// managed-root bookkeeping should not themselves look like a user file change.
fn is_relevant(event: &Event) -> bool {
    event.paths.iter().any(|path| {
        !path
            .components()
            .any(|component| component.as_os_str() == STATE_DIR)
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use tempfile::tempdir;

    use super::watch_root;

    fn wait_until(condition: impl Fn() -> bool, timeout: Duration) -> bool {
        let start = Instant::now();
        while start.elapsed() < timeout {
            if condition() {
                return true;
            }
            thread_sleep();
        }
        condition()
    }

    fn thread_sleep() {
        std::thread::sleep(Duration::from_millis(20));
    }

    #[test]
    fn calls_on_change_after_a_new_file_is_written() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(&root).expect("create root");

        let calls = Arc::new(AtomicUsize::new(0));
        let calls_in_closure = Arc::clone(&calls);
        let _watcher = watch_root(&root, move || {
            calls_in_closure.fetch_add(1, Ordering::SeqCst);
        })
        .expect("watch root");

        fs::write(root.join("note.md"), "# note").expect("write note");

        let saw_change = wait_until(|| calls.load(Ordering::SeqCst) > 0, Duration::from_secs(3));
        assert!(saw_change, "expected on_change to fire after a file write");
    }

    #[test]
    fn ignores_changes_confined_to_the_housemouse_state_dir() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        let state_dir = root.join(".housemouse");
        fs::create_dir_all(&state_dir).expect("create state dir");

        let calls = Arc::new(AtomicUsize::new(0));
        let calls_in_closure = Arc::clone(&calls);
        let _watcher = watch_root(&root, move || {
            calls_in_closure.fetch_add(1, Ordering::SeqCst);
        })
        .expect("watch root");

        fs::write(state_dir.join("journal.jsonl"), "{}\n").expect("write journal");

        let saw_change = wait_until(
            || calls.load(Ordering::SeqCst) > 0,
            Duration::from_millis(1200),
        );
        assert!(
            !saw_change,
            "state-dir-only changes should not trigger on_change"
        );
    }

    #[test]
    fn stops_calling_on_change_after_the_handle_is_dropped() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(&root).expect("create root");

        let calls = Arc::new(AtomicUsize::new(0));
        let calls_in_closure = Arc::clone(&calls);
        let watcher = watch_root(&root, move || {
            calls_in_closure.fetch_add(1, Ordering::SeqCst);
        })
        .expect("watch root");
        drop(watcher);

        fs::write(root.join("note.md"), "# note").expect("write note");
        thread_sleep_for(Duration::from_millis(900));

        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    fn thread_sleep_for(duration: Duration) {
        std::thread::sleep(duration);
    }
}
