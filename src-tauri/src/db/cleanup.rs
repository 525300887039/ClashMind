//! Database cleanup routines for expiring historical data without locking the database.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tauri_plugin_sql::DbPool;

use super::{sqlite_pool, DbError};
use crate::utils::time as time_utils;

// Connection-backed stats expose a 30-day window in the UI, so raw rows must outlive that range.
const CONNECTION_RETENTION_DAYS: i32 = 30;
const TRAFFIC_HOURLY_RETENTION_DAYS: i32 = 365;
const DOMAIN_STATS_RETENTION_DAYS: i32 = 90;
const GEOIP_CACHE_RETENTION_DAYS: i32 = 30;

/// Summary returned after a full cleanup run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CleanupReport {
    pub connections_deleted: usize,
    pub hourly_deleted: usize,
    pub domain_stats_deleted: usize,
    pub geoip_deleted: usize,
    pub executed_at: String,
}

/// Deletes raw connection rows older than the configured retention window.
///
/// # Errors
///
/// Returns [`DbError`] when the retention window is invalid or the delete query fails.
pub async fn cleanup_connections(db: &DbPool, retention_days: i32) -> Result<usize, DbError> {
    execute_cleanup(
        db,
        r#"
DELETE FROM connections
WHERE datetime(COALESCE(close_time, last_observed_at, start_time)) < datetime('now', ?);
"#,
        retention_days,
        "清理 connections 失败",
    )
    .await
}

/// Deletes hourly traffic aggregates older than the configured retention window.
///
/// # Errors
///
/// Returns [`DbError`] when the retention window is invalid or the delete query fails.
pub async fn cleanup_traffic_hourly(db: &DbPool, retention_days: i32) -> Result<usize, DbError> {
    execute_cleanup(
        db,
        r#"
DELETE FROM traffic_hourly
WHERE datetime(hour) < datetime('now', ?);
"#,
        retention_days,
        "清理 traffic_hourly 失败",
    )
    .await
}

/// Deletes domain statistics older than the configured retention window.
///
/// # Errors
///
/// Returns [`DbError`] when the retention window is invalid or the delete query fails.
pub async fn cleanup_domain_stats(db: &DbPool, retention_days: i32) -> Result<usize, DbError> {
    execute_cleanup(
        db,
        r#"
DELETE FROM domain_stats
WHERE date(day) < date('now', ?);
"#,
        retention_days,
        "清理 domain_stats 失败",
    )
    .await
}

/// Deletes stale GeoIP cache rows older than the configured retention window.
///
/// # Errors
///
/// Returns [`DbError`] when the retention window is invalid or the delete query fails.
pub async fn cleanup_geoip_cache(db: &DbPool, retention_days: i32) -> Result<usize, DbError> {
    execute_cleanup(
        db,
        r#"
DELETE FROM geoip_cache
WHERE datetime(updated_at) < datetime('now', ?);
"#,
        retention_days,
        "清理 geoip_cache 失败",
    )
    .await
}

/// Runs the full cleanup policy and returns the deleted row counts.
///
/// # Errors
///
/// Returns [`DbError`] when any cleanup query fails.
pub async fn run_full_cleanup(db: &DbPool) -> Result<CleanupReport, DbError> {
    let connections_deleted = cleanup_connections(db, CONNECTION_RETENTION_DAYS).await?;
    let hourly_deleted = cleanup_traffic_hourly(db, TRAFFIC_HOURLY_RETENTION_DAYS).await?;
    let domain_stats_deleted = cleanup_domain_stats(db, DOMAIN_STATS_RETENTION_DAYS).await?;
    let geoip_deleted = cleanup_geoip_cache(db, GEOIP_CACHE_RETENTION_DAYS).await?;

    Ok(CleanupReport {
        connections_deleted,
        hourly_deleted,
        domain_stats_deleted,
        geoip_deleted,
        executed_at: time_utils::format_utc(Utc::now()),
    })
}

