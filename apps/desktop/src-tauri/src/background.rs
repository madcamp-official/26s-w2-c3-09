use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
#[cfg(feature = "tauri-commands")]
use tauri::async_runtime::JoinHandle;
#[cfg(feature = "tauri-commands")]
use tokio::sync::{watch, Notify};
#[cfg(feature = "tauri-commands")]
use tokio::time::{interval, Duration};

#[cfg(feature = "tauri-commands")]
const BACKGROUND_TICK_SECONDS: u64 = 5;
#[cfg(feature = "tauri-commands")]
const BACKGROUND_FAST_RECONCILE_EVERY_TICKS: u64 = 3;
#[cfg(feature = "tauri-commands")]
const BACKGROUND_FULL_RECONCILE_EVERY_TICKS: u64 = 6;
#[cfg(feature = "tauri-commands")]
const AUTO_CLEANUP_INTERVAL_MS: i64 = 15 * 1000;
/// Delay before retrying the realtime Socket.IO connection after a failed initial connect. The
/// REST background loop keeps working the whole time, so this only affects push responsiveness.
#[cfg(feature = "tauri-commands")]
const REALTIME_RECONNECT_SECONDS: u64 = 15;
#[cfg(feature = "tauri-commands")]
const DEVICE_REVOKED_REASON: &str = "desktop device pairing was revoked; pair the desktop again";
/// Socket.IO event names that mean "there is new agent work or a state change worth a REST pass".
/// Each one only wakes the REST loop; the events themselves are never treated as the source of
/// truth (that stays `/v1/sync/events` replay and command/decision polling).
#[cfg(feature = "tauri-commands")]
const REALTIME_WAKE_EVENTS: [&str; 9] = [
    "command.available",
    "command.updated",
    "proposal.created",
    "decision.created",
    "file.browse.requested",
    "file.transfer.requested",
    "smart-cache.updated",
    "device.revoked",
    "room.removed",
];

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct BackgroundRuntimeStatus {
    pub state: BackgroundRuntimeState,
    pub last_started_unix_ms: Option<i64>,
    pub last_stopped_unix_ms: Option<i64>,
    pub last_heartbeat_unix_ms: Option<i64>,
    pub last_replay_unix_ms: Option<i64>,
    pub last_command_poll_unix_ms: Option<i64>,
    pub last_command_count: usize,
    pub last_processed_command_count: usize,
    pub last_submitted_proposal_count: usize,
    pub last_decision_poll_unix_ms: Option<i64>,
    pub last_decision_count: usize,
    pub last_executed_item_count: usize,
    pub last_execution_failed_count: usize,
    pub last_realtime_signal_unix_ms: Option<i64>,
    pub last_file_browse_poll_unix_ms: Option<i64>,
    pub last_file_browse_count: usize,
    pub last_file_browse_completed_count: usize,
    pub last_file_browse_failed_count: usize,
    pub last_file_index_reconcile_unix_ms: Option<i64>,
    pub last_file_index_reconcile_root_count: usize,
    pub last_file_index_reconcile_failed_count: usize,
    pub last_file_transfer_poll_unix_ms: Option<i64>,
    pub last_file_transfer_count: usize,
    pub last_file_transfer_uploaded_count: usize,
    pub last_file_transfer_failed_count: usize,
    pub last_smart_cache_poll_unix_ms: Option<i64>,
    pub last_smart_cache_candidate_count: usize,
    pub last_smart_cache_uploaded_count: usize,
    pub last_smart_cache_failed_count: usize,
    pub last_auto_cleanup_unix_ms: Option<i64>,
    pub last_auto_cleanup_root_count: usize,
    pub last_auto_cleanup_approved_count: usize,
    pub last_auto_cleanup_executed_count: usize,
    pub last_auto_cleanup_failed_count: usize,
    pub last_auto_cleanup_proposals: Vec<crate::auto_cleanup_processor::AutoCleanupProposalEvent>,
    pub last_auto_submitted_proposal_count: usize,
    pub last_outbox_flush_unix_ms: Option<i64>,
    pub last_outbox_sent_count: usize,
    pub last_outbox_failed_count: usize,
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
    realtime_handle: Option<JoinHandle<()>>,
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
                last_processed_command_count: 0,
                last_submitted_proposal_count: 0,
                last_decision_poll_unix_ms: None,
                last_decision_count: 0,
                last_executed_item_count: 0,
                last_execution_failed_count: 0,
                last_realtime_signal_unix_ms: None,
                last_file_browse_poll_unix_ms: None,
                last_file_browse_count: 0,
                last_file_browse_completed_count: 0,
                last_file_browse_failed_count: 0,
                last_file_index_reconcile_unix_ms: None,
                last_file_index_reconcile_root_count: 0,
                last_file_index_reconcile_failed_count: 0,
                last_file_transfer_poll_unix_ms: None,
                last_file_transfer_count: 0,
                last_file_transfer_uploaded_count: 0,
                last_file_transfer_failed_count: 0,
                last_smart_cache_poll_unix_ms: None,
                last_smart_cache_candidate_count: 0,
                last_smart_cache_uploaded_count: 0,
                last_smart_cache_failed_count: 0,
                last_auto_cleanup_unix_ms: None,
                last_auto_cleanup_root_count: 0,
                last_auto_cleanup_approved_count: 0,
                last_auto_cleanup_executed_count: 0,
                last_auto_cleanup_failed_count: 0,
                last_auto_cleanup_proposals: Vec::new(),
                last_auto_submitted_proposal_count: 0,
                last_outbox_flush_unix_ms: None,
                last_outbox_sent_count: 0,
                last_outbox_failed_count: 0,
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

        let agent_status = app
            .state::<crate::agent::AgentRuntime>()
            .connection_status();
        if matches!(
            agent_status.state,
            crate::agent::AgentConnectionState::Revoked
        ) {
            return self.start_suspended(DEVICE_REVOKED_REASON);
        }

        {
            let mut task = self
                .task
                .lock()
                .map_err(|_| "background task lock poisoned".to_string())?;
            let should_clear_revoked_task = self
                .status
                .lock()
                .map(|status| {
                    status.state == BackgroundRuntimeState::Suspended
                        && status.last_error_message.as_deref() == Some(DEVICE_REVOKED_REASON)
                })
                .unwrap_or(false);
            if should_clear_revoked_task {
                if let Some(background_task) = task.take() {
                    background_task.handle.abort();
                    if let Some(realtime_handle) = background_task.realtime_handle {
                        realtime_handle.abort();
                    }
                }
            }
            if task.is_some() {
                return self.status();
            }

            let (stop, stop_rx) = watch::channel(false);
            let status = Arc::clone(&self.status);
            // `wake` lets the realtime Socket.IO client nudge the REST loop to run a tick
            // immediately instead of waiting for the next fixed interval.
            let wake = Arc::new(Notify::new());

            // Open the realtime notification client only when paired. It is a pure latency
            // optimization: if it never connects, the interval-driven REST loop still runs.
            let realtime_handle = app
                .state::<crate::agent::AgentRuntime>()
                .realtime_credentials()
                .map(|credentials| {
                    tauri::async_runtime::spawn(run_realtime_client(
                        credentials,
                        Arc::clone(&wake),
                        stop_rx.clone(),
                    ))
                });

            let stop_for_loop = stop.clone();
            let handle = tauri::async_runtime::spawn(run_background_loop(
                app,
                status,
                stop_rx,
                wake,
                stop_for_loop,
            ));
            *task = Some(BackgroundTask {
                stop,
                handle,
                realtime_handle,
            });
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
                if let Some(realtime_handle) = task.realtime_handle {
                    realtime_handle.abort();
                }
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
    wake: Arc<Notify>,
    stop_signal: watch::Sender<bool>,
) {
    if matches!(
        run_background_tick(&app, &status, BackgroundTickMode::HeartbeatAndFullReconcile,).await,
        BackgroundTickOutcome::Stop
    ) {
        let _ = stop_signal.send(true);
        return;
    }

    let mut ticker = interval(Duration::from_secs(BACKGROUND_TICK_SECONDS));
    ticker.tick().await;
    let mut scheduled_tick_count = 0_u64;
    loop {
        tokio::select! {
            _ = ticker.tick() => {
                scheduled_tick_count = scheduled_tick_count.saturating_add(1);
                if matches!(
                    run_background_tick(
                        &app,
                        &status,
                        scheduled_tick_mode(scheduled_tick_count),
                    ).await,
                    BackgroundTickOutcome::Stop
                ) {
                    let _ = stop_signal.send(true);
                    break;
                }
            }
            // A realtime notification reconciles work immediately without sending an extra
            // heartbeat. The five-second scheduled heartbeat remains the sole liveness cadence.
            _ = wake.notified() => {
                update_status(&status, |status| {
                    status.last_realtime_signal_unix_ms = Some(unix_ms());
                });
                if matches!(
                    run_background_tick(&app, &status, BackgroundTickMode::FullReconcileOnly).await,
                    BackgroundTickOutcome::Stop
                ) {
                    let _ = stop_signal.send(true);
                    break;
                }
            }
            changed = stop.changed() => {
                if changed.is_err() || *stop.borrow() {
                    mark_suspended(&status, "background runtime stopped");
                    break;
                }
            }
        }
    }
}

/// Runs the realtime Socket.IO client. It joins the authenticated `user:<id>` room and, for each
/// agent-relevant event, nudges the REST loop via `wake`. Socket.IO is treated strictly as a
/// "new data exists" hint: it never carries authoritative state, so a disconnect or missed event
/// only costs latency, not correctness — the interval REST loop still reconciles everything.
#[cfg(feature = "tauri-commands")]
async fn run_realtime_client(
    credentials: crate::agent::RealtimeCredentials,
    wake: Arc<Notify>,
    mut stop: watch::Receiver<bool>,
) {
    use futures_util::FutureExt;
    use rust_socketio::asynchronous::ClientBuilder;

    loop {
        if *stop.borrow() {
            return;
        }

        let mut builder = ClientBuilder::new(credentials.base_url.clone())
            .namespace("/realtime")
            .auth(serde_json::json!({ "token": credentials.device_token }))
            .reconnect(true)
            .reconnect_on_disconnect(true)
            .reconnect_delay(2_000, 10_000);

        for event in REALTIME_WAKE_EVENTS {
            let wake = Arc::clone(&wake);
            builder = builder.on(event, move |_payload, _client| {
                let wake = Arc::clone(&wake);
                async move {
                    wake.notify_one();
                }
                .boxed()
            });
        }

        match builder.connect().await {
            Ok(client) => {
                // The client keeps itself connected (and reconnects on drop) until we stop.
                let _ = stop.changed().await;
                let _ = client.disconnect().await;
                return;
            }
            Err(_) => {
                // Server unreachable at connect time. The REST loop is the fallback; retry later
                // unless we are being stopped.
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(REALTIME_RECONNECT_SECONDS)) => {}
                    _ = stop.changed() => return,
                }
            }
        }
    }
}

