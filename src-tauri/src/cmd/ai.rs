use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_plugin_sql::DbPool;
use thiserror::Error;

use crate::{
    core::{
        mihomo::MihomoError,
        sidecar::{self, AiSidecarError, AiSidecarState},
    },
    db::{
        self,
        repo_conversation::ConversationRepo,
        repo_snapshot::{ConfigSnapshot, SnapshotRepo},
        DbError,
    },
    utils::{geoip::GeoIpConfigState, path::expand_tilde},
};

use super::MihomoState;

const AI_SNAPSHOT_DESCRIPTION: &str = "AI 配置变更前自动备份";
const DEFAULT_MANUAL_SNAPSHOT_DESCRIPTION: &str = "手动快照";

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

impl AiChatRole {
    fn as_str(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::System => "system",
        }
    }
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveConversationMessageParams {
    pub role: AiChatRole,
    pub content: String,
    pub tool_calls: Option<serde_json::Value>,
    pub tokens_used: Option<i32>,
    pub model: Option<String>,
}

#[derive(Error, Debug)]
pub enum AiConfigChangeError {
    #[error("{0}")]
    Database(#[from] DbError),
    #[error("读取当前运行配置失败: {0}")]
    ReadRuntimeFailed(String),
    #[error("读取配置文件失败: {0}")]
    ReadConfigFailed(String),
    #[error("写入配置失败: {0}")]
    WriteFailed(String),
    #[error("序列化对话工具调用失败: {0}")]
    SerializeConversationFailed(String),
    #[error("解析 AI 变更基线失败: {0}")]
    InvalidOriginalConfig(String),
    #[error("当前运行配置已经发生变化，请重新生成 Diff 后再试")]
    StaleBase,
    #[error("找不到配置快照 #{0}")]
    SnapshotNotFound(i64),
    #[error("配置快照 #{0} 缺少原始文件路径，无法安全恢复")]
    SnapshotPathMissing(i64),
    #[error("配置快照 #{snapshot_id} 属于 {snapshot_path}，当前活动配置为 {active_path}，拒绝跨配置文件恢复")]
    SnapshotPathMismatch {
        snapshot_id: i64,
        snapshot_path: String,
        active_path: String,
    },
    #[error("热重载失败，已自动回滚: {0}")]
    ReloadFailedRolledBack(String),
    #[error("热重载失败，且回滚失败: {reload_error}; 回滚失败: {rollback_error}")]
    ReloadFailedRollbackFailed {
        reload_error: String,
        rollback_error: String,
    },
}

impl Serialize for AiConfigChangeError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

fn active_config_file_path(geoip_config: &GeoIpConfigState) -> PathBuf {
    Path::new(&expand_tilde(&geoip_config.config_dir())).join("config.yaml")
}

fn normalize_command_path(file_path: &str) -> PathBuf {
    PathBuf::from(expand_tilde(file_path))
}

fn normalize_line_endings(value: &str) -> String {
    value.replace("\r\n", "\n")
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn snapshot_file_path(config_file: &Path) -> String {
    config_file.to_string_lossy().into_owned()
}

fn normalized_path_key(path: &Path) -> String {
    let normalized = path.to_string_lossy().replace('\\', "/");

    #[cfg(windows)]
    {
        normalized.to_lowercase()
    }

    #[cfg(not(windows))]
    {
        normalized
    }
}

fn resolve_snapshot_target_path(
    geoip_config: &GeoIpConfigState,
    file_path: Option<&str>,
) -> PathBuf {
    match file_path {
        Some(file_path) if !file_path.trim().is_empty() => normalize_command_path(file_path.trim()),
        _ => active_config_file_path(geoip_config),
    }
}

async fn read_runtime_config(
    mihomo_state: &MihomoState,
) -> Result<serde_json::Value, AiConfigChangeError> {
    mihomo_state
        .client
        .lock()
        .await
        .get_configs()
        .await
        .map_err(|error| AiConfigChangeError::ReadRuntimeFailed(error.to_string()))
}

async fn read_config_file(config_file: &Path) -> Result<String, AiConfigChangeError> {
    tokio::fs::read_to_string(config_file)
        .await
        .map_err(|error| AiConfigChangeError::ReadConfigFailed(error.to_string()))
}

async fn create_snapshot_with_cleanup(
    db: &DbPool,
    config_file: &Path,
    content: &str,
    source: &str,
    description: Option<&str>,
) -> Result<i64, AiConfigChangeError> {
    let file_path = snapshot_file_path(config_file);
    let snapshot_id =
        SnapshotRepo::create(db, content, source, description, Some(file_path.as_str())).await?;
    let _deleted = SnapshotRepo::cleanup(db).await?;

    Ok(snapshot_id)
}

async fn restore_config_and_reload(
    config_file: &Path,
    original_config: &str,
    mihomo_state: &MihomoState,
    reload_error: MihomoError,
) -> Result<(), AiConfigChangeError> {
    tokio::fs::write(config_file, original_config)
        .await
        .map_err(|error| AiConfigChangeError::ReloadFailedRollbackFailed {
            reload_error: reload_error.to_string(),
            rollback_error: error.to_string(),
        })?;

    mihomo_state
        .client
        .lock()
        .await
        .reload_configs()
        .await
        .map_err(|error| AiConfigChangeError::ReloadFailedRollbackFailed {
            reload_error: reload_error.to_string(),
            rollback_error: error.to_string(),
        })?;

    Err(AiConfigChangeError::ReloadFailedRolledBack(
        reload_error.to_string(),
    ))
}

async fn write_config_and_reload(
    config_file: &Path,
    next_config: &str,
    rollback_config: &str,
    mihomo_state: &MihomoState,
) -> Result<(), AiConfigChangeError> {
    let normalized_next_config = normalize_line_endings(next_config);

    tokio::fs::write(config_file, normalized_next_config)
        .await
        .map_err(|error| AiConfigChangeError::WriteFailed(error.to_string()))?;

    match mihomo_state.client.lock().await.reload_configs().await {
        Ok(()) => Ok(()),
        Err(error) => {
            restore_config_and_reload(config_file, rollback_config, mihomo_state, error).await
        }
    }
}

async fn load_snapshot(db: &DbPool, id: i64) -> Result<ConfigSnapshot, AiConfigChangeError> {
    let snapshot = SnapshotRepo::get_by_id(db, id).await?;
    snapshot.ok_or(AiConfigChangeError::SnapshotNotFound(id))
}

fn validate_snapshot_path(
    snapshot: &ConfigSnapshot,
    active_config_file: &Path,
) -> Result<(), AiConfigChangeError> {
    let snapshot_path = snapshot
        .file_path
        .as_deref()
        .ok_or(AiConfigChangeError::SnapshotPathMissing(snapshot.id))?;
    let snapshot_path = normalize_command_path(snapshot_path);

    if normalized_path_key(&snapshot_path) == normalized_path_key(active_config_file) {
        return Ok(());
    }

    Err(AiConfigChangeError::SnapshotPathMismatch {
        snapshot_id: snapshot.id,
        snapshot_path: snapshot_file_path(&snapshot_path),
        active_path: snapshot_file_path(active_config_file),
    })
}

fn serialize_tool_calls(
    tool_calls: Option<serde_json::Value>,
) -> Result<Option<String>, AiConfigChangeError> {
    tool_calls
        .filter(|value| !value.is_null())
        .map(|value| {
            serde_json::to_string(&value).map_err(|error| {
                AiConfigChangeError::SerializeConversationFailed(error.to_string())
            })
        })
        .transpose()
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

#[tauri::command]
pub async fn apply_config_change(
    app_handle: AppHandle,
    geoip_config: tauri::State<'_, GeoIpConfigState>,
    mihomo_state: tauri::State<'_, MihomoState>,
    original_config: String,
    modified_config: String,
) -> Result<(), AiConfigChangeError> {
    let config_file = active_config_file_path(&geoip_config);
    let current_runtime_config = read_runtime_config(&mihomo_state).await?;
    let original_config_value = serde_yaml::from_str::<serde_json::Value>(&original_config)
        .map_err(|error| AiConfigChangeError::InvalidOriginalConfig(error.to_string()))?;

    if current_runtime_config != original_config_value {
        return Err(AiConfigChangeError::StaleBase);
    }

    let current_file_content = read_config_file(&config_file).await?;
    let db = db::get_db_pool(&app_handle).await?;
    let _snapshot_id = create_snapshot_with_cleanup(
        &db,
        &config_file,
        &current_file_content,
        "ai",
        Some(AI_SNAPSHOT_DESCRIPTION),
    )
    .await?;

    write_config_and_reload(
        &config_file,
        &modified_config,
        &current_file_content,
        &mihomo_state,
    )
    .await
}

#[tauri::command]
pub fn reject_config_change() -> Result<(), AiConfigChangeError> {
    Ok(())
}

#[tauri::command]
pub async fn list_snapshots(
    app_handle: AppHandle,
    limit: i32,
) -> Result<Vec<ConfigSnapshot>, AiConfigChangeError> {
    let db = db::get_db_pool(&app_handle).await?;
    SnapshotRepo::list(&db, limit, 0).await.map_err(Into::into)
}

#[tauri::command]
pub async fn create_snapshot(
    app_handle: AppHandle,
    geoip_config: tauri::State<'_, GeoIpConfigState>,
    description: Option<String>,
    file_path: Option<String>,
) -> Result<i64, AiConfigChangeError> {
    let config_file = resolve_snapshot_target_path(&geoip_config, file_path.as_deref());
    let current_content = read_config_file(&config_file).await?;
    let db = db::get_db_pool(&app_handle).await?;
    let description = normalize_optional_text(description);
    let description = description
        .as_deref()
        .or(Some(DEFAULT_MANUAL_SNAPSHOT_DESCRIPTION));

    create_snapshot_with_cleanup(&db, &config_file, &current_content, "manual", description).await
}

#[tauri::command]
pub async fn restore_snapshot(
    app_handle: AppHandle,
    geoip_config: tauri::State<'_, GeoIpConfigState>,
    mihomo_state: tauri::State<'_, MihomoState>,
    id: i64,
) -> Result<(), AiConfigChangeError> {
    let db = db::get_db_pool(&app_handle).await?;
    let snapshot = load_snapshot(&db, id).await?;
    let config_file = active_config_file_path(&geoip_config);
    validate_snapshot_path(&snapshot, &config_file)?;
    let current_content = read_config_file(&config_file).await?;
    let backup_description = format!("恢复快照前自动备份 #{id}");

    let _backup_snapshot_id = create_snapshot_with_cleanup(
        &db,
        &config_file,
        &current_content,
        "manual",
        Some(&backup_description),
    )
    .await?;

    write_config_and_reload(
        &config_file,
        &snapshot.content,
        &current_content,
        &mihomo_state,
    )
    .await
}

#[tauri::command]
pub async fn save_conversation_message(
    app_handle: AppHandle,
    params: SaveConversationMessageParams,
) -> Result<i64, AiConfigChangeError> {
    let db = db::get_db_pool(&app_handle).await?;
    let tool_calls = serialize_tool_calls(params.tool_calls)?;
    let model = normalize_optional_text(params.model);
    let message_id = ConversationRepo::save_message(
        &db,
        params.role.as_str(),
        &params.content,
        tool_calls.as_deref(),
        params.tokens_used,
        model.as_deref(),
    )
    .await?;
    let _deleted = ConversationRepo::cleanup(&db).await?;

    Ok(message_id)
}
