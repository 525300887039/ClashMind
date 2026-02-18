use tauri::AppHandle;

use crate::core::sidecar::{self, SidecarError, SidecarState};

#[tauri::command]
pub fn start_mihomo(
    app: AppHandle,
    state: tauri::State<'_, SidecarState>,
    config_path: String,
) -> Result<(), SidecarError> {
    sidecar::start(&app, &state, &config_path)
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
    sidecar::restart(&app, &state, &config_path)
}

#[tauri::command]
pub fn get_mihomo_status(state: tauri::State<'_, SidecarState>) -> Result<bool, SidecarError> {
    Ok(sidecar::is_running(&state))
}