#[cfg(feature = "tauri-commands")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BackgroundTickOutcome {
    Continue,
    Stop,
}

#[cfg(feature = "tauri-commands")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BackgroundTickMode {
    HeartbeatOnly,
    FullReconcileOnly,
    HeartbeatAndFastReconcile,
    HeartbeatAndFullReconcile,
}

#[cfg(feature = "tauri-commands")]
impl BackgroundTickMode {
    fn sends_heartbeat(self) -> bool {
        matches!(
            self,
            Self::HeartbeatOnly | Self::HeartbeatAndFastReconcile | Self::HeartbeatAndFullReconcile
        )
    }

    fn reconciles(self) -> bool {
        !matches!(self, Self::HeartbeatOnly)
    }

    fn runs_heavy_rest(self) -> bool {
        matches!(
            self,
            Self::FullReconcileOnly | Self::HeartbeatAndFullReconcile
        )
    }
}

#[cfg(feature = "tauri-commands")]
fn scheduled_tick_mode(tick: u64) -> BackgroundTickMode {
    if tick % BACKGROUND_FULL_RECONCILE_EVERY_TICKS == 0 {
        BackgroundTickMode::HeartbeatAndFullReconcile
    } else if tick % BACKGROUND_FAST_RECONCILE_EVERY_TICKS == 0 {
        BackgroundTickMode::HeartbeatAndFastReconcile
    } else {
        BackgroundTickMode::HeartbeatOnly
    }
}

