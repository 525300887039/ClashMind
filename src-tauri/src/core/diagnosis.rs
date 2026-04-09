use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use tauri_plugin_sql::DbPool;
use thiserror::Error;

use crate::{
    db::{
        repo_error_log::{self, ErrorCategoryCount, HostFailureRate, ProxyErrorCount},
        sqlite_pool, DbError,
    },
    utils::time as time_utils,
};

const DEFAULT_TOP_LIMIT: i32 = 10;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosisSummary {
    pub time_range_minutes: i32,
    pub error_stats: Vec<ErrorCategoryCount>,
    pub top_error_nodes: Vec<ProxyErrorCount>,
    pub top_failure_hosts: Vec<HostFailureRate>,
    pub dns_error_count: i64,
    pub match_fallback_count: i64,
    pub total_connections: i64,
    pub generated_at: String,
}

#[derive(Debug, Error)]
pub enum DiagnosisError {
    #[error("{0}")]
    Database(#[from] DbError),
    #[error("诊断时间窗口无效: {0}")]
    InvalidTimeRange(String),
}

crate::utils::impl_serialize_display!(DiagnosisError);

#[derive(Debug, Clone, PartialEq, Eq)]
struct ConnectionDiagnosisMetrics {
    total_connections: i64,
    match_fallback_count: i64,
}

pub async fn generate_diagnosis_summary(
    db: &DbPool,
    time_range_minutes: i32,
) -> Result<DiagnosisSummary, DiagnosisError> {
    let time_range_minutes = validate_time_range_minutes(time_range_minutes)?;
    let (error_stats, top_error_nodes, top_failure_hosts, dns_error_count, connection_metrics) = tokio::try_join!(
        repo_error_log::get_error_category_counts(db, time_range_minutes),
        repo_error_log::get_top_error_nodes(db, time_range_minutes, DEFAULT_TOP_LIMIT),
        repo_error_log::get_top_failure_hosts(db, time_range_minutes, DEFAULT_TOP_LIMIT),
        repo_error_log::count_dns_errors(db, time_range_minutes),
        query_connection_metrics(db, time_range_minutes),
    )?;

    Ok(DiagnosisSummary {
        time_range_minutes,
        error_stats,
        top_error_nodes,
        top_failure_hosts,
        dns_error_count,
        match_fallback_count: connection_metrics.match_fallback_count,
        total_connections: connection_metrics.total_connections,
        generated_at: time_utils::format_utc(Utc::now()),
    })
}

async fn query_connection_metrics(
    db: &DbPool,
    time_range_minutes: i32,
) -> Result<ConnectionDiagnosisMetrics, DbError> {
    let pool = sqlite_pool(db)?;
    let row = sqlx::query(
        r#"
SELECT
    COUNT(*) AS total_connections,
    COALESCE(
        SUM(
            CASE
                WHEN COALESCE(NULLIF(TRIM(rule), ''), 'UNKNOWN') = 'MATCH' THEN 1
                ELSE 0
            END
        ),
        0
    ) AS match_fallback_count
FROM connections
WHERE datetime(COALESCE(close_time, last_observed_at, start_time)) >= datetime('now', '-' || ? || ' minutes');
"#,
    )
    .bind(i64::from(time_range_minutes))
    .fetch_one(pool)
    .await
    .map_err(|error| DbError::QueryFailed(format!("查询诊断连接指标失败: {error}")))?;

    Ok(ConnectionDiagnosisMetrics {
        total_connections: row.try_get("total_connections").map_err(|error| {
            DbError::QueryFailed(format!("读取诊断指标 total_connections 失败: {error}"))
        })?,
        match_fallback_count: row.try_get("match_fallback_count").map_err(|error| {
            DbError::QueryFailed(format!("读取诊断指标 match_fallback_count 失败: {error}"))
        })?,
    })
}

fn validate_time_range_minutes(time_range_minutes: i32) -> Result<i32, DiagnosisError> {
    if time_range_minutes <= 0 {
        return Err(DiagnosisError::InvalidTimeRange(
            "time_range_minutes 必须大于 0".to_string(),
        ));
    }

    Ok(time_range_minutes)
}

#[cfg(test)]
mod tests {
    use sqlx::sqlite::SqlitePoolOptions;

    use super::*;

    async fn prepare_db() -> Result<DbPool, String> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .map_err(|error| error.to_string())?;

