//! Rule-based anomaly detection for diagnosis summaries.

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::Row;
use tauri_plugin_sql::DbPool;

use crate::{
    core::diagnosis::DiagnosisSummary,
    db::{sqlite_pool, DbError},
    utils::time as time_utils,
};

/// A structured anomaly alert produced by the rule engine.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AnomalyAlert {
    pub id: String,
    pub severity: AlertSeverity,
    pub alert_type: AlertType,
    pub title: String,
    pub description: String,
    pub context: Value,
    pub detected_at: String,
}

/// Supported anomaly severity levels.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AlertSeverity {
    Critical,
    Warning,
    Info,
}

/// Supported anomaly categories for Step 4.2.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AlertType {
    HighTimeoutRate,
    TrafficSurge,
    TrafficDrop,
    HighMatchFallback,
    DnsFailureCluster,
}

/// Tunable anomaly detection thresholds.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AnomalyThresholds {
    pub timeout_rate: f64,
    pub traffic_surge_multiplier: f64,
    pub traffic_drop_ratio: f64,
    pub match_fallback_rate: f64,
    pub dns_failure_threshold: i64,
}

impl Default for AnomalyThresholds {
    fn default() -> Self {
        Self {
            timeout_rate: 0.15,
            traffic_surge_multiplier: 3.0,
            traffic_drop_ratio: 0.2,
            match_fallback_rate: 0.3,
            dns_failure_threshold: 10,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AlertIdGenerator {
    timestamp_millis: i64,
    next_sequence: u64,
}

impl AlertIdGenerator {
    fn new(now: DateTime<Utc>) -> Self {
        Self {
            timestamp_millis: now.timestamp_millis(),
            next_sequence: 1,
        }
    }

    fn next_id(&mut self) -> String {
        let id = format!("anomaly-{}-{}", self.timestamp_millis, self.next_sequence);
        self.next_sequence += 1;
        id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AlertMetadata<'a> {
    detected_at: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TrafficWindowTotals {
    current_total: i64,
    previous_total: i64,
    bucket_count: i64,
}

/// Detects anomalies for a diagnosis summary using the provided thresholds.
///
/// # Errors
///
/// Returns [`DbError`] when querying traffic samples for traffic anomaly checks fails.
pub async fn detect_anomalies(
    db: &DbPool,
    summary: &DiagnosisSummary,
    thresholds: &AnomalyThresholds,
) -> Result<Vec<AnomalyAlert>, DbError> {
    detect_anomalies_at(db, summary, thresholds, Utc::now()).await
}

async fn detect_anomalies_at(
    db: &DbPool,
    summary: &DiagnosisSummary,
    thresholds: &AnomalyThresholds,
    now: DateTime<Utc>,
) -> Result<Vec<AnomalyAlert>, DbError> {
    let detected_at = time_utils::format_utc(now);
    let metadata = AlertMetadata {
        detected_at: &detected_at,
    };
    let mut id_generator = AlertIdGenerator::new(now);
    let mut alerts = Vec::with_capacity(4);

    if let Some(alert) = check_timeout_rate(summary, thresholds, &metadata, &mut id_generator) {
        alerts.push(alert);
    }

    if let Some(alert) =
        check_traffic_anomaly(db, summary, thresholds, now, &metadata, &mut id_generator).await?
    {
        alerts.push(alert);
    }

    if let Some(alert) = check_match_fallback(summary, thresholds, &metadata, &mut id_generator) {
        alerts.push(alert);
    }

    if let Some(alert) =
        check_dns_failure_cluster(summary, thresholds, &metadata, &mut id_generator)
    {
        alerts.push(alert);
    }

    Ok(alerts)
}

fn check_timeout_rate(
    summary: &DiagnosisSummary,
    thresholds: &AnomalyThresholds,
    metadata: &AlertMetadata<'_>,
    id_generator: &mut AlertIdGenerator,
) -> Option<AnomalyAlert> {
    if summary.total_connections <= 0 {
        return None;
    }

    let timeout_count = summary
        .error_stats
        .iter()
        .find(|stat| stat.category == "timeout")
        .map(|stat| stat.count)
        .unwrap_or(0);
    let rate = timeout_count as f64 / summary.total_connections as f64;

    if rate <= thresholds.timeout_rate {
        return None;
    }

    let severity = upper_bound_severity(rate, thresholds.timeout_rate);

    Some(build_alert(
        id_generator,
        metadata,
        severity,
        AlertType::HighTimeoutRate,
        format!("超时率过高: {:.1}%", rate * 100.0),
        format!(
            "最近 {} 分钟内，{} 个连接中有 {} 个超时（{:.1}%），超过阈值 {:.1}%。",
            summary.time_range_minutes,
            summary.total_connections,
            timeout_count,
            rate * 100.0,
            thresholds.timeout_rate * 100.0,
        ),
        json!({
            "timeoutCount": timeout_count,
            "totalConnections": summary.total_connections,
            "rate": rate,
            "threshold": thresholds.timeout_rate,
            "topErrorNodes": summary.top_error_nodes,
        }),
    ))
}

async fn check_traffic_anomaly(
    db: &DbPool,
    summary: &DiagnosisSummary,
    thresholds: &AnomalyThresholds,
    now: DateTime<Utc>,
    metadata: &AlertMetadata<'_>,
    id_generator: &mut AlertIdGenerator,
) -> Result<Option<AnomalyAlert>, DbError> {
    let totals = query_traffic_window_totals(db, summary.time_range_minutes, now).await?;
    if totals.previous_total <= 0 {
        return Ok(None);
    }

    let comparison_window_minutes = totals.bucket_count * 60;
    let ratio = totals.current_total as f64 / totals.previous_total as f64;

    if ratio > thresholds.traffic_surge_multiplier {
        let severity = upper_bound_severity(ratio, thresholds.traffic_surge_multiplier);
        return Ok(Some(build_alert(
            id_generator,
            metadata,
            severity,
            AlertType::TrafficSurge,
            format!("流量突增: {:.2} 倍", ratio),
            format!(
                "按最近 {} 分钟的完整小时桶与前一窗口比较，当前总流量为 {}，上一窗口为 {}，达到 {:.2} 倍，超过突增阈值 {:.2} 倍。",
                comparison_window_minutes,
                totals.current_total,
                totals.previous_total,
                ratio,
                thresholds.traffic_surge_multiplier,
            ),
            json!({
                "currentTraffic": totals.current_total,
                "previousTraffic": totals.previous_total,
                "ratio": ratio,
                "windowMinutes": summary.time_range_minutes,
                "comparisonWindowMinutes": comparison_window_minutes,
                "bucketCount": totals.bucket_count,
                "surgeThreshold": thresholds.traffic_surge_multiplier,
                "dropThreshold": thresholds.traffic_drop_ratio,
            }),
        )));
    }

    if ratio < thresholds.traffic_drop_ratio {
        let severity = lower_bound_severity(ratio, thresholds.traffic_drop_ratio);
        return Ok(Some(build_alert(
            id_generator,
            metadata,
            severity,
            AlertType::TrafficDrop,
            format!("流量突降: {:.2} 倍", ratio),
            format!(
                "按最近 {} 分钟的完整小时桶与前一窗口比较，当前总流量为 {}，上一窗口为 {}，仅为 {:.2} 倍，低于突降阈值 {:.2} 倍。",
                comparison_window_minutes,
                totals.current_total,
                totals.previous_total,
                ratio,
                thresholds.traffic_drop_ratio,
            ),
            json!({
                "currentTraffic": totals.current_total,
                "previousTraffic": totals.previous_total,
                "ratio": ratio,
                "windowMinutes": summary.time_range_minutes,
                "comparisonWindowMinutes": comparison_window_minutes,
                "bucketCount": totals.bucket_count,
                "surgeThreshold": thresholds.traffic_surge_multiplier,
                "dropThreshold": thresholds.traffic_drop_ratio,
            }),
        )));
    }

    Ok(None)
}

fn check_match_fallback(
    summary: &DiagnosisSummary,
    thresholds: &AnomalyThresholds,
    metadata: &AlertMetadata<'_>,
    id_generator: &mut AlertIdGenerator,
) -> Option<AnomalyAlert> {
    if summary.total_connections <= 0 {
        return None;
    }

    let rate = summary.match_fallback_count as f64 / summary.total_connections as f64;
    if rate <= thresholds.match_fallback_rate {
        return None;
    }

    let severity = upper_bound_severity(rate, thresholds.match_fallback_rate);

    Some(build_alert(
        id_generator,
        metadata,
        severity,
        AlertType::HighMatchFallback,
        format!("MATCH 兜底率过高: {:.1}%", rate * 100.0),
        format!(
            "最近 {} 分钟内，MATCH 规则命中 {} 次（占总连接 {:.1}%），建议补充更精确的路由规则。",
            summary.time_range_minutes,
            summary.match_fallback_count,
            rate * 100.0,
        ),
        json!({
            "matchCount": summary.match_fallback_count,
            "totalConnections": summary.total_connections,
            "rate": rate,
            "threshold": thresholds.match_fallback_rate,
        }),
    ))
}

fn check_dns_failure_cluster(
    summary: &DiagnosisSummary,
    thresholds: &AnomalyThresholds,
    metadata: &AlertMetadata<'_>,
    id_generator: &mut AlertIdGenerator,
) -> Option<AnomalyAlert> {
    if summary.dns_error_count <= thresholds.dns_failure_threshold {
        return None;
    }

    let severity = upper_bound_severity(
        summary.dns_error_count as f64,
        thresholds.dns_failure_threshold as f64,
    );

    Some(build_alert(
        id_generator,
        metadata,
        severity,
        AlertType::DnsFailureCluster,
        format!("DNS 解析失败集中: {} 次", summary.dns_error_count),
        format!(
            "最近 {} 分钟内 DNS 解析失败 {} 次，超过阈值 {} 次，可能存在 DNS 配置或上游解析异常。",
            summary.time_range_minutes, summary.dns_error_count, thresholds.dns_failure_threshold,
        ),
        json!({
            "dnsErrorCount": summary.dns_error_count,
            "threshold": thresholds.dns_failure_threshold,
            "topFailureHosts": summary.top_failure_hosts,
        }),
    ))
}

async fn query_traffic_window_totals(
    db: &DbPool,
    time_range_minutes: i32,
    now: DateTime<Utc>,
) -> Result<TrafficWindowTotals, DbError> {
    if time_range_minutes <= 0 {
        return Err(DbError::InvalidTimeWindow(
            "time_range_minutes 必须大于 0".to_string(),
        ));
    }

    let bucket_count = comparison_bucket_count(time_range_minutes);
    let current_window_end = time_utils::floor_to_hour(now);
    let current_window_start = current_window_end - ChronoDuration::hours(bucket_count);
    let previous_window_start = current_window_start - ChronoDuration::hours(bucket_count);

    let previous_window_start = time_utils::format_utc(previous_window_start);
    let current_window_start = time_utils::format_utc(current_window_start);
    let current_window_end = time_utils::format_utc(current_window_end);

    let pool = sqlite_pool(db)?;
    let row = sqlx::query(
        r#"
SELECT
    COALESCE(
        SUM(
            CASE
                WHEN datetime(hour) >= datetime(?)
                  AND datetime(hour) < datetime(?)
                THEN upload + download
                ELSE 0
            END
        ),
        0
    ) AS previous_total,
    COALESCE(
        SUM(
            CASE
                WHEN datetime(hour) >= datetime(?)
                  AND datetime(hour) < datetime(?)
                THEN upload + download
                ELSE 0
            END
        ),
        0
    ) AS current_total
FROM traffic_hourly
WHERE datetime(hour) >= datetime(?)
  AND datetime(hour) < datetime(?);
"#,
    )
    .bind(&previous_window_start)
    .bind(&current_window_start)
    .bind(&current_window_start)
    .bind(&current_window_end)
    .bind(&previous_window_start)
    .bind(&current_window_end)
    .fetch_one(pool)
    .await
    .map_err(|error| DbError::QueryFailed(format!("查询流量异常基线失败: {error}")))?;

    Ok(TrafficWindowTotals {
        current_total: row.try_get("current_total").map_err(|error| {
            DbError::QueryFailed(format!("读取流量异常指标 current_total 失败: {error}"))
        })?,
        previous_total: row.try_get("previous_total").map_err(|error| {
            DbError::QueryFailed(format!("读取流量异常指标 previous_total 失败: {error}"))
        })?,
        bucket_count,
    })
}

fn comparison_bucket_count(time_range_minutes: i32) -> i64 {
    let minutes = i64::from(time_range_minutes.max(1));
    (minutes + 59) / 60
}

fn upper_bound_severity(value: f64, threshold: f64) -> AlertSeverity {
    if value > threshold * 2.0 {
        AlertSeverity::Critical
    } else {
        AlertSeverity::Warning
    }
}

fn lower_bound_severity(value: f64, threshold: f64) -> AlertSeverity {
    if value < threshold / 2.0 {
        AlertSeverity::Critical
    } else {
        AlertSeverity::Warning
    }
}

fn build_alert(
    id_generator: &mut AlertIdGenerator,
    metadata: &AlertMetadata<'_>,
    severity: AlertSeverity,
    alert_type: AlertType,
    title: String,
    description: String,
    context: Value,
) -> AnomalyAlert {
    AnomalyAlert {
        id: id_generator.next_id(),
        severity,
        alert_type,
        title,
        description,
        context,
        detected_at: metadata.detected_at.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;
    use sqlx::sqlite::SqlitePoolOptions;

    use super::*;
    use crate::db::repo_error_log::{ErrorCategoryCount, HostFailureRate, ProxyErrorCount};

    async fn prepare_db() -> Result<DbPool, String> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .map_err(|error| error.to_string())?;

        sqlx::query(
            r#"
CREATE TABLE traffic_hourly (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    hour       TEXT NOT NULL UNIQUE,
    upload     INTEGER NOT NULL DEFAULT 0,
    download   INTEGER NOT NULL DEFAULT 0,
    conn_count INTEGER NOT NULL DEFAULT 0
);
"#,
        )
        .execute(&pool)
        .await
        .map_err(|error| error.to_string())?;

        Ok(DbPool::Sqlite(pool))
    }

    fn make_summary(
        time_range_minutes: i32,
        total_connections: i64,
        timeout_count: i64,
        match_fallback_count: i64,
        dns_error_count: i64,
    ) -> DiagnosisSummary {
        let error_stats = if timeout_count > 0 {
            vec![ErrorCategoryCount {
                category: "timeout".to_string(),
                count: timeout_count,
            }]
        } else {
            Vec::new()
        };

        DiagnosisSummary {
            time_range_minutes,
            error_stats,
            top_error_nodes: vec![ProxyErrorCount {
                proxy_node: "Proxy-A".to_string(),
                count: timeout_count,
            }],
            top_failure_hosts: vec![HostFailureRate {
                host: "dns.example".to_string(),
                failure_count: dns_error_count,
                total_count: dns_error_count.max(1),
                failure_rate: if dns_error_count > 0 { 1.0 } else { 0.0 },
            }],
            dns_error_count,
            match_fallback_count,
            total_connections,
            generated_at: time_utils::format_utc(Utc::now()),
        }
    }

    async fn insert_traffic_bucket(
        db: &DbPool,
        hour: &str,
        upload: i64,
        download: i64,
    ) -> Result<(), String> {
        let pool = sqlite_pool(db).map_err(|error| error.to_string())?;

        sqlx::query(
            r#"
INSERT INTO traffic_hourly (hour, upload, download, conn_count)
VALUES (?, ?, ?, 0);
"#,
        )
        .bind(hour)
        .bind(upload)
        .bind(download)
        .execute(pool)
        .await
        .map_err(|error| error.to_string())?;

        Ok(())
    }

    fn find_alert<'a>(
        alerts: &'a [AnomalyAlert],
        alert_type: AlertType,
    ) -> Option<&'a AnomalyAlert> {
        alerts.iter().find(|alert| alert.alert_type == alert_type)
    }

    fn fixed_now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 4, 1, 12, 45, 0)
            .single()
            .expect("fixed timestamp should be valid")
    }

    #[tokio::test]
    async fn detect_anomalies_reports_timeout_warning() {
        let db = prepare_db().await.expect("test database should be created");
        let summary = make_summary(30, 30, 6, 0, 0);

        let alerts = detect_anomalies_at(&db, &summary, &AnomalyThresholds::default(), fixed_now())
            .await
            .expect("anomalies should be detected");

        let alert =
            find_alert(&alerts, AlertType::HighTimeoutRate).expect("timeout alert should exist");
        assert_eq!(alert.severity, AlertSeverity::Warning);
    }

    #[tokio::test]
    async fn detect_anomalies_reports_timeout_critical() {
        let db = prepare_db().await.expect("test database should be created");
        let summary = make_summary(30, 20, 8, 0, 0);

        let alerts = detect_anomalies_at(&db, &summary, &AnomalyThresholds::default(), fixed_now())
            .await
            .expect("anomalies should be detected");

        let alert =
            find_alert(&alerts, AlertType::HighTimeoutRate).expect("timeout alert should exist");
        assert_eq!(alert.severity, AlertSeverity::Critical);
    }

    #[tokio::test]
    async fn detect_anomalies_reports_match_fallback_critical() {
        let db = prepare_db().await.expect("test database should be created");
        let summary = make_summary(30, 10, 0, 7, 0);

        let alerts = detect_anomalies_at(&db, &summary, &AnomalyThresholds::default(), fixed_now())
            .await
            .expect("anomalies should be detected");

        let alert = find_alert(&alerts, AlertType::HighMatchFallback)
            .expect("match fallback alert should exist");
        assert_eq!(alert.severity, AlertSeverity::Critical);
    }

    #[tokio::test]
    async fn detect_anomalies_reports_dns_cluster_alert() {
        let db = prepare_db().await.expect("test database should be created");
        let summary = make_summary(30, 10, 0, 0, 11);

        let alerts = detect_anomalies_at(&db, &summary, &AnomalyThresholds::default(), fixed_now())
            .await
            .expect("anomalies should be detected");

        let alert =
            find_alert(&alerts, AlertType::DnsFailureCluster).expect("dns alert should exist");
        assert_eq!(alert.severity, AlertSeverity::Warning);
    }

    #[tokio::test]
    async fn detect_anomalies_reports_traffic_surge() {
        let db = prepare_db().await.expect("test database should be created");
        insert_traffic_bucket(&db, "2026-04-01T10:00:00Z", 8, 12)
            .await
            .expect("previous traffic bucket should be inserted");
        insert_traffic_bucket(&db, "2026-04-01T11:00:00Z", 30, 40)
            .await
            .expect("current traffic bucket should be inserted");
        let summary = make_summary(30, 10, 0, 0, 0);

        let alerts = detect_anomalies_at(&db, &summary, &AnomalyThresholds::default(), fixed_now())
            .await
            .expect("anomalies should be detected");

        let alert =
            find_alert(&alerts, AlertType::TrafficSurge).expect("traffic surge alert should exist");
        assert_eq!(alert.severity, AlertSeverity::Warning);
    }

    #[tokio::test]
    async fn detect_anomalies_reports_traffic_drop() {
        let db = prepare_db().await.expect("test database should be created");
        insert_traffic_bucket(&db, "2026-04-01T10:00:00Z", 60, 40)
            .await
            .expect("previous traffic bucket should be inserted");
        insert_traffic_bucket(&db, "2026-04-01T11:00:00Z", 4, 5)
            .await
            .expect("current traffic bucket should be inserted");
        let summary = make_summary(30, 10, 0, 0, 0);

        let alerts = detect_anomalies_at(&db, &summary, &AnomalyThresholds::default(), fixed_now())
            .await
            .expect("anomalies should be detected");

        let alert =
            find_alert(&alerts, AlertType::TrafficDrop).expect("traffic drop alert should exist");
        assert_eq!(alert.severity, AlertSeverity::Critical);
    }

    #[tokio::test]
    async fn detect_anomalies_skips_traffic_rule_without_previous_baseline() {
        let db = prepare_db().await.expect("test database should be created");
        insert_traffic_bucket(&db, "2026-04-01T11:00:00Z", 20, 20)
            .await
            .expect("current traffic bucket should be inserted");
        let summary = make_summary(30, 10, 0, 0, 0);

        let alerts = detect_anomalies_at(&db, &summary, &AnomalyThresholds::default(), fixed_now())
            .await
            .expect("anomalies should be detected");

        assert!(find_alert(&alerts, AlertType::TrafficSurge).is_none());
        assert!(find_alert(&alerts, AlertType::TrafficDrop).is_none());
    }

    #[tokio::test]
    async fn detect_anomalies_returns_empty_when_metrics_are_normal() {
        let db = prepare_db().await.expect("test database should be created");
        insert_traffic_bucket(&db, "2026-04-01T10:00:00Z", 10, 10)
            .await
            .expect("previous traffic bucket should be inserted");
        insert_traffic_bucket(&db, "2026-04-01T11:00:00Z", 12, 8)
            .await
            .expect("current traffic bucket should be inserted");
        let summary = make_summary(30, 20, 2, 4, 3);

        let alerts = detect_anomalies_at(&db, &summary, &AnomalyThresholds::default(), fixed_now())
            .await
            .expect("anomalies should be detected");

        assert!(alerts.is_empty());
    }

    #[tokio::test]
    async fn query_traffic_window_totals_uses_completed_hour_buckets() {
        let db = prepare_db().await.expect("test database should be created");
        insert_traffic_bucket(&db, "2026-04-01T09:00:00Z", 3, 7)
            .await
            .expect("older traffic bucket should be inserted");
        insert_traffic_bucket(&db, "2026-04-01T10:00:00Z", 8, 12)
            .await
            .expect("previous traffic bucket should be inserted");
        insert_traffic_bucket(&db, "2026-04-01T11:00:00Z", 30, 40)
            .await
            .expect("current traffic bucket should be inserted");
        insert_traffic_bucket(&db, "2026-04-01T12:00:00Z", 100, 200)
            .await
            .expect("open traffic bucket should be inserted");

        let totals = query_traffic_window_totals(&db, 30, fixed_now())
            .await
            .expect("traffic totals should be queryable");

        assert_eq!(totals.bucket_count, 1);
        assert_eq!(totals.previous_total, 20);
        assert_eq!(totals.current_total, 70);
    }
}
