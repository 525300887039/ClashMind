use crate::core::mihomo::MihomoError;

use super::MihomoState;

#[tauri::command]
pub async fn get_configs(
    state: tauri::State<'_, MihomoState>,
) -> Result<serde_json::Value, MihomoError> {
    state.client.get_configs().await
}

#[tauri::command]
pub async fn patch_configs(
    state: tauri::State<'_, MihomoState>,
    payload: serde_json::Value,
) -> Result<(), MihomoError> {
    state.client.patch_configs(payload).await
}