#[cfg(feature = "tauri-commands")]
async fn run_background_tick(
    app: &tauri::AppHandle,
    status: &Arc<Mutex<BackgroundRuntimeStatus>>,
    mode: BackgroundTickMode,
) -> BackgroundTickOutcome {
    use tauri::{Emitter, Manager};

    let agent = app.state::<crate::agent::AgentRuntime>();
    let sync_store = app.state::<crate::storage::agent_sync::AgentSyncStore>();
    let roots = app.state::<crate::storage::managed_roots::ManagedRootStore>();
    let auto_approval = app.state::<crate::storage::auto_approval::AutoApprovalStore>();
    let outbox = app.state::<crate::storage::outbox::OutboxStore>();
    let smart_cache = app.state::<crate::storage::smart_cache::SmartCacheStore>();
    let watchers = app.state::<crate::storage::watchers::WatcherStore>();
    let work_limiter = app.state::<crate::work_limiter::WorkLimiter>();

    // Collected across this pass and turned into a single CharacterEvent for the overlay at the end.
    let mut activity = crate::overlay::OverlayActivity::default();

    if mode.sends_heartbeat() {
        let device_id_before_heartbeat = agent.connection_status().device_id;
        match agent.heartbeat("ONLINE_IDLE".to_string()).await {
            Ok(_) => update_status(status, |status| {
                status.state = BackgroundRuntimeState::Running;
                status.last_heartbeat_unix_ms = Some(unix_ms());
                status.last_error_message = None;
            }),
            Err(error) => {
                let is_confirmed_device_absent = error.is_confirmed_device_absent();
                let error_message = error.to_string();
                if is_confirmed_device_absent {
                    let _ = crate::commands::agent::apply_local_device_revoked(
                        &agent,
                        &sync_store,
                        &roots,
                        &watchers,
                    )
                    .await;
                    if let Some(device_id) = device_id_before_heartbeat {
                        let _ = sync_store.clear_device(&device_id).await;
                        // An authoritative DEVICE_REVOKED or DEVICE_NOT_REGISTERED verdict means
                        // the saved pairing cannot be recovered. Wake the UI so it starts a fresh
                        // pairing session instead of waiting for its periodic status refresh.
                        let _ = app.emit("desktop-device-revoked", device_id);
                    }
                    mark_suspended(status, DEVICE_REVOKED_REASON);
                } else {
                    update_status(status, |status| {
                        status.last_error_message = Some(error_message);
                    });
                }
                // Heartbeat failure means we are offline; tell the overlay so the character can
                // reflect it, then stop this pass.
                emit_overlay_activity(
                    app,
                    crate::overlay::OverlayActivity {
                        had_error: true,
                        ..crate::overlay::OverlayActivity::default()
                    },
                );
                return if is_confirmed_device_absent {
                    BackgroundTickOutcome::Stop
                } else {
                    BackgroundTickOutcome::Continue
                };
            }
        }
    }

    if !mode.reconciles() {
        return BackgroundTickOutcome::Continue;
    }

    if let Some(device_id) = agent.connection_status().device_id {
        match replay_events(app, &agent, &sync_store, &roots, &watchers, &device_id).await {
            Ok(ReplayOutcome::Continue) => update_status(status, |status| {
                status.last_replay_unix_ms = Some(unix_ms());
            }),
            Ok(ReplayOutcome::DeviceRevoked) => {
                mark_suspended(status, DEVICE_REVOKED_REASON);
                return BackgroundTickOutcome::Stop;
            }
            Err(error) => {
                update_status(status, |status| {
                    status.last_error_message = Some(error);
                });
                // Replay establishes the durable room/device authority used by every autonomous
                // action below. Fail closed for this tick so a missed room.removed event cannot be
                // followed by a stale proposal or file operation.
                emit_overlay_activity(
                    app,
                    crate::overlay::OverlayActivity {
                        had_error: true,
                        ..crate::overlay::OverlayActivity::default()
                    },
                );
                return BackgroundTickOutcome::Continue;
            }
        }
        if stop_if_agent_revoked(app, status, &agent) {
            return BackgroundTickOutcome::Stop;
        }
    }

    // Background proposals are computed only after heartbeat and replay have confirmed that this
    // desktop still owns an active server connection.
    let mut pending_auto_cleanup = None;
    if should_run_auto_cleanup(status) {
        match work_limiter.try_write() {
            Ok(_permit) => {
                match crate::auto_cleanup_processor::process_auto_cleanup(&roots, &auto_approval) {
                    Ok(report) => {
                        for proposal in &report.proposals {
                            let _ = app.emit(
                                crate::auto_cleanup_processor::AUTO_CLEANUP_PROPOSAL_EVENT,
                                proposal,
                            );
                        }
                        activity.executed_item_count += report.executed_count;
                        if report.failed_count > 0 {
                            activity.had_error = true;
                        }
                        update_status(status, |status| {
                            status.state = BackgroundRuntimeState::Running;
                            status.last_auto_cleanup_unix_ms = Some(unix_ms());
                            status.last_auto_cleanup_root_count = report.proposed_root_count;
                            status.last_auto_cleanup_approved_count = report.approved_count;
                            status.last_auto_cleanup_executed_count = report.executed_count;
                            status.last_auto_cleanup_failed_count = report.failed_count;
                            status.last_auto_cleanup_proposals = report.proposals.clone();
                            if report.failed_count > 0 {
                                status.last_error_message = Some(format!(
                                    "{} auto cleanup root(s) failed",
                                    report.failed_count
                                ));
                            }
                        });
                        pending_auto_cleanup = Some(report);
                    }
                    Err(error) => {
                        activity.had_error = true;
                        update_status(status, |status| {
                            status.state = BackgroundRuntimeState::Running;
                            status.last_auto_cleanup_unix_ms = Some(unix_ms());
                            status.last_error_message = Some(error);
                        });
                    }
                }
            }
            Err(error) => record_busy_skip(status, error),
        }
    }

    // Now that the heartbeat proved we are online, push this pass's autonomous cleanup proposals to
    // the server so they surface on mobile as pending approvals. Deliberately best-effort: a
    // failure here is counted but never stops the tick.
    if let Some(cleanup) = pending_auto_cleanup {
        let auto =
            crate::auto_proposal::submit_autonomous_proposals(&agent, &roots, &cleanup).await;
        activity.submitted_proposal_count += auto.submitted_count;
        if auto.failed_count > 0 {
            activity.had_error = true;
        }
        update_status(status, |status| {
            status.last_auto_submitted_proposal_count = auto.submitted_count;
        });
        if stop_if_agent_revoked(app, status, &agent) {
            return BackgroundTickOutcome::Stop;
        }
    }

    match work_limiter.try_scan() {
        Ok(_permit) => {
            match crate::command_processor::process_pending_commands(&agent, &roots, &outbox).await
            {
                Ok(report) => {
                    activity.processed_command_count += report.processed_count;
                    activity.submitted_proposal_count += report.submitted_proposal_count;
                    if report.failed_count > 0 {
                        activity.had_error = true;
                    }
                    update_status(status, |status| {
                        status.last_command_poll_unix_ms = Some(unix_ms());
                        status.last_command_count = report.inspected_count;
                        status.last_processed_command_count = report.processed_count;
                        status.last_submitted_proposal_count = report.submitted_proposal_count;
                        if report.failed_count > 0 {
                            status.last_error_message = Some(format!(
                                "{} command(s) failed during proposal processing",
                                report.failed_count
                            ));
                        }
                    });
                }
                Err(error) => {
                    activity.had_error = true;
                    update_status(status, |status| {
                        status.last_error_message = Some(error);
                    });
                }
            }
        }
        Err(error) => record_busy_skip(status, error),
    }
    if stop_if_agent_revoked(app, status, &agent) {
        return BackgroundTickOutcome::Stop;
    }

    match work_limiter.try_write() {
        Ok(_permit) => {
            match crate::execution_processor::process_pending_decisions(&agent, &roots, &outbox)
                .await
            {
                Ok(report) => {
                    activity.executed_item_count = report.executed_item_count;
                    activity.execution_failed_count = report.failed_count;
                    update_status(status, |status| {
                        status.last_decision_poll_unix_ms = Some(unix_ms());
                        status.last_decision_count = report.inspected_count;
                        status.last_executed_item_count = report.executed_item_count;
                        status.last_execution_failed_count = report.failed_count;
                        if report.failed_count > 0 {
                            status.last_error_message = Some(format!(
                                "{} decision(s) failed during execution",
                                report.failed_count
                            ));
                        }
                    });
                }
                Err(error) => {
                    activity.had_error = true;
                    update_status(status, |status| {
                        status.last_error_message = Some(error);
                    });
                }
            }
        }
        Err(error) => record_busy_skip(status, error),
    }
    if stop_if_agent_revoked(app, status, &agent) {
        return BackgroundTickOutcome::Stop;
    }

    if mode.runs_heavy_rest() {
        match work_limiter.try_scan() {
            Ok(_permit) => {
                match crate::watcher_lifecycle::reconcile_watched_root_indexes(
                    &roots,
                    &watchers,
                    |root_id| {
                        crate::cleanliness::reconcile_cleanliness_snapshot(app, root_id).map(|_| ())
                    },
                ) {
                    Ok(report) => {
                        if report.failed_root_count > 0 {
                            activity.had_error = true;
                        }
                        update_status(status, |status| {
                            status.last_file_index_reconcile_unix_ms = Some(unix_ms());
                            status.last_file_index_reconcile_root_count =
                                report.inspected_root_count;
                            status.last_file_index_reconcile_failed_count =
                                report.failed_root_count;
                            if report.failed_root_count > 0 {
                                status.last_error_message = Some(format!(
                                    "{} managed root index reconcile(s) failed",
                                    report.failed_root_count
                                ));
                            }
                        });
                    }
                    Err(error) => {
                        activity.had_error = true;
                        update_status(status, |status| {
                            status.last_error_message = Some(error);
                        });
                    }
                }
            }
            Err(error) => record_busy_skip(status, error),
        }
    }

    match work_limiter.try_scan() {
        Ok(_permit) => {
            match crate::file_browse_processor::process_pending_file_browse_requests(&agent, &roots)
                .await
            {
                Ok(report) => {
                    if report.failed_count > 0 {
                        activity.had_error = true;
                    }
                    update_status(status, |status| {
                        status.last_file_browse_poll_unix_ms = Some(unix_ms());
                        status.last_file_browse_count = report.inspected_count;
                        status.last_file_browse_completed_count = report.completed_count;
                        status.last_file_browse_failed_count = report.failed_count;
                        if report.failed_count > 0 {
                            status.last_error_message = Some(format!(
                                "{} file browse request(s) failed",
                                report.failed_count
                            ));
                        }
                    });
                }
                Err(error) => {
                    activity.had_error = true;
                    update_status(status, |status| {
                        status.last_error_message = Some(error);
                    });
                }
            }
        }
        Err(error) => record_busy_skip(status, error),
    }
    if stop_if_agent_revoked(app, status, &agent) {
        return BackgroundTickOutcome::Stop;
    }

    // Fast reconcile runs every 15 seconds and handles low-cost control-plane work.
    // File transfer and smart-cache discovery can touch storage/object APIs, so scheduled
    // polling keeps those on the 30-second full pass. Realtime wakeups still use a full pass
    // when the server tells us new agent work exists.
    if !mode.runs_heavy_rest() {
        flush_background_outbox(&agent, &outbox, status).await;
        if stop_if_agent_revoked(app, status, &agent) {
            return BackgroundTickOutcome::Stop;
        }
        emit_overlay_activity(app, activity);
        return BackgroundTickOutcome::Continue;
    }

    match work_limiter.try_transfer() {
        Ok(_permit) => {
            match crate::file_transfer_processor::process_pending_file_transfers(
                &agent, &roots, &outbox,
            )
            .await
            {
                Ok(report) => {
                    if report.failed_count > 0 {
                        activity.had_error = true;
                    }
                    update_status(status, |status| {
                        status.last_file_transfer_poll_unix_ms = Some(unix_ms());
                        status.last_file_transfer_count = report.inspected_count;
                        status.last_file_transfer_uploaded_count = report.uploaded_count;
                        status.last_file_transfer_failed_count = report.failed_count;
                        if report.failed_count > 0 {
                            status.last_error_message =
                                Some(format!("{} file transfer(s) failed", report.failed_count));
                        }
                    });
                }
                Err(error) => {
                    activity.had_error = true;
                    update_status(status, |status| {
                        status.last_error_message = Some(error);
                    });
                }
            }
        }
        Err(error) => record_busy_skip(status, error),
    }
    if stop_if_agent_revoked(app, status, &agent) {
        return BackgroundTickOutcome::Stop;
    }

    match work_limiter.try_transfer() {
        Ok(_permit) => {
            match crate::smart_cache_processor::process_smart_cache_for_enabled_rooms(
                &agent,
                &roots,
                &smart_cache,
                &outbox,
                25,
            )
            .await
            {
                Ok(report) => {
                    if report.failed_count > 0 {
                        activity.had_error = true;
                    }
                    update_status(status, |status| {
                        status.last_smart_cache_poll_unix_ms = Some(unix_ms());
                        status.last_smart_cache_candidate_count = report.inspected_count;
                        status.last_smart_cache_uploaded_count = report.uploaded_count;
                        status.last_smart_cache_failed_count = report.failed_count;
                        if report.failed_count > 0 {
                            status.last_error_message = Some(format!(
                                "{} smart cache item(s) failed",
                                report.failed_count
                            ));
                        }
                    });
                }
                Err(error) => {
                    activity.had_error = true;
                    update_status(status, |status| {
                        status.last_error_message = Some(error);
                    });
                }
            }
        }
        Err(error) => record_busy_skip(status, error),
    }
    if stop_if_agent_revoked(app, status, &agent) {
        return BackgroundTickOutcome::Stop;
    }

    // Deliver everything the command and decision passes just queued (plus any backlog from a
    // previous tick when the network was down). This runs last so a result enqueued moments ago
    // is sent in the same tick when the network is healthy.
    flush_background_outbox(&agent, &outbox, status).await;
    if stop_if_agent_revoked(app, status, &agent) {
        return BackgroundTickOutcome::Stop;
    }

    // Turn this whole pass into one character state for the overlay. This is best-effort: if no
    // overlay window is open it is silently skipped.
    emit_overlay_activity(app, activity);
    BackgroundTickOutcome::Continue
}

