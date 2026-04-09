use serde::{Deserialize, Serialize};
use sqlx::{Row, Sqlite, Transaction};
use tauri_plugin_sql::DbPool;

use super::{sqlite_pool, try_col, DbError};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ErrorLog {
    pub id: i64,
    pub category: String,
    pub proxy_node: Option<String>,
    pub host: Option<String>,
    pub rule: Option<String>,
    pub message: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ErrorLogInsert {
    pub category: String,
    pub proxy_node: Option<String>,
    pub host: Option<String>,
    pub rule: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ErrorCategoryCount {
    pub category: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProxyErrorCount {
    pub proxy_node: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HostFailureRate {
    pub host: String,
    pub failure_count: i64,
    pub total_count: i64,
    pub failure_rate: f64,
}

#[allow(dead_code)]
pub async fn insert_error_log(db: &DbPool, log: &ErrorLogInsert) -> Result<i64, DbError> {
    let pool = sqlite_pool(db)?;
    let result = sqlx::query(
        r#"
INSERT INTO error_logs (category, proxy_node, host, rule, message)
VALUES (?, ?, ?, ?, ?);
"#,
    )
    .bind(&log.category)
    .bind(log.proxy_node.as_deref())
    .bind(log.host.as_deref())
    .bind(log.rule.as_deref())
    .bind(&log.message)
    .execute(pool)
    .await
    .map_err(|error| DbError::WriteFailed(format!("写入 error_logs 失败: {error}")))?;

    Ok(result.last_insert_rowid())
}

pub async fn insert_error_logs_batch(
    db: &DbPool,
    logs: &[ErrorLogInsert],
) -> Result<usize, DbError> {
    if logs.is_empty() {
        return Ok(0);
    }

    let pool = sqlite_pool(db)?;
    let mut transaction = pool
        .begin()
        .await
        .map_err(|error| DbError::TransactionFailed(error.to_string()))?;

    for log in logs {
        insert_error_log_in_tx(&mut transaction, log).await?;
    }

    transaction
        .commit()
        .await
        .map_err(|error| DbError::TransactionFailed(error.to_string()))?;

    Ok(logs.len())
}

#[allow(dead_code)]
pub async fn get_error_log_by_id(db: &DbPool, id: i64) -> Result<Option<ErrorLog>, DbError> {
    let pool = sqlite_pool(db)?;
    let row = sqlx::query(
        r#"
SELECT id, category, proxy_node, host, rule, message, created_at
FROM error_logs
WHERE id = ?;
"#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|error| DbError::QueryFailed(format!("查询 error_log 详情失败: {error}")))?;

    row.map(map_error_log).transpose()
}

#[allow(dead_code)]
pub async fn get_error_logs_since(
    db: &DbPool,
    since_minutes: i32,
) -> Result<Vec<ErrorLog>, DbError> {
    let minutes = validate_minutes(since_minutes)?;
    let pool = sqlite_pool(db)?;
    let rows = sqlx::query(
        r#"
SELECT id, category, proxy_node, host, rule, message, created_at
FROM error_logs
WHERE datetime(created_at) >= datetime('now', '-' || ? || ' minutes')
ORDER BY datetime(created_at) DESC, id DESC;
"#,
    )
    .bind(minutes)
    .fetch_all(pool)
    .await
    .map_err(|error| DbError::QueryFailed(format!("查询 error_logs 失败: {error}")))?;

    rows.into_iter().map(map_error_log).collect()
}

#[allow(dead_code)]
pub async fn delete_error_log(db: &DbPool, id: i64) -> Result<usize, DbError> {
    let pool = sqlite_pool(db)?;
    let result = sqlx::query(
        r#"
DELETE FROM error_logs
WHERE id = ?;
"#,
    )
    .bind(id)
    .execute(pool)
    .await
    .map_err(|error| DbError::WriteFailed(format!("删除 error_log 失败: {error}")))?;

    usize::try_from(result.rows_affected())
        .map_err(|error| DbError::WriteFailed(format!("删除 error_log 结果溢出: {error}")))
}

pub async fn get_error_category_counts(
    db: &DbPool,
    since_minutes: i32,
) -> Result<Vec<ErrorCategoryCount>, DbError> {
    let minutes = validate_minutes(since_minutes)?;
    let pool = sqlite_pool(db)?;
    let rows = sqlx::query(
        r#"
SELECT
    category,
    COUNT(*) AS count
FROM error_logs
WHERE datetime(created_at) >= datetime('now', '-' || ? || ' minutes')
GROUP BY category
ORDER BY count DESC, category ASC;
"#,
    )
    .bind(minutes)
    .fetch_all(pool)
    .await
    .map_err(|error| DbError::QueryFailed(format!("按分类统计错误日志失败: {error}")))?;

    let mut counts = Vec::with_capacity(rows.len());
    for row in rows {
        counts.push(ErrorCategoryCount {
            category: try_col!(row, "category", "错误分类统计"),
            count: try_col!(row, "count", "错误分类统计"),
        });
    }

    Ok(counts)
}

pub async fn get_top_error_nodes(
    db: &DbPool,
    since_minutes: i32,
    limit: i32,
) -> Result<Vec<ProxyErrorCount>, DbError> {
    if limit <= 0 {
        return Ok(Vec::new());
    }

    let minutes = validate_minutes(since_minutes)?;
    let pool = sqlite_pool(db)?;
    let rows = sqlx::query(
        r#"
SELECT
    TRIM(proxy_node) AS proxy_node,
    COUNT(*) AS count
FROM error_logs
WHERE proxy_node IS NOT NULL
  AND TRIM(proxy_node) <> ''
  AND datetime(created_at) >= datetime('now', '-' || ? || ' minutes')
GROUP BY TRIM(proxy_node)
ORDER BY count DESC, proxy_node ASC
LIMIT ?;
"#,
    )
    .bind(minutes)
    .bind(i64::from(limit))
    .fetch_all(pool)
    .await
    .map_err(|error| DbError::QueryFailed(format!("查询错误节点排行失败: {error}")))?;

    let mut nodes = Vec::with_capacity(rows.len());
    for row in rows {
        nodes.push(ProxyErrorCount {
            proxy_node: try_col!(row, "proxy_node", "错误节点排行"),
            count: try_col!(row, "count", "错误节点排行"),
        });
    }

    Ok(nodes)
}

pub async fn get_top_failure_hosts(
    db: &DbPool,
    since_minutes: i32,
    limit: i32,
) -> Result<Vec<HostFailureRate>, DbError> {
    if limit <= 0 {
        return Ok(Vec::new());
    }

    let minutes = validate_minutes(since_minutes)?;
    let pool = sqlite_pool(db)?;
    let rows = sqlx::query(
        r#"
WITH error_counts AS (
    SELECT
        TRIM(host) AS host,
        COUNT(*) AS failure_count
    FROM error_logs
    WHERE host IS NOT NULL
      AND TRIM(host) <> ''
      AND datetime(created_at) >= datetime('now', '-' || ? || ' minutes')
    GROUP BY TRIM(host)
),
connection_counts AS (
    SELECT
        TRIM(host) AS host,
        COUNT(*) AS total_count
    FROM connections
    WHERE host IS NOT NULL
      AND TRIM(host) <> ''
      AND datetime(COALESCE(close_time, last_observed_at, start_time)) >= datetime('now', '-' || ? || ' minutes')
    GROUP BY TRIM(host)
)
SELECT
    error_counts.host AS host,
    error_counts.failure_count AS failure_count,
    connection_counts.total_count AS total_count,
    CAST(error_counts.failure_count AS REAL) / CAST(connection_counts.total_count AS REAL) AS failure_rate
FROM error_counts
INNER JOIN connection_counts ON connection_counts.host = error_counts.host
WHERE connection_counts.total_count > 0
ORDER BY failure_rate DESC, failure_count DESC, host ASC
LIMIT ?;
"#,
    )
    .bind(minutes)
    .bind(minutes)
    .bind(i64::from(limit))
    .fetch_all(pool)
    .await
    .map_err(|error| DbError::QueryFailed(format!("查询域名失败率排行失败: {error}")))?;

    let mut hosts = Vec::with_capacity(rows.len());
    for row in rows {
        hosts.push(HostFailureRate {
            host: try_col!(row, "host", "域名失败率排行"),
            failure_count: try_col!(row, "failure_count", "域名失败率排行"),
            total_count: try_col!(row, "total_count", "域名失败率排行"),
            failure_rate: try_col!(row, "failure_rate", "域名失败率排行"),
        });
    }

    Ok(hosts)
}

pub async fn count_dns_errors(db: &DbPool, since_minutes: i32) -> Result<i64, DbError> {
    let minutes = validate_minutes(since_minutes)?;
    let pool = sqlite_pool(db)?;
    sqlx::query_scalar::<_, i64>(
        r#"
SELECT COUNT(*)
FROM error_logs
WHERE category = 'dns'
  AND datetime(created_at) >= datetime('now', '-' || ? || ' minutes');
"#,
    )
    .bind(minutes)
    .fetch_one(pool)
    .await
    .map_err(|error| DbError::QueryFailed(format!("统计 DNS 错误失败: {error}")))
}

pub async fn cleanup_old_error_logs(db: &DbPool, retain_days: i32) -> Result<usize, DbError> {
    let modifier = retention_modifier(retain_days)?;
    let pool = sqlite_pool(db)?;
    let result = sqlx::query(
        r#"
DELETE FROM error_logs
WHERE datetime(created_at) < datetime('now', ?);
"#,
    )
    .bind(modifier)
    .execute(pool)
    .await
    .map_err(|error| DbError::WriteFailed(format!("清理 error_logs 失败: {error}")))?;

    usize::try_from(result.rows_affected())
        .map_err(|error| DbError::WriteFailed(format!("清理 error_logs 结果溢出: {error}")))
}

async fn insert_error_log_in_tx(
    transaction: &mut Transaction<'_, Sqlite>,
    log: &ErrorLogInsert,
) -> Result<(), DbError> {
    sqlx::query(
        r#"
INSERT INTO error_logs (category, proxy_node, host, rule, message)
VALUES (?, ?, ?, ?, ?);
"#,
    )
    .bind(&log.category)
    .bind(log.proxy_node.as_deref())
    .bind(log.host.as_deref())
    .bind(log.rule.as_deref())
    .bind(&log.message)
    .execute(transaction.as_mut())
    .await
    .map_err(|error| DbError::WriteFailed(format!("批量写入 error_logs 失败: {error}")))?;

    Ok(())
}

#[allow(dead_code)]
fn map_error_log(row: sqlx::sqlite::SqliteRow) -> Result<ErrorLog, DbError> {
    Ok(ErrorLog {
        id: try_col!(row, "id", "错误日志"),
        category: try_col!(row, "category", "错误日志"),
        proxy_node: try_col!(row, "proxy_node", "错误日志"),
        host: try_col!(row, "host", "错误日志"),
        rule: try_col!(row, "rule", "错误日志"),
        message: try_col!(row, "message", "错误日志"),
        created_at: try_col!(row, "created_at", "错误日志"),
    })
}

fn validate_minutes(since_minutes: i32) -> Result<i64, DbError> {
    if since_minutes <= 0 {
        return Err(DbError::InvalidTimeWindow(
            "since_minutes 必须大于 0".to_string(),
        ));
    }

    Ok(i64::from(since_minutes))
}

fn retention_modifier(retain_days: i32) -> Result<String, DbError> {
    if retain_days < 0 {
        return Err(DbError::InvalidTimeWindow(
            "retain_days 不能为负数".to_string(),
        ));
    }

    Ok(format!("-{retain_days} days"))
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
    async fn insert_and_get_error_log_work() {
        let db = prepare_db().await;
        assert!(db.is_ok());
        let Ok(db) = db else {
            panic!("error log test database should be created");
        };

        let inserted = insert_error_log(
            &db,
            &ErrorLogInsert {
                category: "timeout".to_string(),
                proxy_node: Some("Proxy-A".to_string()),
                host: Some("api.example".to_string()),
                rule: Some("MATCH".to_string()),
                message: "i/o timeout".to_string(),
            },
        )
        .await;
        assert!(inserted.is_ok());
        let Ok(inserted) = inserted else {
            panic!("error log should be inserted");
        };

        let fetched = get_error_log_by_id(&db, inserted).await;
        assert!(fetched.is_ok());
        let Ok(fetched) = fetched else {
            panic!("error log should be queryable");
        };

        assert!(fetched.is_some());
        let Some(fetched) = fetched else {
            panic!("inserted error log should exist");
        };
        assert_eq!(fetched.category, "timeout");
        assert_eq!(fetched.proxy_node.as_deref(), Some("Proxy-A"));
        assert_eq!(fetched.host.as_deref(), Some("api.example"));
    }

    #[tokio::test]
    async fn batch_insert_and_aggregates_work() {
        let db = prepare_db().await;
        assert!(db.is_ok());
        let Ok(db) = db else {
            panic!("error log test database should be created");
        };

        let batch_inserted = insert_error_logs_batch(
            &db,
            &[
                ErrorLogInsert {
                    category: "timeout".to_string(),
                    proxy_node: Some("Proxy-A".to_string()),
                    host: Some("api.example".to_string()),
                    rule: Some("MATCH".to_string()),
                    message: "deadline exceeded".to_string(),
                },
                ErrorLogInsert {
                    category: "timeout".to_string(),
                    proxy_node: Some("Proxy-A".to_string()),
                    host: Some("api.example".to_string()),
                    rule: Some("MATCH".to_string()),
                    message: "i/o timeout".to_string(),
                },
                ErrorLogInsert {
                    category: "dns".to_string(),
                    proxy_node: Some("Proxy-B".to_string()),
                    host: Some("dns.example".to_string()),
                    rule: Some("RULE-SET".to_string()),
                    message: "no such host".to_string(),
                },
            ],
        )
        .await;
        assert!(batch_inserted.is_ok());
        let Ok(batch_inserted) = batch_inserted else {
            panic!("error log batch should be inserted");
        };
        assert_eq!(batch_inserted, 3);

        let pool = sqlite_pool(&db);
        assert!(pool.is_ok());
        let Ok(pool) = pool else {
            panic!("sqlite pool should be available");
        };

        let connection_seed = sqlx::query(
            r#"
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
        'MATCH',
        '["Proxy-A"]',
        strftime('%Y-%m-%dT%H:%M:%SZ', datetime('now', '-8 minutes')),
        strftime('%Y-%m-%dT%H:%M:%SZ', datetime('now', '-7 minutes')),
        strftime('%Y-%m-%dT%H:%M:%SZ', datetime('now', '-7 minutes'))
    ),
    (
        'conn-3',
        'api.example',
        'tcp',
        'HTTPS',
        'MATCH',
        '["Proxy-A"]',
        strftime('%Y-%m-%dT%H:%M:%SZ', datetime('now', '-6 minutes')),
        NULL,
        strftime('%Y-%m-%dT%H:%M:%SZ', datetime('now'))
    ),
    (
        'conn-4',
        'dns.example',
        'udp',
        'DNS',
        'RULE-SET',
        '["Proxy-B"]',
        strftime('%Y-%m-%dT%H:%M:%SZ', datetime('now', '-5 minutes')),
        NULL,
        strftime('%Y-%m-%dT%H:%M:%SZ', datetime('now'))
    );
"#,
        )
        .execute(pool)
        .await;
        assert!(connection_seed.is_ok());

        let counts = get_error_category_counts(&db, 30).await;
        assert!(counts.is_ok());
        let Ok(counts) = counts else {
            panic!("error categories should be queryable");
        };

        assert_eq!(counts.len(), 2);
        assert_eq!(counts[0].category, "timeout");
        assert_eq!(counts[0].count, 2);

        let top_nodes = get_top_error_nodes(&db, 30, 10).await;
        assert!(top_nodes.is_ok());
        let Ok(top_nodes) = top_nodes else {
            panic!("error nodes should be queryable");
        };

        assert_eq!(top_nodes.len(), 2);
        assert_eq!(top_nodes[0].proxy_node, "Proxy-A");
        assert_eq!(top_nodes[0].count, 2);

        let top_hosts = get_top_failure_hosts(&db, 30, 10).await;
        assert!(top_hosts.is_ok());
        let Ok(top_hosts) = top_hosts else {
            panic!("failure hosts should be queryable");
        };

        assert_eq!(top_hosts.len(), 2);
        assert_eq!(top_hosts[0].host, "dns.example");
        assert_eq!(top_hosts[0].failure_count, 1);
        assert_eq!(top_hosts[0].total_count, 1);
        assert_eq!(top_hosts[0].failure_rate, 1.0);
        assert_eq!(top_hosts[1].host, "api.example");
        assert_eq!(top_hosts[1].failure_count, 2);
        assert_eq!(top_hosts[1].total_count, 3);

        let dns_count = count_dns_errors(&db, 30).await;
        assert!(dns_count.is_ok());
        let Ok(dns_count) = dns_count else {
            panic!("dns error count should be queryable");
        };

        assert_eq!(dns_count, 1);
    }

    #[tokio::test]
    async fn cleanup_and_delete_remove_expected_rows() {
        let db = prepare_db().await;
        assert!(db.is_ok());
        let Ok(db) = db else {
            panic!("error log test database should be created");
        };

        let pool = sqlite_pool(&db);
        assert!(pool.is_ok());
        let Ok(pool) = pool else {
            panic!("sqlite pool should be available");
        };

        let seed = sqlx::query(
            r#"
INSERT INTO error_logs (category, proxy_node, host, rule, message, created_at) VALUES
    ('timeout', 'Proxy-A', 'old.example', 'MATCH', 'timeout old', datetime('now', '-10 days')),
    ('dns', 'Proxy-B', 'new.example', 'MATCH', 'no such host', datetime('now', '-1 day'));
"#,
        )
        .execute(pool)
        .await;
        assert!(seed.is_ok());

        let deleted = delete_error_log(&db, 2).await;
        assert!(deleted.is_ok());
        let Ok(deleted) = deleted else {
            panic!("error log should be deleted");
        };
        assert_eq!(deleted, 1);

        let cleaned = cleanup_old_error_logs(&db, 7).await;
        assert!(cleaned.is_ok());
        let Ok(cleaned) = cleaned else {
            panic!("old error logs should be cleaned");
        };
        assert_eq!(cleaned, 1);

        let remaining = get_error_logs_since(&db, 10_080).await;
        assert!(remaining.is_ok());
        let Ok(remaining) = remaining else {
            panic!("remaining error logs should be queryable");
        };
        assert!(remaining.is_empty());
    }
}
