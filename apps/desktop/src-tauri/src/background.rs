use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
#[cfg(feature = "tauri-commands")]
use tauri::async_runtime::JoinHandle;
#[cfg(feature = "tauri-commands")]
use tokio::sync::watch;
#[cfg(feature = "tauri-commands")]
use tokio::time::{interval, Duration};

#[cfg(feature = "tauri-commands")]
const BACKGROUND_TICK_SECONDS: u64 = 15;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct BackgroundRuntimeStatus {
    pub state: BackgroundRuntimeState,
    pub last_started_unix_ms: Option<i64>,
    pub last_stopped_unix_ms: Option<i64>,
    pub last_heartbeat_unix_ms: Option<i64>,
    pub last_replay_unix_ms: Option<i64>,
    pub last_command_poll_unix_ms: Option<i64>,
    pub last_command_count: usize,
    pub last_error_message: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BackgroundRuntimeState {
    Stopped,
    Running,
    Suspended,
}

#[derive(Debug)]
pub struct BackgroundRuntime {
    status: Arc<Mutex<BackgroundRuntimeStatus>>,
    #[cfg(feature = "tauri-commands")]
    task: Mutex<Option<BackgroundTask>>,
}

#[cfg(feature = "tauri-commands")]
#[derive(Debug)]
struct BackgroundTask {
    stop: watch::Sender<bool>,
    handle: JoinHandle<()>,
}

impl Default for BackgroundRuntime {
    fn default() -> Self {
        Self {
            status: Arc::new(Mutex::new(BackgroundRuntimeStatus {
                state: BackgroundRuntimeState::Stopped,
                last_started_unix_ms: None,
                last_stopped_unix_ms: None,
                last_heartbeat_unix_ms: None,
                last_replay_unix_ms: None,
                last_command_poll_unix_ms: None,
                last_command_count: 0,
                last_error_message: None,
            })),
            #[cfg(feature = "tauri-commands")]
            task: Mutex::new(None),
        }
    }
}

impl BackgroundRuntime {
    pub fn status(&self) -> Result<BackgroundRuntimeStatus, String> {
        self.status
            .lock()
            .map(|status| status.clone())
            .map_err(|_| "background runtime lock poisoned".to_string())
    }

    #[cfg(feature = "tauri-commands")]
    pub fn start(&self, app: tauri::AppHandle) -> Result<BackgroundRuntimeStatus, String> {
        use tauri::Manager;

        if app
            .state::<crate::agent::AgentRuntime>()
            .connection_status()
            .device_id
            .is_none()
        {
            return self.start_suspended("desktop device pairing is required");
        }

        {
            let mut task = self
                .task
                .lock()
                .map_err(|_| "background task lock poisoned".to_string())?;
            if task.is_some() {
                return self.status();
            }

            let (stop, stop_rx) = watch::channel(false);
            let status = Arc::clone(&self.status);
            let handle = tauri::async_runtime::spawn(run_background_loop(app, status, stop_rx));
            *task = Some(BackgroundTask { stop, handle });
        }

        update_status(&self.status, |status| {
            status.state = BackgroundRuntimeState::Running;
            status.last_started_unix_ms = Some(unix_ms());
            status.last_error_message = None;
        });
        self.status()
    }

    pub fn start_suspended(
        &self,
        reason: impl Into<String>,
    ) -> Result<BackgroundRuntimeStatus, String> {
        update_status(&self.status, |status| {
            status.state = BackgroundRuntimeState::Suspended;
            status.last_started_unix_ms = Some(unix_ms());
            status.last_error_message = Some(reason.into());
        });
        self.status()
    }

    pub fn pause(&self, reason: impl Into<String>) -> Result<BackgroundRuntimeStatus, String> {
        #[cfg(feature = "tauri-commands")]
        {
            let mut task = self
                .task
                .lock()
                .map_err(|_| "background task lock poisoned".to_string())?;
            if let Some(task) = task.take() {
                let _ = task.stop.send(true);
                task.handle.abort();
            }
        }

        update_status(&self.status, |status| {
            status.state = BackgroundRuntimeState::Suspended;
            status.last_stopped_unix_ms = Some(unix_ms());
            status.last_error_message = Some(reason.into());
        });
        self.status()
    }
}

#[cfg(feature = "tauri-commands")]
async fn run_background_loop(
    app: tauri::AppHandle,
    status: Arc<Mutex<BackgroundRuntimeStatus>>,
    mut stop: watch::Receiver<bool>,
) {
    run_background_tick(&app, &status).await;

    let mut ticker = interval(Duration::from_secs(BACKGROUND_TICK_SECONDS));
    ticker.tick().await;
    loop {
        tokio::select! {
            _ = ticker.tick() => run_background_tick(&app, &status).await,
            changed = stop.changed() => {
                if changed.is_err() || *stop.borrow() {
                    mark_suspended(&status, "background runtime stopped");
                    break;
                }
            }
        }
    }
}

#[cfg(feature = "tauri-commands")]
async fn run_background_tick(app: &tauri::AppHandle, status: &Arc<Mutex<BackgroundRuntimeStatus>>) {
    use tauri::Manager;

    let agent = app.state::<crate::agent::AgentRuntime>();
    let sync_store = app.state::<crate::storage::agent_sync::AgentSyncStore>();

    match agent.heartbeat("ONLINE_IDLE".to_string()).await {
        Ok(_) => update_status(status, |status| {
            status.state = BackgroundRuntimeState::Running;
            status.last_heartbeat_unix_ms = Some(unix_ms());
            status.last_error_message = None;
        }),
        Err(error) => {
            update_status(status, |status| {
                status.last_error_message = Some(error.to_string());
            });
            return;
        }
    }

    if let Some(device_id) = agent.connection_status().device_id {
        match replay_events(&agent, &sync_store, &device_id).await {
            Ok(()) => update_status(status, |status| {
                status.last_replay_unix_ms = Some(unix_ms());
            }),
            Err(error) => update_status(status, |status| {
                status.last_error_message = Some(error);
            }),
        }
    }

    match agent.poll_commands().await {
        Ok(commands) => update_status(status, |status| {
            status.last_command_poll_unix_ms = Some(unix_ms());
            status.last_command_count = commands.len();
        }),
        Err(error) => update_status(status, |status| {
            status.last_error_message = Some(error.to_string());
        }),
    }
}

#[cfg(feature = "tauri-commands")]
async fn replay_events(
    agent: &crate::agent::AgentRuntime,
    sync_store: &crate::storage::agent_sync::AgentSyncStore,
    device_id: &str,
) -> Result<(), String> {
    let previous_cursor = sync_store.cursor(device_id)?;
    let events = agent
        .replay_events(previous_cursor, 100)
        .await
        .map_err(|error| error.to_string())?;
    if let Some(next_cursor) = events.last().map(|event| event.sequence) {
        sync_store.advance(device_id, next_cursor).await?;
    }
    Ok(())
}

#[cfg(feature = "tauri-commands")]
fn mark_suspended(status: &Arc<Mutex<BackgroundRuntimeStatus>>, reason: &str) {
    update_status(status, |status| {
        status.state = BackgroundRuntimeState::Suspended;
        status.last_stopped_unix_ms = Some(unix_ms());
        status.last_error_message = Some(reason.to_string());
    });
}

fn update_status(
    status: &Arc<Mutex<BackgroundRuntimeStatus>>,
    update: impl FnOnce(&mut BackgroundRuntimeStatus),
) {
    if let Ok(mut status) = status.lock() {
        update(&mut status);
    }
}

fn unix_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::{BackgroundRuntime, BackgroundRuntimeState};

    #[test]
    fn starts_suspended_until_transport_exists() {
        let runtime = BackgroundRuntime::default();

        let status = runtime
            .start_suspended("agent transport is not configured")
            .expect("start");

        assert_eq!(status.state, BackgroundRuntimeState::Suspended);
        assert!(status.last_started_unix_ms.is_some());
        assert_eq!(
            status.last_error_message,
            Some("agent transport is not configured".to_string())
        );
    }

    #[test]
    fn pause_records_reason_and_stop_time() {
        let runtime = BackgroundRuntime::default();

        let status = runtime.pause("user paused agent").expect("pause");

        assert_eq!(status.state, BackgroundRuntimeState::Suspended);
        assert!(status.last_stopped_unix_ms.is_some());
    }

    #[test]
    fn default_status_has_no_background_ticks() {
        let runtime = BackgroundRuntime::default();
        let status = runtime.status().expect("status");

        assert_eq!(status.state, BackgroundRuntimeState::Stopped);
        assert_eq!(status.last_command_count, 0);
        assert!(status.last_heartbeat_unix_ms.is_none());
    }
}