        sqlx::query(
            r#"
CREATE TABLE error_logs (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    category   TEXT NOT NULL,
    proxy_node TEXT,
    host       TEXT,
    rule       TEXT,
    message    TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE connections (
    id               TEXT PRIMARY KEY,
    host             TEXT,
    network          TEXT NOT NULL,
    conn_type        TEXT NOT NULL,
    rule             TEXT NOT NULL,
    proxy_chain      TEXT NOT NULL,
    start_time       TEXT NOT NULL,
    close_time       TEXT,
    last_observed_at TEXT
);
"#,
        )
        .execute(&pool)
        .await
        .map_err(|error| error.to_string())?;

        Ok(DbPool::Sqlite(pool))
    }

    #[tokio::test]
    async fn generate_diagnosis_summary_returns_expected_metrics() {
        let db = prepare_db().await;
        assert!(db.is_ok());
        let Ok(db) = db else {
            panic!("diagnosis test database should be created");
        };

        let pool = sqlite_pool(&db);
        assert!(pool.is_ok());
        let Ok(pool) = pool else {
            panic!("sqlite pool should be available");
        };

        let seed = sqlx::query(
            r#"
INSERT INTO error_logs (category, proxy_node, host, rule, message, created_at) VALUES
    ('timeout', 'Proxy-A', 'api.example', 'MATCH', 'i/o timeout', datetime('now', '-10 minutes')),
    ('timeout', 'Proxy-A', 'api.example', 'MATCH', 'deadline exceeded', datetime('now', '-9 minutes')),
    ('dns', 'Proxy-B', 'dns.example', 'RULE-SET', 'no such host', datetime('now', '-8 minutes')),
    ('other', NULL, NULL, NULL, 'websocket closed unexpectedly', datetime('now', '-7 minutes'));

INSERT INTO connections (
    id, host, network, conn_type, rule, proxy_chain, start_time, close_time, last_observed_at
) VALUES
    (
        'conn-1',
        'api.example',
        'tcp',
        'HTTPS',
        'MATCH',
        '["Proxy-A"]',
        strftime('%Y-%m-%dT%H:%M:%SZ', datetime('now', '-10 minutes')),
        strftime('%Y-%m-%dT%H:%M:%SZ', datetime('now', '-9 minutes')),
        strftime('%Y-%m-%dT%H:%M:%SZ', datetime('now', '-9 minutes'))
    ),
    (
        'conn-2',
        'api.example',
        'tcp',
        'HTTPS',
        'RULE-SET',
        '["Proxy-A"]',
        strftime('%Y-%m-%dT%H:%M:%SZ', datetime('now', '-8 minutes')),
        NULL,
        strftime('%Y-%m-%dT%H:%M:%SZ', datetime('now'))
    ),
    (
        'conn-3',
        'dns.example',
        'udp',
        'DNS',
        'MATCH',
        '["Proxy-B"]',
        strftime('%Y-%m-%dT%H:%M:%SZ', datetime('now', '-7 minutes')),
        NULL,
        strftime('%Y-%m-%dT%H:%M:%SZ', datetime('now'))
    );
"#,
        )
        .execute(pool)
        .await;
        assert!(seed.is_ok());

        let summary = generate_diagnosis_summary(&db, 30).await;
        assert!(summary.is_ok());
        let Ok(summary) = summary else {
            panic!("diagnosis summary should be generated");
        };

        assert_eq!(summary.time_range_minutes, 30);
        assert_eq!(summary.total_connections, 3);
        assert_eq!(summary.match_fallback_count, 2);
        assert_eq!(summary.dns_error_count, 1);
        assert_eq!(summary.error_stats.len(), 3);
        assert_eq!(summary.top_error_nodes.len(), 2);
        assert_eq!(summary.top_error_nodes[0].proxy_node, "Proxy-A");
        assert_eq!(summary.top_error_nodes[0].count, 2);
        assert_eq!(summary.top_failure_hosts.len(), 1);
        assert_eq!(summary.top_failure_hosts[0].host, "api.example");
        assert_eq!(summary.top_failure_hosts[0].failure_count, 1);
        assert_eq!(summary.top_failure_hosts[0].failure_rate, 0.5);
        assert!(!summary.generated_at.is_empty());
    }

    #[tokio::test]
    async fn generate_diagnosis_summary_rejects_invalid_time_ranges() {
        let db = prepare_db().await;
        assert!(db.is_ok());
        let Ok(db) = db else {
            panic!("diagnosis test database should be created");
        };

        let result = generate_diagnosis_summary(&db, 0).await;
        assert!(matches!(result, Err(DiagnosisError::InvalidTimeRange(_))));
    }
}