#[cfg(feature = "tauri-commands")]
async fn flush_background_outbox(
    agent: &crate::agent::AgentRuntime,
    outbox: &crate::storage::outbox::OutboxStore,
    status: &Arc<Mutex<BackgroundRuntimeStatus>>,
) {
    match crate::outbox_processor::flush_outbox(agent, outbox).await {
        Ok(report) => update_status(status, |status| {
            status.last_outbox_flush_unix_ms = Some(unix_ms());
            status.last_outbox_sent_count = report.sent_count;
            status.last_outbox_failed_count = report.failed_count;
            if report.failed_count > 0 {
                status.last_error_message = Some(format!(
                    "{} outbox item(s) permanently failed delivery",
                    report.failed_count
                ));
            }
        }),
        Err(error) => update_status(status, |status| {
            status.last_error_message = Some(error);
        }),
    }
}

#[cfg(feature = "tauri-commands")]
fn should_run_auto_cleanup(status: &Arc<Mutex<BackgroundRuntimeStatus>>) -> bool {
    let now = unix_ms();
    status
        .lock()
        .map(|status| {
            status
                .last_auto_cleanup_unix_ms
                .map(|last| now.saturating_sub(last) >= AUTO_CLEANUP_INTERVAL_MS)
                .unwrap_or(true)
        })
        .unwrap_or(false)
}

