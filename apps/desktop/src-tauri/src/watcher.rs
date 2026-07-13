use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use file_engine_cli::journal::{STATE_DIR, TRASH_DIR};

const DEBOUNCE: Duration = Duration::from_millis(500);
const POLL_INTERVAL: Duration = Duration::from_millis(100);

/// Watches a managed root for filesystem changes and calls `on_change` after a quiet period,
/// so a burst of edits (e.g. a large copy) collapses into one UI refresh instead of many.
/// Dropping the returned handle stops both the OS watch and the debounce thread.
pub struct RootWatcher {
    _watcher: RecommendedWatcher,
    stop_tx: Sender<()>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchChange {
    UpsertFile { relative_path: String },
    RemovePath { relative_path: String },
    Reindex,
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
    watch_root_changes(root, move |_| on_change())
}

pub fn watch_root_changes(
    root: impl AsRef<Path>,
    on_change: impl Fn(WatchChange) + Send + 'static,
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
        let mut pending = Vec::new();
        let mut last_relevant_event_at = Instant::now();

        loop {
            if stop_rx.try_recv().is_ok() {
                return;
            }

            match event_rx.recv_timeout(POLL_INTERVAL) {
                Ok(Ok(event)) => {
                    if is_relevant(&event) {
                        pending.extend(classify_change(&root, &event));
                        last_relevant_event_at = Instant::now();
                    }
                }
                // The watcher itself reports an error (e.g. event queue overflow): we can no
                // longer trust that we saw every change, so err on the side of refreshing.
                Ok(Err(_)) => {
                    pending.push(WatchChange::Reindex);
                    last_relevant_event_at = Instant::now();
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }

            if !pending.is_empty() && last_relevant_event_at.elapsed() >= DEBOUNCE {
                let changes = std::mem::take(&mut pending);
                for change in coalesce_changes(changes) {
                    on_change(change);
                }
            }
        }
    });

    Ok(RootWatcher {
        _watcher: watcher,
        stop_tx,
    })
}

/// Ignores events where every touched path is inside MouseKeeper bookkeeping folders:
/// journal writes and trash metadata should not themselves look like user file changes.
fn is_relevant(event: &Event) -> bool {
    event.paths.iter().any(|path| {
        !path.components().any(|component| {
            let name = component.as_os_str();
            name == STATE_DIR || name == TRASH_DIR
        })
    })
}

fn classify_change(root: &Path, event: &Event) -> Vec<WatchChange> {
    if event.need_rescan() || event.paths.is_empty() {
        return vec![WatchChange::Reindex];
    }

    match event.kind {
        EventKind::Remove(_) => classify_remove(root, &event.paths),
        EventKind::Create(_) | EventKind::Modify(_) => {
            classify_present_or_missing(root, &event.paths)
        }
        EventKind::Any | EventKind::Other => vec![WatchChange::Reindex],
        _ => vec![WatchChange::Reindex],
    }
}

fn classify_remove(root: &Path, paths: &[PathBuf]) -> Vec<WatchChange> {
    let changes = paths
        .iter()
        .filter_map(|path| relative_path(root, path))
        .map(|relative_path| WatchChange::RemovePath { relative_path })
        .collect::<Vec<_>>();

    if changes.is_empty() {
        vec![WatchChange::Reindex]
    } else {
        changes
    }
}

fn classify_present_or_missing(root: &Path, paths: &[PathBuf]) -> Vec<WatchChange> {
    let mut changes = Vec::new();

    for path in paths {
        let Some(relative_path) = relative_path(root, path) else {
            changes.push(WatchChange::Reindex);
            continue;
        };

        let Ok(metadata) = std::fs::symlink_metadata(path) else {
            changes.push(WatchChange::RemovePath { relative_path });
            continue;
        };

        if metadata.is_file() {
            changes.push(WatchChange::UpsertFile { relative_path });
        } else {
            changes.push(WatchChange::Reindex);
        }
    }

    changes
}

fn relative_path(root: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(root).ok()?;
    if relative.as_os_str().is_empty() {
        return None;
    }

    Some(
        relative
            .components()
            .map(|component| component.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/"),
    )
}

fn coalesce_changes(changes: Vec<WatchChange>) -> Vec<WatchChange> {
    if changes.iter().any(|change| *change == WatchChange::Reindex) {
        return vec![WatchChange::Reindex];
    }

    changes
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use notify::event::{CreateKind, ModifyKind, RemoveKind};
    use notify::{Event, EventKind};
    use tempfile::tempdir;

    use super::{classify_change, is_relevant, watch_root, WatchChange};

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
    fn ignores_changes_confined_to_the_mousekeeper_state_dir() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        let state_dir = root.join(".mousekeeper");
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
    fn ignores_events_confined_to_the_mousekeeper_trash_dir() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        let event = Event::new(EventKind::Modify(ModifyKind::Data(
            notify::event::DataChange::Content,
        )))
        .add_path(root.join(".mousekeeper_trash").join("trash-1").join("file"));

        assert!(!is_relevant(&event));
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

    #[test]
    fn classifies_file_create_as_upsert() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(&root).expect("create root");
        let path = root.join("note.md");
        fs::write(&path, "# note").expect("write note");

        let event = Event::new(EventKind::Create(CreateKind::File)).add_path(path);

        assert_eq!(
            classify_change(&root, &event),
            vec![WatchChange::UpsertFile {
                relative_path: "note.md".to_string()
            }]
        );
    }

    #[test]
    fn classifies_remove_as_path_removal() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(&root).expect("create root");
        let path = root.join("old").join("note.md");

        let event = Event::new(EventKind::Remove(RemoveKind::File)).add_path(path);

        assert_eq!(
            classify_change(&root, &event),
            vec![WatchChange::RemovePath {
                relative_path: "old/note.md".to_string()
            }]
        );
    }

    #[test]
    fn classifies_multi_path_rename_like_event_for_old_and_new_paths() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir_all(&root).expect("create root");
        let old_path = root.join("old.txt");
        let new_path = root.join("new.txt");
        fs::write(&new_path, "new").expect("write new");

        let event = Event::new(EventKind::Modify(ModifyKind::Name(
            notify::event::RenameMode::Both,
        )))
        .add_path(old_path)
        .add_path(new_path);

        assert_eq!(
            classify_change(&root, &event),
            vec![
                WatchChange::RemovePath {
                    relative_path: "old.txt".to_string()
                },
                WatchChange::UpsertFile {
                    relative_path: "new.txt".to_string()
                }
            ]
        );
    }
}