async fn execute_cleanup(
    db: &DbPool,
    sql: &str,
    retention_days: i32,
    operation: &str,
) -> Result<usize, DbError> {
    let pool = sqlite_pool(db)?;
    let modifier = retention_modifier(retention_days)?;
    let result = sqlx::query(sql)
        .bind(modifier)
        .execute(pool)
        .await
        .map_err(|error| DbError::WriteFailed(format!("{operation}: {error}")))?;

    usize::try_from(result.rows_affected())
        .map_err(|error| DbError::WriteFailed(format!("{operation}结果溢出: {error}")))
}

fn retention_modifier(retention_days: i32) -> Result<String, DbError> {
    if retention_days < 0 {
        return Err(DbError::InvalidTimeWindow(
            "retention_days 不能为负数".to_string(),
        ));
    }

    Ok(format!("-{retention_days} days"))
}

#[cfg(test)]
mod tests {
    use sqlx::{sqlite::SqlitePoolOptions, Row};

    use super::*;

    async fn prepare_db() -> Result<DbPool, String> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .map_err(|error| error.to_string())?;

        sqlx::query(
            r#"
CREATE TABLE connections (
    id          TEXT PRIMARY KEY,
    host        TEXT NOT NULL,
    network     TEXT NOT NULL,
    conn_type   TEXT NOT NULL,
    rule        TEXT NOT NULL,
    proxy_chain TEXT NOT NULL,
    start_time  TEXT NOT NULL,
    close_time  TEXT,
    last_observed_at TEXT
);

CREATE TABLE traffic_hourly (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    hour       TEXT NOT NULL UNIQUE,
    upload     INTEGER NOT NULL DEFAULT 0,
    download   INTEGER NOT NULL DEFAULT 0,
    conn_count INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE domain_stats (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    domain     TEXT NOT NULL,
    day        TEXT NOT NULL,
    hit_count  INTEGER NOT NULL DEFAULT 0,
    upload     INTEGER NOT NULL DEFAULT 0,
    download   INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE geoip_cache (
    ip           TEXT PRIMARY KEY,
    country      TEXT,
    country_code TEXT,
    city         TEXT,
    latitude     REAL,
    longitude    REAL,
    updated_at   TEXT NOT NULL
);
"#,
        )
        .execute(&pool)
        .await
        .map_err(|error| error.to_string())?;

        Ok(DbPool::Sqlite(pool))
    }

    #[tokio::test]
    async fn cleanup_functions_delete_only_expired_rows() {
        let db = prepare_db().await;
        assert!(db.is_ok());
        let Ok(db) = db else {
            panic!("cleanup test database should be created");
        };

        let pool = sqlite_pool(&db);
        assert!(pool.is_ok());
        let Ok(pool) = pool else {
            panic!("sqlite pool should be available");
        };

        let seed = sqlx::query(
            r#"
INSERT INTO connections (id, host, network, conn_type, rule, proxy_chain, start_time) VALUES
    ('conn-old', 'old.example', 'tcp', 'HTTPS', 'MATCH', '["DIRECT"]', strftime('%Y-%m-%dT%H:%M:%SZ', datetime('now', '-40 days'))),
    ('conn-new', 'new.example', 'tcp', 'HTTPS', 'MATCH', '["DIRECT"]', strftime('%Y-%m-%dT%H:%M:%SZ', datetime('now', '-2 days'))),
    ('conn-long-lived', 'long-lived.example', 'tcp', 'HTTPS', 'MATCH', '["DIRECT"]', strftime('%Y-%m-%dT%H:%M:%SZ', datetime('now', '-60 days')));

UPDATE connections
SET
    close_time = strftime('%Y-%m-%dT%H:%M:%SZ', datetime('now', '-31 days')),
    last_observed_at = strftime('%Y-%m-%dT%H:%M:%SZ', datetime('now', '-31 days'))
WHERE id = 'conn-old';

UPDATE connections
SET last_observed_at = strftime('%Y-%m-%dT%H:%M:%SZ', datetime('now', '-1 day'))
WHERE id = 'conn-new';

UPDATE connections
SET last_observed_at = strftime('%Y-%m-%dT%H:%M:%SZ', datetime('now'))
WHERE id = 'conn-long-lived';

INSERT INTO traffic_hourly (hour, upload, download, conn_count) VALUES
    (strftime('%Y-%m-%dT%H:00:00Z', datetime('now', '-366 days')), 1, 2, 3),
    (strftime('%Y-%m-%dT%H:00:00Z', datetime('now', '-10 days')), 4, 5, 6);

INSERT INTO domain_stats (domain, day, hit_count, upload, download) VALUES
    ('legacy.example', date('now', '-91 days'), 1, 10, 20),
    ('recent.example', date('now', '-5 days'), 2, 30, 40);

INSERT INTO geoip_cache (ip, country, country_code, city, latitude, longitude, updated_at) VALUES
    ('1.1.1.1', 'Australia', 'AU', 'Sydney', NULL, NULL, datetime('now', '-31 days')),
    ('8.8.8.8', 'United States', 'US', 'Mountain View', NULL, NULL, datetime('now', '-1 day'));
"#,
        )
        .execute(pool)
        .await;
        assert!(seed.is_ok());

        let connections_deleted = cleanup_connections(&db, 30).await;
        assert!(connections_deleted.is_ok());
        let Ok(connections_deleted) = connections_deleted else {
            panic!("connection cleanup should succeed");
        };
        assert_eq!(connections_deleted, 1);

        let hourly_deleted = cleanup_traffic_hourly(&db, 365).await;
        assert!(hourly_deleted.is_ok());
        let Ok(hourly_deleted) = hourly_deleted else {
            panic!("hourly cleanup should succeed");
        };
        assert_eq!(hourly_deleted, 1);

        let domain_deleted = cleanup_domain_stats(&db, 90).await;
        assert!(domain_deleted.is_ok());
        let Ok(domain_deleted) = domain_deleted else {
            panic!("domain cleanup should succeed");
        };
        assert_eq!(domain_deleted, 1);

        let geoip_deleted = cleanup_geoip_cache(&db, 30).await;
        assert!(geoip_deleted.is_ok());
        let Ok(geoip_deleted) = geoip_deleted else {
            panic!("geoip cleanup should succeed");
        };
        assert_eq!(geoip_deleted, 1);

        let counts = sqlx::query(
            r#"
SELECT
    (SELECT COUNT(*) FROM connections) AS connection_count,
    (SELECT COUNT(*) FROM traffic_hourly) AS hourly_count,
    (SELECT COUNT(*) FROM domain_stats) AS domain_count,
    (SELECT COUNT(*) FROM geoip_cache) AS geoip_count;
"#,
        )
        .fetch_one(pool)
        .await;
        assert!(counts.is_ok());
        let Ok(counts) = counts else {
            panic!("remaining row counts should be queryable");
        };

        assert_eq!(counts.get::<i64, _>("connection_count"), 2);
        assert_eq!(counts.get::<i64, _>("hourly_count"), 1);
        assert_eq!(counts.get::<i64, _>("domain_count"), 1);
        assert_eq!(counts.get::<i64, _>("geoip_count"), 1);
    }

    #[tokio::test]
    async fn run_full_cleanup_returns_deleted_counts() {
        let db = prepare_db().await;
        assert!(db.is_ok());
        let Ok(db) = db else {
            panic!("cleanup test database should be created");
        };

        let pool = sqlite_pool(&db);
        assert!(pool.is_ok());
        let Ok(pool) = pool else {
            panic!("sqlite pool should be available");
        };

        let seed = sqlx::query(
            r#"
INSERT INTO connections (
    id, host, network, conn_type, rule, proxy_chain, start_time, close_time, last_observed_at
) VALUES
    (
        'conn-old',
        'old.example',
        'tcp',
        'HTTPS',
        'MATCH',
        '["DIRECT"]',
        strftime('%Y-%m-%dT%H:%M:%SZ', datetime('now', '-40 days')),
        strftime('%Y-%m-%dT%H:%M:%SZ', datetime('now', '-35 days')),
        strftime('%Y-%m-%dT%H:%M:%SZ', datetime('now', '-35 days'))
    );

INSERT INTO traffic_hourly (hour, upload, download, conn_count) VALUES
    (strftime('%Y-%m-%dT%H:00:00Z', datetime('now', '-400 days')), 1, 1, 1);

INSERT INTO domain_stats (domain, day, hit_count, upload, download) VALUES
    ('legacy.example', date('now', '-120 days'), 1, 1, 1);

INSERT INTO geoip_cache (ip, updated_at) VALUES
    ('1.1.1.1', datetime('now', '-40 days'));
"#,
        )
        .execute(pool)
        .await;
        assert!(seed.is_ok());

        let report = run_full_cleanup(&db).await;
        assert!(report.is_ok());
        let Ok(report) = report else {
            panic!("full cleanup should succeed");
        };

        assert_eq!(
            report,
            CleanupReport {
                connections_deleted: 1,
                hourly_deleted: 1,
                domain_stats_deleted: 1,
                geoip_deleted: 1,
                executed_at: report.executed_at.clone(),
            }
        );
        assert!(!report.executed_at.is_empty());
    }

    #[tokio::test]
    async fn cleanup_rejects_negative_retention_days() {
        let db = prepare_db().await;
        assert!(db.is_ok());
        let Ok(db) = db else {
            panic!("cleanup test database should be created");
        };

        let result = cleanup_connections(&db, -1).await;
        assert!(matches!(result, Err(DbError::InvalidTimeWindow(_))));
    }

    #[tokio::test]
    async fn cleanup_connections_keeps_long_lived_connections_with_recent_activity() {
        let db = prepare_db().await;
        assert!(db.is_ok());
        let Ok(db) = db else {
            panic!("cleanup test database should be created");
        };

        let pool = sqlite_pool(&db);
        assert!(pool.is_ok());
        let Ok(pool) = pool else {
            panic!("sqlite pool should be available");
        };

        let seed = sqlx::query(
            r#"
INSERT INTO connections (
    id, host, network, conn_type, rule, proxy_chain, start_time, close_time, last_observed_at
) VALUES
    (
        'active-long-lived',
        'stream.example',
        'tcp',
        'HTTPS',
        'MATCH',
        '["DIRECT"]',
        strftime('%Y-%m-%dT%H:%M:%SZ', datetime('now', '-90 days')),
        NULL,
        strftime('%Y-%m-%dT%H:%M:%SZ', datetime('now'))
    ),
    (
        'recently-closed',
        'download.example',
        'tcp',
        'HTTPS',
        'MATCH',
        '["DIRECT"]',
        strftime('%Y-%m-%dT%H:%M:%SZ', datetime('now', '-45 days')),
        strftime('%Y-%m-%dT%H:%M:%SZ', datetime('now', '-2 days')),
        strftime('%Y-%m-%dT%H:%M:%SZ', datetime('now', '-2 days'))
    ),
    (
        'expired-closed',
        'legacy.example',
        'tcp',
        'HTTPS',
        'MATCH',
        '["DIRECT"]',
        strftime('%Y-%m-%dT%H:%M:%SZ', datetime('now', '-90 days')),
        strftime('%Y-%m-%dT%H:%M:%SZ', datetime('now', '-31 days')),
        strftime('%Y-%m-%dT%H:%M:%SZ', datetime('now', '-31 days'))
    );
"#,
        )
        .execute(pool)
        .await;
        assert!(seed.is_ok());

        let deleted = cleanup_connections(&db, 30).await;
        assert!(deleted.is_ok());
        let Ok(deleted) = deleted else {
            panic!("connection cleanup should succeed");
        };
        assert_eq!(deleted, 1);

        let remaining_ids = sqlx::query(
            r#"
SELECT id
FROM connections
ORDER BY id ASC;
"#,
        )
        .fetch_all(pool)
        .await;
        assert!(remaining_ids.is_ok());
        let Ok(remaining_ids) = remaining_ids else {
            panic!("remaining connections should be queryable");
        };

        let ids: Vec<String> = remaining_ids.into_iter().map(|row| row.get("id")).collect();
        assert_eq!(ids, vec!["active-long-lived", "recently-closed"]);
    }
}
