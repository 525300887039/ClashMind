use tauri::AppHandle;

use crate::core::sidecar::{self, AiSidecarError, AiSidecarState};

#[tauri::command]
pub async fn start_ai_service(
    app: AppHandle,
    state: tauri::State<'_, AiSidecarState>,
) -> Result<(), AiSidecarError> {
    sidecar::start_ai(&app, &state).await
}

#[tauri::command]
pub fn stop_ai_service(state: tauri::State<'_, AiSidecarState>) -> Result<(), AiSidecarError> {
    sidecar::stop_ai(&state)
}

#[tauri::command]
pub fn get_ai_status(state: tauri::State<'_, AiSidecarState>) -> Result<bool, AiSidecarError> {
    Ok(sidecar::is_ai_running(&state))
}

#[tauri::command]
pub async fn ai_ping(
    state: tauri::State<'_, AiSidecarState>,
) -> Result<serde_json::Value, AiSidecarError> {
    sidecar::send_rpc(&state, "ping", None).await
}