/// Emits the derived character state to the overlay window, if one is open. The overlay bridge is
/// deliberately one-way (state out) and never routes back into any file-operation path.
#[cfg(feature = "tauri-commands")]
fn emit_overlay_activity(app: &tauri::AppHandle, activity: crate::overlay::OverlayActivity) {
    use tauri::Manager;

    let overlay = app.state::<crate::overlay::OverlayRuntime>();
    let event = crate::overlay::character_event_for(&activity);
    crate::commands::overlay::emit_character_event_if_open(app, &overlay, &event);
}

#[cfg(feature = "tauri-commands")]
fn stop_if_agent_revoked(
    app: &tauri::AppHandle,
    status: &Arc<Mutex<BackgroundRuntimeStatus>>,
    agent: &crate::agent::AgentRuntime,
) -> bool {
    if !matches!(
        agent.connection_status().state,
        crate::agent::AgentConnectionState::Revoked
    ) {
        return false;
    }

    mark_suspended(status, DEVICE_REVOKED_REASON);
    emit_overlay_activity(
        app,
        crate::overlay::OverlayActivity {
            had_error: true,
            ..crate::overlay::OverlayActivity::default()
        },
    );
    true
}

#[cfg(feature = "tauri-commands")]
async fn replay_events(
    app: &tauri::AppHandle,
    agent: &crate::agent::AgentRuntime,
    sync_store: &crate::storage::agent_sync::AgentSyncStore,
    roots: &crate::storage::managed_roots::ManagedRootStore,
    watchers: &crate::storage::watchers::WatcherStore,
    device_id: &str,
) -> Result<ReplayOutcome, String> {
    use tauri::Emitter;

    let previous_cursor = sync_store.cursor(device_id)?;
    let events = agent
        .replay_events(previous_cursor, 100)
        .await
        .map_err(|error| error.to_string())?;
    for event in &events {
        if event.event_type == "room.removed" {
            let room_id = event.room_id.as_deref().or_else(|| {
                (event.aggregate_type == "room").then_some(event.aggregate_id.as_str())
            });
            if let Some(room_id) = room_id {
                if let Some(report) =
                    crate::commands::agent::apply_local_room_detached(room_id, roots, watchers)?
                {
                    let _ = app.emit("managed-root-binding-changed", report.root_id);
                }
            }
        }
        if event.event_type == "device.revoked"
            && (event.device_id.as_deref() == Some(device_id)
                || (event.aggregate_type == "device" && event.aggregate_id == device_id))
        {
            crate::commands::agent::apply_local_device_revoked(agent, sync_store, roots, watchers)
                .await?;
            let _ = app.emit("desktop-device-revoked", device_id);
            return Ok(ReplayOutcome::DeviceRevoked);
        }
    }
    if let Some(next_cursor) = events.last().map(|event| event.sequence) {
        sync_store.advance(device_id, next_cursor).await?;
    }
    Ok(ReplayOutcome::Continue)
}

