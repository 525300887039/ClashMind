use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{Duration as ChronoDuration, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tauri_plugin_sql::DbPool;
use thiserror::Error;

use crate::utils::impl_serialize_display;

use crate::{
    core::{
        mihomo::MihomoError,
        sidecar::{self, AiSidecarError, AiSidecarState},
    },
    db::{
        self, repo_connection,
        repo_conversation::ConversationRepo,
        repo_domain,
        repo_snapshot::{ConfigSnapshot, SnapshotRepo},
        repo_traffic, DbError,
    },
    utils::{geoip::GeoIpConfigState, path::expand_tilde},
};

use super::MihomoState;

const AI_SNAPSHOT_DESCRIPTION: &str = "AI 配置变更前自动备份";
const DEFAULT_MANUAL_SNAPSHOT_DESCRIPTION: &str = "手动快照";
const REPORT_RPC_TIMEOUT: Duration = Duration::from_secs(120);
const AI_SETTINGS_FILE_NAME: &str = "ai-settings.json";

const LEGACY_DEEPSEEK_BASE_URL: &str = "https://api.deepseek.com/v1";
const LEGACY_OLLAMA_OPENAI_BASE_URL: &str = "http://127.0.0.1:11434/v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AiProviderKind {
    Openai,
    OpenaiCompatible,
    Claude,
    Gemini,
}

