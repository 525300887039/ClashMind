use std::collections::HashMap;

use crate::core::mihomo::MihomoError;

use super::MihomoState;

#[tauri::command]
pub async fn get_proxies(
    state: tauri::State<'_, MihomoState>,
) -> Result<serde_json::Value, MihomoError> {
    state.client.get_proxies().await
}

#[tauri::command]
pub async fn switch_proxy(
    state: tauri::State<'_, MihomoState>,
    group: String,
    name: String,
) -> Result<(), MihomoError> {
    state.client.switch_proxy(&group, &name).await
}

#[tauri::command]
pub async fn test_delay(
    state: tauri::State<'_, MihomoState>,
    name: String,
    url: String,
    timeout: u32,
) -> Result<u32, MihomoError> {
    state.client.test_delay(&name, &url, timeout).await
}

#[tauri::command]
pub async fn test_group_delay(
    state: tauri::State<'_, MihomoState>,
    group: String,
    url: String,
    timeout: u32,
) -> Result<HashMap<String, u32>, MihomoError> {
    state.client.test_group_delay(&group, &url, timeout).await
}

#[tauri::command]
pub async fn get_rules(
    state: tauri::State<'_, MihomoState>,
) -> Result<serde_json::Value, MihomoError> {
    state.client.get_rules().await
}
