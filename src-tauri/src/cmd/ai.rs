use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::core::sidecar::{self, AiSidecarError, AiSidecarState};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AiProviderKind {
    Openai,
    Claude,
    Deepseek,
    Ollama,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AiChatRole {
    User,
    Assistant,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiProviderSettings {
    pub provider: AiProviderKind,
    pub model: String,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub temperature: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiChatMessage {
    pub role: AiChatRole,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiChatContext {
    pub current_config: Option<String>,
    pub recent_stats: Option<serde_json::Value>,
    pub available_proxies: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiChatParams {
    pub messages: Vec<AiChatMessage>,
    pub context: Option<AiChatContext>,
    pub settings: AiProviderSettings,
}

#[tauri::command]
pub async fn start_ai_service(
    app: AppHandle,
    state: tauri::State<'_, AiSidecarState>,
) -> Result<(), AiSidecarError> {
    sidecar::start_ai(&app, &state).await
}

#[tauri::command]
pub fn stop_ai_service(
    app: AppHandle,
    state: tauri::State<'_, AiSidecarState>,
) -> Result<(), AiSidecarError> {
    sidecar::stop_ai(Some(&app), &state)
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

#[tauri::command]
pub fn ai_chat(
    state: tauri::State<'_, AiSidecarState>,
    params: AiChatParams,
) -> Result<(), AiSidecarError> {
    if params.messages.is_empty() {
        return Err(AiSidecarError::InvalidResponse(
            "chat messages must not be empty".to_string(),
        ));
    }

    if params.settings.model.trim().is_empty() {
        return Err(AiSidecarError::InvalidResponse(
            "chat model must not be empty".to_string(),
        ));
    }

    let payload = serde_json::to_value(params)
        .map_err(|error| AiSidecarError::InvalidResponse(error.to_string()))?;

    sidecar::send_streaming_rpc(&state, "chat", Some(payload))
}