impl AiProviderKind {
    fn default_model(&self) -> &'static str {
        match self {
            Self::Openai => "gpt-4o-mini",
            Self::OpenaiCompatible => "",
            Self::Claude => "claude-sonnet-4-5",
            Self::Gemini => "gemini-2.5-flash",
        }
    }

    fn requires_api_key(&self) -> bool {
        matches!(self, Self::Openai | Self::Claude | Self::Gemini)
    }

    fn requires_base_url(&self) -> bool {
        matches!(self, Self::OpenaiCompatible)
    }
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
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiModelCatalogSettings {
    pub provider: AiProviderKind,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AiSettings {
    pub provider: AiProviderKind,
    pub model: String,
    pub api_key: String,
    pub base_url: String,
    pub temperature: f64,
    pub max_tokens: u32,
    pub auto_start: bool,
}

impl Default for AiSettings {
    fn default() -> Self {
        let provider = AiProviderKind::Openai;
        Self {
            model: provider.default_model().to_string(),
            provider,
            api_key: String::new(),
            base_url: String::new(),
            temperature: 0.3,
            max_tokens: 4096,
            auto_start: false,
        }
    }
}

fn non_empty_string(value: &str) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

impl AiSettings {
    fn normalized(mut self) -> Self {
        self.api_key = self.api_key.trim().to_string();
        self.base_url = self.base_url.trim().to_string();
        self.model = self.model.trim().to_string();
        self.temperature = if self.temperature.is_finite() {
            self.temperature.clamp(0.0, 1.0)
        } else {
            Self::default().temperature
        };
        self.max_tokens = self.max_tokens.max(1);

        if self.model.is_empty() {
            self.model = self.provider.default_model().to_string();
        }

        self
    }

    fn validate_for_provider_request(&self) -> Result<(), AiSettingsError> {
        if self.model.is_empty() {
            return Err(AiSettingsError::InvalidSettings(
                "AI 模型不能为空".to_string(),
            ));
        }

        if self.provider.requires_api_key() && self.api_key.is_empty() {
            return Err(AiSettingsError::InvalidSettings(
                "当前 Provider 需要 API Key".to_string(),
            ));
        }

        if self.provider.requires_base_url() && self.base_url.is_empty() {
            return Err(AiSettingsError::InvalidSettings(
                "当前 Provider 需要 Base URL".to_string(),
            ));
        }

        Ok(())
    }

    fn to_provider_settings(&self) -> AiProviderSettings {
        AiProviderSettings {
            provider: self.provider.clone(),
            model: self.model.clone(),
            api_key: non_empty_string(&self.api_key),
            base_url: non_empty_string(&self.base_url),
            temperature: Some(self.temperature),
            max_tokens: Some(self.max_tokens),
        }
    }

    fn to_model_catalog_settings(&self) -> AiModelCatalogSettings {
        AiModelCatalogSettings {
            provider: self.provider.clone(),
            api_key: non_empty_string(&self.api_key),
            base_url: non_empty_string(&self.base_url),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AiConnectionTestResult {
    pub success: bool,
    pub latency_ms: u64,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AiModelCatalogSource {
    Remote,
    Fallback,
    Empty,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AiModelCatalog {
    pub models: Vec<String>,
    pub source: AiModelCatalogSource,
    pub message: String,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ReportType {
    Daily,
    Weekly,
}

impl ReportType {
    fn max_top_n_count(&self) -> i32 {
        match self {
            Self::Daily => 5,
            Self::Weekly => 10,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReportPeriod {
    pub start: String,
    pub end: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReportTrafficSummary {
    pub upload: i64,
    pub download: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReportDomainStat {
    pub domain: String,
    pub traffic: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReportRuleStat {
    pub rule: String,
    pub hit_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReportComparison {
    pub traffic_change: f64,
    pub connection_change: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReportDailyTrendPoint {
    pub date: String,
    pub upload: i64,
    pub download: i64,
    pub total_traffic: i64,
    pub conn_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReportStats {
    pub total_traffic: ReportTrafficSummary,
    pub total_connections: i64,
    pub top_domains: Vec<ReportDomainStat>,
    pub top_rules: Vec<ReportRuleStat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peak_hour: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comparison: Option<ReportComparison>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub daily_trend: Option<Vec<ReportDailyTrendPoint>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_rate: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReportStatsPayload {
    pub period: ReportPeriod,
    pub stats: ReportStats,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReportResult {
    #[serde(rename = "type")]
    pub report_type: ReportType,
    pub period: ReportPeriod,
    pub content: String,
    pub stats: ReportStats,
    pub generated_at: String,
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

impl_serialize_display!(AiConfigChangeError);

#[derive(Error, Debug)]
pub enum AiReportError {
    #[error("{0}")]
    Database(#[from] DbError),
    #[error("{0}")]
    Sidecar(#[from] AiSidecarError),
    #[error("报告日期无效: {0}")]
    InvalidDate(String),
    #[error("报告参数无效: {0}")]
    InvalidParams(String),
    #[error("报告结果无效: {0}")]
    InvalidResult(String),
}

impl_serialize_display!(AiReportError);

#[derive(Error, Debug)]
pub enum AiSettingsError {
    #[error("解析应用数据目录失败: {0}")]
    ResolveAppDataDir(String),
    #[error("AI 设置文件不存在: {path}")]
    NotFound { path: String },
    #[error("创建 AI 设置目录失败: {path}: {source}")]
    CreateDir {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("读取 AI 设置失败: {path}: {source}")]
    ReadFile {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("写入 AI 设置失败: {path}: {source}")]
    WriteFile {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("序列化 AI 设置失败: {0}")]
    Serialize(String),
    #[error("AI 设置无效: {0}")]
    InvalidSettings(String),
    #[error("解析连通性测试结果失败: {0}")]
    InvalidConnectionTest(String),
    #[error("解析模型列表结果失败: {0}")]
    InvalidModelCatalog(String),
    #[error("{0}")]
    Sidecar(#[from] AiSidecarError),
}

impl_serialize_display!(AiSettingsError);

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReportWindow {
    period: ReportPeriod,
    start_day: NaiveDate,
    end_day_exclusive: NaiveDate,
    previous_start_day: NaiveDate,
    previous_end_day_exclusive: NaiveDate,
}

fn parse_report_date(date: Option<&str>) -> Result<NaiveDate, AiReportError> {
    match date {
        Some(value) => {
            let trimmed = value.trim();
            NaiveDate::parse_from_str(trimmed, "%Y-%m-%d")
                .map_err(|error| AiReportError::InvalidDate(format!("{trimmed} ({error})")))
        }
        None => Ok(Utc::now().date_naive()),
    }
}

fn build_report_window(report_type: &ReportType, end_date: NaiveDate) -> ReportWindow {
    match report_type {
        ReportType::Daily => {
            let start_day = end_date;
            let end_day_exclusive = end_date + ChronoDuration::days(1);
            let previous_start_day = start_day - ChronoDuration::days(1);
            let previous_end_day_exclusive = start_day;

            ReportWindow {
                period: ReportPeriod {
                    start: start_day.to_string(),
                    end: end_date.to_string(),
                },
                start_day,
                end_day_exclusive,
                previous_start_day,
                previous_end_day_exclusive,
            }
        }
        ReportType::Weekly => {
            let start_day = end_date - ChronoDuration::days(6);
            let week_end = end_date;
            let end_day_exclusive = end_date + ChronoDuration::days(1);
            let previous_start_day = start_day - ChronoDuration::days(7);
            let previous_end_day_exclusive = start_day;

            ReportWindow {
                period: ReportPeriod {
                    start: start_day.to_string(),
                    end: week_end.to_string(),
                },
                start_day,
                end_day_exclusive,
                previous_start_day,
                previous_end_day_exclusive,
            }
        }
    }
}

fn format_utc_day_start(day: NaiveDate) -> String {
    format!("{day}T00:00:00Z")
}

fn percentage_change(current: i64, previous: i64) -> f64 {
    if previous <= 0 {
        return 0.0;
    }

    ((current - previous) as f64 / previous as f64) * 100.0
}

fn match_rate_percentage(summary: &repo_connection::RuleHitSummary) -> f64 {
    if summary.total_hits <= 0 {
        return 0.0;
    }

    (summary.match_hits as f64 / summary.total_hits as f64) * 100.0
}

fn build_report_daily_trend(
    window: &ReportWindow,
    points: Vec<repo_connection::DailyTrafficTotalRow>,
) -> Vec<ReportDailyTrendPoint> {
    let mut point_map = std::collections::HashMap::with_capacity(points.len());
    for point in points {
        point_map.insert(point.time.clone(), point);
    }

    let mut rows = Vec::with_capacity(7);
    let mut cursor = window.start_day;
    while cursor < window.end_day_exclusive {
        let bucket_key = format_utc_day_start(cursor);
        let point = point_map.remove(&bucket_key);
        let upload = point.as_ref().map_or(0, |entry| entry.upload);
        let download = point.as_ref().map_or(0, |entry| entry.download);
        let conn_count = point.as_ref().map_or(0, |entry| entry.conn_count);

        rows.push(ReportDailyTrendPoint {
            date: cursor.to_string(),
            upload,
            download,
            total_traffic: upload + download,
            conn_count,
        });

        cursor = cursor + ChronoDuration::days(1);
    }

    rows
}

fn peak_hour_label(points: &[repo_traffic::TrafficBucketRow]) -> Option<String> {
    points
        .iter()
        .max_by(|left, right| {
            let left_total = left.upload + left.download;
            let right_total = right.upload + right.download;
            left_total
                .cmp(&right_total)
                .then_with(|| left.conn_count.cmp(&right.conn_count))
                .then_with(|| left.time.cmp(&right.time))
        })
        .map(|point| point.time.clone())
}

pub(crate) async fn get_report_stats(
    app_handle: &AppHandle,
    report_type: ReportType,
    date: Option<&str>,
) -> Result<ReportStatsPayload, AiReportError> {
    let report_date = parse_report_date(date)?;
    let window = build_report_window(&report_type, report_date);
    let start_day = window.start_day.to_string();
    let end_day_exclusive = window.end_day_exclusive.to_string();
    let previous_start_day = window.previous_start_day.to_string();
    let previous_end_day_exclusive = window.previous_end_day_exclusive.to_string();
    let start_iso = format_utc_day_start(window.start_day);
    let end_iso = format_utc_day_start(window.end_day_exclusive);
    let db = db::get_db_pool(app_handle).await?;

    let (current_overview, previous_overview, top_domains, top_rules, rule_hit_summary) = tokio::try_join!(
        repo_connection::get_overview_by_window(&db, &start_day, &end_day_exclusive),
        repo_connection::get_overview_by_window(
            &db,
            &previous_start_day,
            &previous_end_day_exclusive,
        ),
        repo_domain::query_top_domains_by_window(
            &db,
            &start_day,
            &end_day_exclusive,
            report_type.max_top_n_count(),
        ),
        repo_connection::query_rule_stats_by_window(
            &db,
            &start_day,
            &end_day_exclusive,
            report_type.max_top_n_count(),
        ),
        repo_connection::summarize_rule_hits_by_window(&db, &start_day, &end_day_exclusive),
    )?;

    let peak_hour = if report_type == ReportType::Daily {
        let hourly_points = repo_traffic::query_hourly(&db, &start_iso, &end_iso).await?;
        peak_hour_label(&hourly_points)
    } else {
        None
    };

    let daily_trend = if report_type == ReportType::Weekly {
        let daily_points =
            repo_connection::query_daily_totals_by_window(&db, &start_day, &end_day_exclusive)
                .await?;
        Some(build_report_daily_trend(&window, daily_points))
    } else {
        None
    };

    let current_total_traffic = current_overview.total_upload + current_overview.total_download;
    let previous_total_traffic = previous_overview.total_upload + previous_overview.total_download;

    Ok(ReportStatsPayload {
        period: window.period,
        stats: ReportStats {
            total_traffic: ReportTrafficSummary {
                upload: current_overview.total_upload,
                download: current_overview.total_download,
            },
            total_connections: current_overview.total_connections,
            top_domains: top_domains
                .into_iter()
                .map(|row| ReportDomainStat {
                    domain: row.domain,
                    traffic: row.upload + row.download,
                })
                .collect(),
            top_rules: top_rules
                .into_iter()
                .map(|row| ReportRuleStat {
                    rule: row.rule,
                    hit_count: row.hit_count,
                })
                .collect(),
            peak_hour,
            comparison: Some(ReportComparison {
                traffic_change: percentage_change(current_total_traffic, previous_total_traffic),
                connection_change: percentage_change(
                    current_overview.total_connections,
                    previous_overview.total_connections,
                ),
            }),
            daily_trend,
            match_rate: Some(match_rate_percentage(&rule_hit_summary)),
        },
    })
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

fn migrate_legacy_provider_value(settings_value: &mut serde_json::Value) {
    let Some(settings_object) = settings_value.as_object_mut() else {
        return;
    };

    let Some(provider) = settings_object
        .get("provider")
        .and_then(serde_json::Value::as_str)
    else {
        return;
    };

    let fallback_base_url = match provider {
        "deepseek" => Some(LEGACY_DEEPSEEK_BASE_URL),
        "ollama" => Some(LEGACY_OLLAMA_OPENAI_BASE_URL),
        _ => None,
    };

    let Some(fallback_base_url) = fallback_base_url else {
        return;
    };

    settings_object.insert(
        "provider".to_string(),
        serde_json::Value::String("openai_compatible".to_string()),
    );

    let has_base_url = settings_object
        .get("baseUrl")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|value| !value.trim().is_empty());

    if !has_base_url {
        settings_object.insert(
            "baseUrl".to_string(),
            serde_json::Value::String(fallback_base_url.to_string()),
        );
    }
}

fn ai_settings_file_path(app: &AppHandle) -> Result<PathBuf, AiSettingsError> {
    app.path()
        .app_data_dir()
        .map(|path| path.join(AI_SETTINGS_FILE_NAME))
        .map_err(|error| AiSettingsError::ResolveAppDataDir(error.to_string()))
}

async fn load_ai_settings(app: &AppHandle) -> Result<AiSettings, AiSettingsError> {
    let settings_path = ai_settings_file_path(app)?;
    let settings_content = match tokio::fs::read_to_string(&settings_path).await {
        Ok(content) => content,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Err(AiSettingsError::NotFound {
                path: settings_path.to_string_lossy().into_owned(),
            });
        }
        Err(error) => {
            return Err(AiSettingsError::ReadFile {
                path: settings_path.to_string_lossy().into_owned(),
                source: error,
            });
        }
    };

    match serde_json::from_str::<serde_json::Value>(&settings_content) {
        Ok(mut settings_value) => {
            migrate_legacy_provider_value(&mut settings_value);

            match serde_json::from_value::<AiSettings>(settings_value) {
                Ok(settings) => Ok(settings.normalized()),
                Err(error) => {
                    tracing::warn!(
                        "AI 设置文件解析失败，已回退到默认值: path={}, error={error}",
                        settings_path.display()
                    );
                    Ok(AiSettings::default())
                }
            }
        }
        Err(error) => {
            tracing::warn!(
                "AI 设置文件解析失败，已回退到默认值: path={}, error={error}",
                settings_path.display()
            );
            Ok(AiSettings::default())
        }
    }
}

async fn store_ai_settings(app: &AppHandle, settings: AiSettings) -> Result<(), AiSettingsError> {
    let settings_path = ai_settings_file_path(app)?;

    if let Some(parent) = settings_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|source| AiSettingsError::CreateDir {
                path: parent.to_string_lossy().into_owned(),
                source,
            })?;
    }

    let payload = serde_json::to_vec_pretty(&settings)
        .map_err(|error| AiSettingsError::Serialize(error.to_string()))?;

    tokio::fs::write(&settings_path, payload)
        .await
        .map_err(|source| AiSettingsError::WriteFile {
            path: settings_path.to_string_lossy().into_owned(),
            source,
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
pub async fn get_ai_settings(app: AppHandle) -> Result<AiSettings, AiSettingsError> {
    load_ai_settings(&app).await
}

#[tauri::command]
pub async fn set_ai_settings(app: AppHandle, settings: AiSettings) -> Result<(), AiSettingsError> {
    store_ai_settings(&app, settings.normalized()).await
}

#[tauri::command]
pub async fn ai_ping(
    state: tauri::State<'_, AiSidecarState>,
) -> Result<serde_json::Value, AiSidecarError> {
    sidecar::send_rpc(&state, "ping", None).await
}

#[tauri::command]
pub async fn test_ai_connection(
    app: AppHandle,
    state: tauri::State<'_, AiSidecarState>,
    settings: AiSettings,
) -> Result<AiConnectionTestResult, AiSettingsError> {
    let settings = settings.normalized();
    settings.validate_for_provider_request()?;

    if !sidecar::is_ai_running(&state) {
        sidecar::start_ai(&app, &state).await?;
    }

    let response = sidecar::send_rpc(
        &state,
        "test_connection",
        Some(serde_json::json!({
            "settings": settings.to_provider_settings(),
        })),
    )
    .await?;

    serde_json::from_value(response)
        .map_err(|error| AiSettingsError::InvalidConnectionTest(error.to_string()))
}

#[tauri::command]
pub async fn fetch_ai_models(
    app: AppHandle,
    state: tauri::State<'_, AiSidecarState>,
    settings: AiSettings,
) -> Result<AiModelCatalog, AiSettingsError> {
    let settings = settings.normalized();

    if !sidecar::is_ai_running(&state) {
        sidecar::start_ai(&app, &state).await?;
    }

    let response = sidecar::send_rpc(
        &state,
        "list_models",
        Some(serde_json::json!({
            "settings": settings.to_model_catalog_settings(),
        })),
    )
    .await?;

    serde_json::from_value(response)
        .map_err(|error| AiSettingsError::InvalidModelCatalog(error.to_string()))
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
pub async fn ai_generate_report(
    app: AppHandle,
    state: tauri::State<'_, AiSidecarState>,
    report_type: ReportType,
    date: Option<String>,
    settings: AiProviderSettings,
) -> Result<ReportResult, AiReportError> {
    if settings.model.trim().is_empty() {
        return Err(AiReportError::InvalidParams(
            "report model must not be empty".to_string(),
        ));
    }

    if !sidecar::is_ai_running(&state) {
        sidecar::start_ai(&app, &state).await?;
    }

    let payload = serde_json::json!({
        "type": report_type,
        "date": date,
        "settings": settings,
    });
    let response = sidecar::send_rpc_with_timeout(
        &state,
        "generate_report",
        Some(payload),
        REPORT_RPC_TIMEOUT,
    )
    .await?;

    serde_json::from_value(response)
        .map_err(|error| AiReportError::InvalidResult(error.to_string()))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ai_settings_default_matches_expected_defaults() {
        let settings = AiSettings::default();

        assert!(matches!(settings.provider, AiProviderKind::Openai));
        assert_eq!(settings.model, "gpt-4o-mini");
        assert_eq!(settings.api_key, "");
        assert_eq!(settings.base_url, "");
        assert_eq!(settings.temperature, 0.3);
        assert_eq!(settings.max_tokens, 4096);
        assert!(!settings.auto_start);
    }

    #[test]
    fn ai_settings_normalized_clamps_temperature_and_fills_default_model() {
        let settings = AiSettings {
            provider: AiProviderKind::Claude,
            model: "   ".to_string(),
            api_key: "  test-key  ".to_string(),
            base_url: "  https://example.com  ".to_string(),
            temperature: 42.0,
            max_tokens: 0,
            auto_start: true,
        }
        .normalized();

        assert_eq!(settings.model, "claude-sonnet-4-5");
        assert_eq!(settings.api_key, "test-key");
        assert_eq!(settings.base_url, "https://example.com");
        assert_eq!(settings.temperature, 1.0);
        assert_eq!(settings.max_tokens, 1);
        assert!(settings.auto_start);
    }

    #[test]
    fn ai_settings_to_provider_settings_preserves_max_tokens() {
        let settings = AiSettings {
            max_tokens: 2048,
            ..AiSettings::default()
        };

        let provider_settings = settings.to_provider_settings();

        assert_eq!(provider_settings.max_tokens, Some(2048));
    }

    #[test]
    fn openai_compatible_requires_base_url_but_not_api_key() {
        let settings = AiSettings {
            provider: AiProviderKind::OpenaiCompatible,
            model: "llama3.2".to_string(),
            api_key: String::new(),
            base_url: String::new(),
            temperature: 0.3,
            max_tokens: 2048,
            auto_start: false,
        };

        let error = settings.validate_for_provider_request();
        assert!(matches!(error, Err(AiSettingsError::InvalidSettings(_))));

        let valid_settings = AiSettings {
            base_url: LEGACY_OLLAMA_OPENAI_BASE_URL.to_string(),
            ..settings
        };

        assert!(valid_settings.validate_for_provider_request().is_ok());
    }

    #[test]
    fn migrate_legacy_provider_value_maps_deepseek_to_openai_compatible() {
        let mut value = serde_json::json!({
            "provider": "deepseek",
            "model": "deepseek-chat",
            "apiKey": "test-key",
            "baseUrl": "",
            "temperature": 0.3,
            "maxTokens": 4096,
            "autoStart": false
        });

        migrate_legacy_provider_value(&mut value);

        assert_eq!(
            value.get("provider").and_then(serde_json::Value::as_str),
            Some("openai_compatible")
        );
        assert_eq!(
            value.get("baseUrl").and_then(serde_json::Value::as_str),
            Some(LEGACY_DEEPSEEK_BASE_URL)
        );
    }

    #[test]
    fn weekly_report_window_uses_selected_end_date_as_period_end() {
        let end_date = NaiveDate::from_ymd_opt(2026, 4, 9);
        assert!(end_date.is_some());
        let Some(end_date) = end_date else {
            panic!("date should be valid");
        };

        let window = build_report_window(&ReportType::Weekly, end_date);

        assert_eq!(window.period.start, "2026-04-03");
        assert_eq!(window.period.end, "2026-04-09");
        assert_eq!(window.start_day.to_string(), "2026-04-03");
        assert_eq!(window.end_day_exclusive.to_string(), "2026-04-10");
        assert_eq!(window.previous_start_day.to_string(), "2026-03-27");
        assert_eq!(window.previous_end_day_exclusive.to_string(), "2026-04-03");
    }
}
