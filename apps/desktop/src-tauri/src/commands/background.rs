use crate::background::{BackgroundRuntime, BackgroundRuntimeStatus};

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub fn get_background_runtime_status(
    runtime: tauri::State<'_, BackgroundRuntime>,
) -> Result<BackgroundRuntimeStatus, String> {
    get_background_runtime_status_impl(&runtime)
}

#[cfg(not(feature = "tauri-commands"))]
pub fn get_background_runtime_status(
    runtime: &BackgroundRuntime,
) -> Result<BackgroundRuntimeStatus, String> {
    get_background_runtime_status_impl(runtime)
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub fn start_background_runtime(
    app: tauri::AppHandle,
    runtime: tauri::State<'_, BackgroundRuntime>,
) -> Result<BackgroundRuntimeStatus, String> {
    runtime.start(app)
}

#[cfg(feature = "tauri-commands")]
#[tauri::command]
pub fn pause_background_runtime(
    runtime: tauri::State<'_, BackgroundRuntime>,
) -> Result<BackgroundRuntimeStatus, String> {
    pause_background_runtime_impl(&runtime)
}

#[cfg(not(feature = "tauri-commands"))]
pub fn pause_background_runtime(
    runtime: &BackgroundRuntime,
) -> Result<BackgroundRuntimeStatus, String> {
    pause_background_runtime_impl(runtime)
}

fn get_background_runtime_status_impl(
    runtime: &BackgroundRuntime,
) -> Result<BackgroundRuntimeStatus, String> {
    runtime.status()
}

fn pause_background_runtime_impl(
    runtime: &BackgroundRuntime,
) -> Result<BackgroundRuntimeStatus, String> {
    runtime.pause("paused by user")
}

// These tests exercise the `not(feature = "tauri-commands")` variants above, which take a plain
// `&BackgroundRuntime` so they are callable without a live Tauri `State`. The `tauri-commands`
// variants need a running app to construct their `State` argument and are not unit-testable this
// way, so this module only compiles for the CLI/no-tauri-commands build.
#[cfg(all(test, not(feature = "tauri-commands")))]
mod tests {
    use crate::background::{BackgroundRuntime, BackgroundRuntimeState};

    use super::{get_background_runtime_status, pause_background_runtime};

    #[test]
    fn status_reports_runtime_state() {
        let runtime = BackgroundRuntime::default();

        let status = get_background_runtime_status(&runtime).expect("status");

        assert_eq!(status.state, BackgroundRuntimeState::Stopped);
    }

    #[test]
    fn pause_command_suspends_runtime() {
        let runtime = BackgroundRuntime::default();

        let status = pause_background_runtime(&runtime).expect("pause");

        assert_eq!(status.state, BackgroundRuntimeState::Suspended);
    }
}
