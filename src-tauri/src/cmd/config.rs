use crate::core::mihomo::MihomoError;
use serde::Serialize;
use thiserror::Error;

use super::MihomoState;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("读取配置失败: {0}")]
    ReadFailed(#[from] std::io::Error),
    #[error("重载配置失败: {0}")]
    ReloadFailed(#[from] MihomoError),
}

impl Serialize for ConfigError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

fn expand_tilde(path: &str) -> String {
    if let Some(rest) = path.strip_prefix('~') {
        if let Ok(home) = std::env::var("HOME") {
            return format!("{home}{rest}");
        }
    }
    path.to_string()
}

#[tauri::command]
pub async fn read_config(path: String) -> Result<String, ConfigError> {
    let real_path = expand_tilde(&path);
    Ok(tokio::fs::read_to_string(real_path).await?)
}

#[tauri::command]
pub async fn write_config(path: String, content: String) -> Result<(), ConfigError> {
    let real_path = expand_tilde(&path);
    Ok(tokio::fs::write(real_path, content).await?)
}

#[tauri::command]
pub async fn reload_config(mihomo_url: String) -> Result<(), ConfigError> {
    let client = crate::core::mihomo::MihomoClient::new(&format!("http://{mihomo_url}"), "");
    Ok(client.reload_configs().await?)
}

#[tauri::command]
pub async fn get_configs(
    state: tauri::State<'_, MihomoState>,
) -> Result<serde_json::Value, MihomoError> {
    state.client.lock().await.get_configs().await
}

#[tauri::command]
pub async fn patch_configs(
    state: tauri::State<'_, MihomoState>,
    payload: serde_json::Value,
) -> Result<(), MihomoError> {
    state.client.lock().await.patch_configs(payload).await
}