#[cfg(feature = "tauri-commands")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReplayOutcome {
    Continue,
    DeviceRevoked,
}

#[cfg(feature = "tauri-commands")]
fn mark_suspended(status: &Arc<Mutex<BackgroundRuntimeStatus>>, reason: &str) {
    update_status(status, |status| {
        status.state = BackgroundRuntimeState::Suspended;
        status.last_stopped_unix_ms = Some(unix_ms());
        status.last_error_message = Some(reason.to_string());
    });
}

#[cfg(feature = "tauri-commands")]
fn record_busy_skip(status: &Arc<Mutex<BackgroundRuntimeStatus>>, message: String) {
    update_status(status, |status| {
        status.last_error_message = Some(message);
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
    #[cfg(feature = "tauri-commands")]
    use super::{scheduled_tick_mode, BackgroundTickMode};
    use super::{BackgroundRuntime, BackgroundRuntimeState};

    #[cfg(feature = "tauri-commands")]
    #[test]
    fn scheduled_ticks_keep_heartbeats_frequent_and_split_rest_reconcile() {
        assert_eq!(scheduled_tick_mode(1), BackgroundTickMode::HeartbeatOnly);
        assert_eq!(scheduled_tick_mode(2), BackgroundTickMode::HeartbeatOnly);
        assert_eq!(
            scheduled_tick_mode(3),
            BackgroundTickMode::HeartbeatAndFastReconcile
        );
        assert!(scheduled_tick_mode(3).sends_heartbeat());
        assert!(scheduled_tick_mode(3).reconciles());
        assert!(!scheduled_tick_mode(3).runs_heavy_rest());
        assert_eq!(
            scheduled_tick_mode(6),
            BackgroundTickMode::HeartbeatAndFullReconcile
        );
        assert!(scheduled_tick_mode(6).sends_heartbeat());
        assert!(scheduled_tick_mode(6).reconciles());
        assert!(scheduled_tick_mode(6).runs_heavy_rest());
    }

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
        assert_eq!(status.last_processed_command_count, 0);
        assert_eq!(status.last_submitted_proposal_count, 0);
        assert_eq!(status.last_decision_count, 0);
        assert_eq!(status.last_executed_item_count, 0);
        assert_eq!(status.last_execution_failed_count, 0);
        assert!(status.last_heartbeat_unix_ms.is_none());
        assert!(status.last_decision_poll_unix_ms.is_none());
        assert!(status.last_realtime_signal_unix_ms.is_none());
        assert!(status.last_file_browse_poll_unix_ms.is_none());
        assert_eq!(status.last_file_browse_count, 0);
        assert_eq!(status.last_file_browse_completed_count, 0);
        assert_eq!(status.last_file_browse_failed_count, 0);
        assert!(status.last_file_index_reconcile_unix_ms.is_none());
        assert_eq!(status.last_file_index_reconcile_root_count, 0);
        assert_eq!(status.last_file_index_reconcile_failed_count, 0);
        assert!(status.last_file_transfer_poll_unix_ms.is_none());
        assert_eq!(status.last_file_transfer_count, 0);
        assert_eq!(status.last_file_transfer_uploaded_count, 0);
        assert_eq!(status.last_file_transfer_failed_count, 0);
        assert!(status.last_outbox_flush_unix_ms.is_none());
        assert_eq!(status.last_outbox_sent_count, 0);
        assert_eq!(status.last_outbox_failed_count, 0);
    }
}
