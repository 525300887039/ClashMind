use crate::core::mihomo::MihomoError;
use crate::core::sysproxy::{self, SysproxyError};

use super::MihomoState;

#[tauri::command]
pub async fn get_version(
    state: tauri::State<'_, MihomoState>,
) -> Result<serde_json::Value, MihomoError> {
    state.client.lock().await.get_version().await
}

#[tauri::command]
pub async fn close_connection(
    state: tauri::State<'_, MihomoState>,
    id: String,
) -> Result<(), MihomoError> {
    state.client.lock().await.close_connection(&id).await
}

#[tauri::command]
pub async fn close_all_connections(
    state: tauri::State<'_, MihomoState>,
) -> Result<(), MihomoError> {
    state.client.lock().await.close_all_connections().await
}

#[tauri::command]
pub async fn get_connections(
    state: tauri::State<'_, MihomoState>,
) -> Result<serde_json::Value, MihomoError> {
    state.client.lock().await.get_connections().await
}

#[tauri::command]
pub fn set_system_proxy(enable: bool, port: u16) -> Result<(), SysproxyError> {
    sysproxy::set_system_proxy(enable, "127.0.0.1", port)
}

#[tauri::command]
pub fn get_system_proxy() -> Result<serde_json::Value, SysproxyError> {
    let proxy = sysproxy::get_system_proxy()?;
    Ok(serde_json::json!({
        "enable": proxy.enable,
        "host": proxy.host,
        "port": proxy.port,
    }))
}

#[tauri::command]
pub async fn update_mihomo_client(
    state: tauri::State<'_, MihomoState>,
    base_url: String,
    secret: String,
) -> Result<(), MihomoError> {
    state.client.lock().await.update_connection(&base_url, &secret);
    Ok(())
}
