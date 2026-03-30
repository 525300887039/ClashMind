use tauri::AppHandle;

use crate::core::sidecar::{self, SidecarError, SidecarState};

/// Abort old log/traffic tasks and start fresh subscriptions.
/// Each mutex is locked only once: abort the old handle and store the new one
/// under the same guard to avoid a race window between release and re-acquire.
fn restart_subscriptions(app: AppHandle, state: &SidecarState) -> Result<(), SidecarError> {
    let mut log_guard = state.log_task.lock().map_err(|e| SidecarError::SpawnFailed(e.to_string()))?;
    if let Some(h) = log_guard.take() {
        h.abort();
    }
    *log_guard = Some(crate::core::logs::start_log_subscription(app.clone()));

    let mut traffic_guard = state.traffic_task.lock().map_err(|e| SidecarError::SpawnFailed(e.to_string()))?;
    if let Some(h) = traffic_guard.take() {
        h.abort();
    }
    *traffic_guard = Some(crate::core::traffic::start_traffic_subscription(app));

    Ok(())
}

#[tauri::command]
pub fn start_mihomo(
    app: AppHandle,
    state: tauri::State<'_, SidecarState>,
    config_path: String,
) -> Result<(), SidecarError> {
    sidecar::start(&app, &state, &config_path)?;
    restart_subscriptions(app, &state)
}

#[tauri::command]
pub fn stop_mihomo(state: tauri::State<'_, SidecarState>) -> Result<(), SidecarError> {
    sidecar::stop(&state)
}

#[tauri::command]
pub fn restart_mihomo(
    app: AppHandle,
    state: tauri::State<'_, SidecarState>,
    config_path: String,
) -> Result<(), SidecarError> {
    sidecar::restart(&app, &state, &config_path)?;
    restart_subscriptions(app, &state)
}

#[tauri::command]
pub fn get_mihomo_status(state: tauri::State<'_, SidecarState>) -> Result<bool, SidecarError> {
    Ok(sidecar::is_running(&state))
}

/// Check if a mihomo config directory has a valid config.yaml with external-controller
#[tauri::command]
pub fn check_config_exists(config_path: String) -> Result<bool, SidecarError> {
    let config_file = std::path::Path::new(&config_path).join("config.yaml");
    if !config_file.exists() {
        return Ok(false);
    }
    let content = std::fs::read_to_string(&config_file)
        .map_err(|e| SidecarError::SpawnFailed(e.to_string()))?;
    Ok(content.contains("external-controller"))
}

/// Create default config directory and config.yaml if they don't exist
#[tauri::command]
pub fn ensure_default_config(config_path: String) -> Result<(), SidecarError> {
    let dir = std::path::Path::new(&config_path);
    if !dir.exists() {
        std::fs::create_dir_all(dir)
            .map_err(|e| SidecarError::SpawnFailed(e.to_string()))?;
    }
    let config_file = dir.join("config.yaml");
    let needs_default = if config_file.exists() {
        let content = std::fs::read_to_string(&config_file)
            .map_err(|e| SidecarError::SpawnFailed(e.to_string()))?;
        !content.contains("external-controller")
    } else {
        true
    };
    if needs_default {
        let default_config = "mixed-port: 7890\nexternal-controller: 127.0.0.1:9090\n";
        std::fs::write(&config_file, default_config)
            .map_err(|e| SidecarError::SpawnFailed(e.to_string()))?;
    }
    Ok(())
}
