use std::path::Path;

use chrono::{Duration as ChronoDuration, Utc};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::{
    cmd::{self, stats::RuleStat, MihomoState},
    collector::{CollectorState, RealtimeStore},
    core::{
        anomaly::{self, AnomalyThresholds},
        diagnosis,
        sidecar::AiSidecarError,
    },
    db::{self, repo_connection},
    utils::{geoip::GeoIpConfigState, path::expand_tilde, time},
};

const DEFAULT_DELAY_TEST_URL: &str = "http://www.gstatic.com/generate_204";
const DEFAULT_DELAY_TEST_TIMEOUT_MS: u32 = 5_000;
const DEFAULT_RULE_STATS_LIMIT: i32 = 20;
const REDACTED_VALUE: &str = "<redacted>";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiCallbackRequest {
    pub callback_id: String,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum TrafficGranularity {
    Hourly,
    Daily,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DaysOnlyParams {
    days: Option<i32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TopDomainsParams {
    days: Option<i32>,
    limit: Option<i32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DelayParams {
    name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrafficTrendParams {
    granularity: TrafficGranularity,
    days: Option<i32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecentErrorsParams {
    minutes: Option<i32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReportStatsParams {
    #[serde(rename = "type")]
    report_type: cmd::ai::ReportType,
    date: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosisParams {
    time_range_minutes: Option<i32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SelectedProxyGroup {
    group: String,
    current: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConnectivitySummary {
    reachable: bool,
    api_address: String,
    collector_running: bool,
    active_connections: usize,
    proxy_count: usize,
    selected_groups: Vec<SelectedProxyGroup>,
    version: Option<serde_json::Value>,
    issues: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeIssue {
    source: String,
    severity: String,
    message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RecentErrorsSummary {
    window_minutes: i32,
    issues: Vec<RuntimeIssue>,
    note: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuleMatchStatsSummary {
    days: i32,
    total_hits: i64,
    match_hits: i64,
    match_rate: f64,
    rules: Vec<RuleStat>,
}

fn invalid_callback(message: impl Into<String>) -> AiSidecarError {
    AiSidecarError::InvalidResponse(message.into())
}

fn parse_params<T>(params: serde_json::Value) -> Result<T, AiSidecarError>
where
    T: DeserializeOwned,
{
    let normalized_params = if params.is_null() {
        serde_json::json!({})
    } else {
        params
    };

    serde_json::from_value(normalized_params)
        .map_err(|error| invalid_callback(format!("callback 参数无效: {error}")))
}

fn bounded_i32(
    value: Option<i32>,
    default_value: i32,
    min_value: i32,
    max_value: i32,
    label: &str,
) -> Result<i32, AiSidecarError> {
    let resolved = value.unwrap_or(default_value);
    if (min_value..=max_value).contains(&resolved) {
        Ok(resolved)
    } else {
        Err(invalid_callback(format!(
            "{label} 超出范围，期望 {min_value}..={max_value}，收到 {resolved}"
        )))
    }
}

fn extract_proxy_count(proxies: &serde_json::Value) -> usize {
    proxies
        .get("proxies")
        .and_then(serde_json::Value::as_object)
        .map(|items| items.len())
        .unwrap_or(0)
}

fn extract_selected_groups(proxies: &serde_json::Value) -> Vec<SelectedProxyGroup> {
    let Some(proxy_items) = proxies
        .get("proxies")
        .and_then(serde_json::Value::as_object)
    else {
        return Vec::new();
    };

    let mut groups = proxy_items
        .iter()
        .filter_map(|(group_name, value)| {
            let current = value.get("now")?.as_str()?.trim();
            if current.is_empty() {
                return None;
            }

            Some(SelectedProxyGroup {
                group: group_name.clone(),
                current: current.to_string(),
            })
        })
        .collect::<Vec<_>>();

    groups.sort_by(|left, right| left.group.cmp(&right.group));
    groups
}

fn build_traffic_window(granularity: &TrafficGranularity, days: i32) -> (String, String) {
    let now = Utc::now();
    let days = i64::from(days);

    match granularity {
        TrafficGranularity::Hourly => {
            let end = time::next_hour_boundary(&now);
            let start = end - ChronoDuration::days(days);
            (time::format_utc(start), time::format_utc(end))
        }
        TrafficGranularity::Daily => {
            let end = time::next_day_boundary(&now);
            let start = end - ChronoDuration::days(days);
            (time::format_utc(start), time::format_utc(end))
        }
    }
}

fn normalize_config_key(key: &str) -> String {
    key.chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn is_sensitive_config_key(key: &str) -> bool {
    matches!(
        normalize_config_key(key).as_str(),
        "password"
            | "passwd"
            | "secret"
            | "token"
            | "uuid"
            | "apikey"
            | "privatekey"
            | "auth"
            | "authstr"
            | "authorization"
            | "clientsecret"
            | "users"
    )
}

fn redact_sensitive_config(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(entries) => {
            let mut sanitized = serde_json::Map::with_capacity(entries.len());
            for (key, entry_value) in entries {
                if is_sensitive_config_key(&key) {
                    sanitized.insert(key, serde_json::Value::String(REDACTED_VALUE.to_string()));
                } else {
                    sanitized.insert(key, redact_sensitive_config(entry_value));
                }
            }
            serde_json::Value::Object(sanitized)
        }
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.into_iter().map(redact_sensitive_config).collect())
        }
        other_value => other_value,
    }
}

fn active_config_file_path(app: &AppHandle) -> std::path::PathBuf {
    let geoip_config = app.state::<GeoIpConfigState>();
    Path::new(&expand_tilde(&geoip_config.config_dir())).join("config.yaml")
}

async fn read_active_config_file(app: &AppHandle) -> Result<String, AiSidecarError> {
    let config_file = active_config_file_path(app);

    tokio::fs::read_to_string(&config_file)
        .await
        .map_err(|error| invalid_callback(format!("读取配置文件失败: {error}")))
}

fn parse_config_yaml(yaml_content: &str) -> Result<serde_json::Value, AiSidecarError> {
    serde_yaml::from_str::<serde_json::Value>(yaml_content)
        .map_err(|error| invalid_callback(format!("解析配置文件失败: {error}")))
}

fn build_sanitized_config_response(source: &str, config: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "source": source,
        "sanitized": true,
        "config": redact_sensitive_config(config),
    })
}

async fn with_mihomo_client<F, T>(app: &AppHandle, callback: F) -> Result<T, AiSidecarError>
where
    F: for<'a> FnOnce(
        &'a crate::core::mihomo::MihomoClient,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<T, crate::core::mihomo::MihomoError>>
                + Send
                + 'a,
        >,
    >,
{
    let mihomo_state = app.state::<MihomoState>();
    let client = mihomo_state.client.lock().await;
    callback(&client)
        .await
        .map_err(|error| invalid_callback(error.to_string()))
}

async fn read_runtime_config(app: &AppHandle) -> Result<serde_json::Value, AiSidecarError> {
    with_mihomo_client(app, |client| Box::pin(client.get_configs())).await
}

pub async fn handle_callback(
    app: &AppHandle,
    request: AiCallbackRequest,
) -> Result<serde_json::Value, AiSidecarError> {
    match request.method.as_str() {
        "get_config" => match read_runtime_config(app).await {
            Ok(config) => Ok(build_sanitized_config_response("mihomo_runtime", config)),
            Err(runtime_error) => {
                let yaml_content = read_active_config_file(app)
                        .await
                        .map_err(|file_error| {
                            invalid_callback(format!(
                                "读取 Mihomo 运行配置失败: {runtime_error}; 同时读取配置文件失败: {file_error}"
                            ))
                        })?;
                let config = parse_config_yaml(&yaml_content).map_err(|file_error| {
                        invalid_callback(format!(
                            "读取 Mihomo 运行配置失败: {runtime_error}; 同时解析配置文件失败: {file_error}"
                        ))
                    })?;

                Ok(build_sanitized_config_response("config_file", config))
            }
        },
        "get_config_file" => {
            let yaml_content = read_active_config_file(app).await?;
            let config = parse_config_yaml(&yaml_content)?;
            Ok(build_sanitized_config_response("config_file", config))
        }
        "read_active_config_file" => Ok(serde_json::Value::String(
            read_active_config_file(app).await?,
        )),
        "read_active_runtime_config" => read_runtime_config(app).await,
        "get_proxies" => with_mihomo_client(app, |client| Box::pin(client.get_proxies())).await,
        "test_delay" => {
            let params = parse_params::<DelayParams>(request.params)?;
            let proxy_name = params.name.clone();
            let delay = with_mihomo_client(app, move |client| {
                Box::pin(async move {
                    client
                        .test_delay(
                            &proxy_name,
                            DEFAULT_DELAY_TEST_URL,
                            DEFAULT_DELAY_TEST_TIMEOUT_MS,
                        )
                        .await
                })
            })
            .await?;

            Ok(serde_json::json!({
                "name": params.name,
                "url": DEFAULT_DELAY_TEST_URL,
                "timeout": DEFAULT_DELAY_TEST_TIMEOUT_MS,
                "delay": delay,
            }))
        }
        "get_stats_overview" => {
            let params = parse_params::<DaysOnlyParams>(request.params)?;
            let days = bounded_i32(params.days, 7, 1, 365, "days")?;
            let summary = cmd::stats::get_stats_overview(app.clone(), days)
                .await
                .map_err(|error| invalid_callback(error.to_string()))?;

            Ok(serde_json::json!({
                "days": days,
                "summary": summary,
            }))
        }
        "get_domain_stats" => {
            let params = parse_params::<TopDomainsParams>(request.params)?;
            let days = bounded_i32(params.days, 7, 1, 365, "days")?;
            let limit = bounded_i32(params.limit, 20, 1, 100, "limit")?;
            let domains = cmd::stats::get_domain_stats(app.clone(), days, limit)
                .await
                .map_err(|error| invalid_callback(error.to_string()))?;

            Ok(serde_json::json!({
                "days": days,
                "limit": limit,
                "domains": domains,
            }))
        }
        "get_traffic_trend" => {
            let params = parse_params::<TrafficTrendParams>(request.params)?;
            let days = bounded_i32(params.days, 7, 1, 365, "days")?;
            let (start, end) = build_traffic_window(&params.granularity, days);
            let granularity = match params.granularity {
                TrafficGranularity::Hourly => "hourly",
                TrafficGranularity::Daily => "daily",
            };

            let points = match params.granularity {
                TrafficGranularity::Hourly => {
                    cmd::stats::get_traffic_hourly(app.clone(), start.clone(), end.clone()).await
                }
                TrafficGranularity::Daily => {
                    cmd::stats::get_traffic_daily(app.clone(), start.clone(), end.clone()).await
                }
            }
            .map_err(|error| invalid_callback(error.to_string()))?;

            Ok(serde_json::json!({
                "granularity": granularity,
                "days": days,
                "start": start,
                "end": end,
                "points": points,
            }))
        }
        "check_connectivity" => {
            let api_address = {
                let mihomo_state = app.state::<MihomoState>();
                let client = mihomo_state.client.lock().await;
                client.connection_info().0
            };

            let version_result =
                with_mihomo_client(app, |client| Box::pin(client.get_version())).await;
            let proxies_result =
                with_mihomo_client(app, |client| Box::pin(client.get_proxies())).await;

            let collector_running = app.state::<CollectorState>().is_running();
            let active_connections = app
                .state::<RealtimeStore>()
                .get_summary()
                .await
                .active_count;
            let mut issues = Vec::new();

            if let Err(error) = &version_result {
                issues.push(format!("mihomo version API 不可用: {error}"));
            }

            if let Err(error) = &proxies_result {
                issues.push(format!("mihomo proxies API 不可用: {error}"));
            }

            if !collector_running {
                issues.push("连接采集器当前未运行".to_string());
            }

            let proxy_count = proxies_result
                .as_ref()
                .map(extract_proxy_count)
                .unwrap_or_default();
            let selected_groups = proxies_result
                .as_ref()
                .map(extract_selected_groups)
                .unwrap_or_default();

            Ok(serde_json::to_value(ConnectivitySummary {
                reachable: version_result.is_ok() && proxies_result.is_ok(),
                api_address,
                collector_running,
                active_connections,
                proxy_count,
                selected_groups,
                version: version_result.ok(),
                issues,
            })
            .map_err(|error| invalid_callback(error.to_string()))?)
        }
        "get_recent_errors" => {
            let params = parse_params::<RecentErrorsParams>(request.params)?;
            let minutes = bounded_i32(params.minutes, 30, 1, 1_440, "minutes")?;
            let api_address = {
                let mihomo_state = app.state::<MihomoState>();
                let client = mihomo_state.client.lock().await;
                client.connection_info().0
            };
            let collector_running = app.state::<CollectorState>().is_running();
            let active_connections = app
                .state::<RealtimeStore>()
                .get_summary()
                .await
                .active_count;
            let mut issues = Vec::new();

            match with_mihomo_client(app, |client| Box::pin(client.get_version())).await {
                Ok(_) => {}
                Err(error) => issues.push(RuntimeIssue {
                    source: "mihomo_api".to_string(),
                    severity: "error".to_string(),
                    message: format!("{api_address} 连通性检查失败: {error}"),
                }),
            }

            if !collector_running {
                issues.push(RuntimeIssue {
                    source: "collector".to_string(),
                    severity: "warning".to_string(),
                    message: "连接采集器未运行，最近统计可能不完整".to_string(),
                });
            }

            if active_connections == 0 {
                issues.push(RuntimeIssue {
                    source: "runtime".to_string(),
                    severity: "info".to_string(),
                    message: "当前没有活跃连接，近期错误摘要可能为空".to_string(),
                });
            }

            Ok(serde_json::to_value(RecentErrorsSummary {
                window_minutes: minutes,
                issues,
                note: "当前版本没有独立的持久化错误日志；此结果基于即时运行时健康检查生成。"
                    .to_string(),
            })
            .map_err(|error| invalid_callback(error.to_string()))?)
        }
        "get_rule_stats" => {
            let params = parse_params::<DaysOnlyParams>(request.params)?;
            let days = bounded_i32(params.days, 1, 1, 30, "days")?;
            let db_pool = db::get_db_pool(app)
                .await
                .map_err(|error| invalid_callback(error.to_string()))?;
            let rules = cmd::stats::get_rule_stats(app.clone(), days, DEFAULT_RULE_STATS_LIMIT)
                .await
                .map_err(|error| invalid_callback(error.to_string()))?;
            let hit_summary = repo_connection::summarize_rule_hits(&db_pool, days)
                .await
                .map_err(|error| invalid_callback(error.to_string()))?;
            let total_hits = hit_summary.total_hits;
            let match_hits = hit_summary.match_hits;
            let match_rate = if total_hits == 0 {
                0.0
            } else {
                match_hits as f64 / total_hits as f64
            };

            Ok(serde_json::to_value(RuleMatchStatsSummary {
                days,
                total_hits,
                match_hits,
                match_rate,
                rules,
            })
            .map_err(|error| invalid_callback(error.to_string()))?)
        }
        "get_report_stats" => {
            let params = parse_params::<ReportStatsParams>(request.params)?;
            let report = cmd::ai::get_report_stats(app, params.report_type, params.date.as_deref())
                .await
                .map_err(|error| invalid_callback(error.to_string()))?;

            serde_json::to_value(report).map_err(|error| invalid_callback(error.to_string()))
        }
        "get_diagnosis_summary" => {
            let params = parse_params::<DiagnosisParams>(request.params)?;
            let time_range_minutes =
                bounded_i32(params.time_range_minutes, 30, 5, 1_440, "timeRangeMinutes")?;
            let db_pool = db::get_db_pool(app)
                .await
                .map_err(|error| invalid_callback(error.to_string()))?;
            let summary = diagnosis::generate_diagnosis_summary(&db_pool, time_range_minutes)
                .await
                .map_err(|error| invalid_callback(error.to_string()))?;

            serde_json::to_value(summary).map_err(|error| invalid_callback(error.to_string()))
        }
        "detect_anomalies" => {
            let params = parse_params::<DiagnosisParams>(request.params)?;
            let time_range_minutes =
                bounded_i32(params.time_range_minutes, 30, 5, 1_440, "timeRangeMinutes")?;
            let db_pool = db::get_db_pool(app)
                .await
                .map_err(|error| invalid_callback(error.to_string()))?;
            let summary = diagnosis::generate_diagnosis_summary(&db_pool, time_range_minutes)
                .await
                .map_err(|error| invalid_callback(error.to_string()))?;
            let alerts =
                anomaly::detect_anomalies(&db_pool, &summary, &AnomalyThresholds::default())
                    .await
                    .map_err(|error| invalid_callback(error.to_string()))?;

            serde_json::to_value(alerts).map_err(|error| invalid_callback(error.to_string()))
        }
        "run_full_diagnosis" => {
            let params = parse_params::<DiagnosisParams>(request.params)?;
            let time_range_minutes =
                bounded_i32(params.time_range_minutes, 30, 5, 1_440, "timeRangeMinutes")?;
            let db_pool = db::get_db_pool(app)
                .await
                .map_err(|error| invalid_callback(error.to_string()))?;
            let summary = diagnosis::generate_diagnosis_summary(&db_pool, time_range_minutes)
                .await
                .map_err(|error| invalid_callback(error.to_string()))?;
            let alerts =
                anomaly::detect_anomalies(&db_pool, &summary, &AnomalyThresholds::default())
                    .await
                    .map_err(|error| invalid_callback(error.to_string()))?;

            Ok(serde_json::json!({
                "summary": summary,
                "alerts": alerts,
            }))
        }
        other_method => Err(invalid_callback(format!(
            "未知 callback 方法: {other_method}; callbackId={}",
            request.callback_id
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_sensitive_config_masks_secret_fields_recursively() {
        let config = serde_json::json!({
            "mixed-port": 7890,
            "secret": "controller-token",
            "dns": {
                "nameserver": ["1.1.1.1"],
                "token": "dns-token"
            },
            "proxies": [
                {
                    "name": "vmess-node",
                    "uuid": "uuid-secret",
                    "server": "example.com"
                }
            ],
            "tuic-server": {
                "users": [
                    {
                        "username": "alice",
                        "password": "secret-password"
                    }
                ],
                "private-key": "secret-key"
            }
        });

        let redacted = redact_sensitive_config(config);

        assert_eq!(redacted["mixed-port"], serde_json::json!(7890));
        assert_eq!(redacted["secret"], serde_json::json!(REDACTED_VALUE));
        assert_eq!(redacted["dns"]["token"], serde_json::json!(REDACTED_VALUE));
        assert_eq!(
            redacted["proxies"][0]["uuid"],
            serde_json::json!(REDACTED_VALUE)
        );
        assert_eq!(
            redacted["tuic-server"]["users"],
            serde_json::json!(REDACTED_VALUE)
        );
        assert_eq!(
            redacted["tuic-server"]["private-key"],
            serde_json::json!(REDACTED_VALUE)
        );
    }
}
