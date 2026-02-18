use crate::core::mihomo::MihomoError;

use super::MihomoState;

#[tauri::command]
pub async fn get_version(
    state: tauri::State<'_, MihomoState>,
) -> Result<serde_json::Value, MihomoError> {
    state.client.get_version().await
}

#[tauri::command]
pub async fn close_connection(
    state: tauri::State<'_, MihomoState>,
    id: String,
) -> Result<(), MihomoError> {
    state.client.close_connection(&id).await
}

#[tauri::command]
pub async fn close_all_connections(
    state: tauri::State<'_, MihomoState>,
) -> Result<(), MihomoError> {
    state.client.close_all_connections().await
}

#[tauri::command]
pub async fn get_connections(
    state: tauri::State<'_, MihomoState>,
) -> Result<serde_json::Value, MihomoError> {
    state.client.get_connections().await
}
