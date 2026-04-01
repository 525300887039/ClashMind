use std::collections::HashMap;

use sqlx::{Row, Sqlite, Transaction};
use tauri_plugin_sql::DbPool;

use crate::collector::ws_client::ConnectionRecord;

use super::{
    repo_domain::{self, DomainStatsUpdate},
    repo_geoip::{self, IpTrafficStatsUpdate},
    repo_traffic::{self, TrafficSampleInsert},
    sqlite_pool, DbError,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConnectionOverview {
    pub total_connections: i64,
    pub total_upload: i64,
    pub total_download: i64,
    pub active_connections: i64,
    pub unique_domains: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuleStatRow {
    pub rule: String,
    pub hit_count: i64,
    pub upload: i64,
    pub download: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GeoIpTrafficRow {
    pub dst_ip: String,
    pub conn_count: i64,
    pub total_traffic: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleStatsUpdate {
    pub rule: String,
    pub day: String,
    pub hit_count: i64,
    pub upload: i64,
    pub download: i64,
}

#[allow(dead_code)]
pub async fn batch_insert_connections(
    db: &DbPool,
    records: &[ConnectionRecord],
) -> Result<(), DbError> {
    if records.is_empty() {
        return Ok(());
    }

    let pool = sqlite_pool(db)?;
    let mut transaction = pool
        .begin()
        .await
        .map_err(|error| DbError::TransactionFailed(error.to_string()))?;

    batch_insert_connections_in_tx(&mut transaction, records).await?;
    transaction
        .commit()
        .await
        .map_err(|error| DbError::TransactionFailed(error.to_string()))?;

    Ok(())
}

pub async fn update_close_time(db: &DbPool, id: &str, close_time: &str) -> Result<(), DbError> {
    let pool = sqlite_pool(db)?;

    sqlx::query(
        r#"
UPDATE connections
SET close_time = ?
WHERE id = ?;
"#,
    )
    .bind(close_time)
    .bind(id)
    .execute(pool)
    .await
    .map_err(|error| {
        DbError::WriteFailed(format!("更新连接关闭时间失败: id={id}, error={error}"))
    })?;

    Ok(())
}

pub async fn list_open_connections(
    db: &DbPool,
) -> Result<HashMap<String, ConnectionRecord>, DbError> {
    let pool = sqlite_pool(db)?;
    let rows = sqlx::query(
        r#"
SELECT
    id,
    host,
    dst_ip,
    dst_port,
    src_ip,
    src_port,
    network,
    conn_type,
    rule,
    rule_payload,
    proxy_chain,
    upload,
    download,
    start_time,
    last_observed_at
FROM connections
WHERE close_time IS NULL;
"#,
    )
    .fetch_all(pool)
    .await
    .map_err(|error| DbError::QueryFailed(format!("查询打开连接失败: {error}")))?;

    let mut records = HashMap::with_capacity(rows.len());
    for row in rows {
        let record = ConnectionRecord {
            id: row
                .try_get("id")
                .map_err(|error| DbError::QueryFailed(format!("读取连接 id 失败: {error}")))?,
            host: row
                .try_get("host")
                .map_err(|error| DbError::QueryFailed(format!("读取连接 host 失败: {error}")))?,
            dst_ip: row
                .try_get("dst_ip")
                .map_err(|error| DbError::QueryFailed(format!("读取连接 dst_ip 失败: {error}")))?,
            dst_port: row.try_get("dst_port").map_err(|error| {
                DbError::QueryFailed(format!("读取连接 dst_port 失败: {error}"))
            })?,
            src_ip: row
                .try_get("src_ip")
                .map_err(|error| DbError::QueryFailed(format!("读取连接 src_ip 失败: {error}")))?,
            src_port: row.try_get("src_port").map_err(|error| {
                DbError::QueryFailed(format!("读取连接 src_port 失败: {error}"))
            })?,
            network: row
                .try_get("network")
                .map_err(|error| DbError::QueryFailed(format!("读取连接 network 失败: {error}")))?,
            conn_type: row.try_get("conn_type").map_err(|error| {
                DbError::QueryFailed(format!("读取连接 conn_type 失败: {error}"))
            })?,
            rule: row
                .try_get("rule")
                .map_err(|error| DbError::QueryFailed(format!("读取连接 rule 失败: {error}")))?,
            rule_payload: row.try_get("rule_payload").map_err(|error| {
                DbError::QueryFailed(format!("读取连接 rule_payload 失败: {error}"))
            })?,
            proxy_chain: row.try_get("proxy_chain").map_err(|error| {
                DbError::QueryFailed(format!("读取连接 proxy_chain 失败: {error}"))
            })?,
            upload: row
                .try_get("upload")
                .map_err(|error| DbError::QueryFailed(format!("读取连接 upload 失败: {error}")))?,
            download: row.try_get("download").map_err(|error| {
                DbError::QueryFailed(format!("读取连接 download 失败: {error}"))
            })?,
            start_time: row.try_get("start_time").map_err(|error| {
                DbError::QueryFailed(format!("读取连接 start_time 失败: {error}"))
            })?,
            last_observed_at: row.try_get("last_observed_at").map_err(|error| {
                DbError::QueryFailed(format!("读取连接 last_observed_at 失败: {error}"))
            })?,
        };
        records.insert(record.id.clone(), record);
    }

    Ok(records)
}

pub(crate) async fn get_overview(db: &DbPool, days: i32) -> Result<ConnectionOverview, DbError> {
    let pool = sqlite_pool(db)?;
    let row = sqlx::query(
        r#"
WITH window AS (
    SELECT date('now', '-' || ? || ' days') AS window_start
),
scoped_connections AS (
    SELECT
        id,
        close_time,
        last_observed_at,
        start_time
    FROM connections, window
    WHERE date(COALESCE(close_time, last_observed_at, start_time)) >= window.window_start
),
daily_traffic_candidates AS (
    SELECT
        day,
        upload,
        download,
        upload + download AS total_bytes,
        2 AS source_priority
    FROM traffic_daily, window
    WHERE date(day) >= window.window_start

    UNION ALL

    SELECT
        day,
        COALESCE(SUM(upload), 0) AS upload,
        COALESCE(SUM(download), 0) AS download,
        COALESCE(SUM(upload), 0) + COALESCE(SUM(download), 0) AS total_bytes,
        1 AS source_priority
    FROM domain_stats, window
    WHERE date(day) >= window.window_start
    GROUP BY day
),
daily_traffic_totals AS (
    SELECT
        day,
        upload,
        download
    FROM (
        SELECT
            day,
            upload,
            download,
            ROW_NUMBER() OVER (
                PARTITION BY day
                ORDER BY total_bytes DESC, source_priority DESC
            ) AS row_number
        FROM daily_traffic_candidates
    )
    WHERE row_number = 1
)
SELECT
    (
        SELECT COUNT(*)
        FROM scoped_connections
    ) AS total_connections,
    (
        SELECT COALESCE(SUM(upload), 0)
        FROM daily_traffic_totals
    ) AS total_upload,
    (
        SELECT COALESCE(SUM(download), 0)
        FROM daily_traffic_totals
    ) AS total_download,
    (
        SELECT COUNT(*)
        FROM scoped_connections
        WHERE close_time IS NULL
    ) AS active_connections,
    (
        SELECT COUNT(DISTINCT domain)
        FROM domain_stats, window
        WHERE date(day) >= window.window_start
          AND TRIM(domain) <> ''
    ) AS unique_domains;
"#,
    )
    .bind(i64::from(days.max(0)))
    .fetch_one(pool)
    .await
    .map_err(|error| DbError::QueryFailed(format!("查询统计概览失败: {error}")))?;

    Ok(ConnectionOverview {
        total_connections: row.try_get("total_connections").map_err(|error| {
            DbError::QueryFailed(format!("读取统计概览 total_connections 失败: {error}"))
        })?,
        total_upload: row.try_get("total_upload").map_err(|error| {
            DbError::QueryFailed(format!("读取统计概览 total_upload 失败: {error}"))
        })?,
        total_download: row.try_get("total_download").map_err(|error| {
            DbError::QueryFailed(format!("读取统计概览 total_download 失败: {error}"))
        })?,
        active_connections: row.try_get("active_connections").map_err(|error| {
            DbError::QueryFailed(format!("读取统计概览 active_connections 失败: {error}"))
        })?,
        unique_domains: row.try_get("unique_domains").map_err(|error| {
            DbError::QueryFailed(format!("读取统计概览 unique_domains 失败: {error}"))
        })?,
    })
}

pub(crate) async fn query_rule_stats(
    db: &DbPool,
    days: i32,
    limit: i32,
) -> Result<Vec<RuleStatRow>, DbError> {
    if limit <= 0 {
        return Ok(Vec::new());
    }

    let pool = sqlite_pool(db)?;
    let rows = sqlx::query(
        r#"
SELECT
    COALESCE(NULLIF(TRIM(rule), ''), 'UNKNOWN') AS rule,
    COALESCE(SUM(hit_count), 0) AS hit_count,
    COALESCE(SUM(upload), 0) AS upload,
    COALESCE(SUM(download), 0) AS download
FROM rule_stats
WHERE date(day) >= date('now', '-' || ? || ' days')
GROUP BY COALESCE(NULLIF(TRIM(rule), ''), 'UNKNOWN')
ORDER BY
    (COALESCE(SUM(upload), 0) + COALESCE(SUM(download), 0)) DESC,
    COALESCE(SUM(hit_count), 0) DESC,
    rule ASC
LIMIT ?;
"#,
    )
    .bind(i64::from(days.max(0)))
    .bind(i64::from(limit.max(0)))
    .fetch_all(pool)
    .await
    .map_err(|error| DbError::QueryFailed(format!("查询规则统计失败: {error}")))?;

    let mut rules = Vec::with_capacity(rows.len());
    for row in rows {
        rules.push(RuleStatRow {
            rule: row.try_get("rule").map_err(|error| {
                DbError::QueryFailed(format!("读取规则统计 rule 失败: {error}"))
            })?,
            hit_count: row.try_get("hit_count").map_err(|error| {
                DbError::QueryFailed(format!("读取规则统计 hit_count 失败: {error}"))
            })?,
            upload: row.try_get("upload").map_err(|error| {
                DbError::QueryFailed(format!("读取规则统计 upload 失败: {error}"))
            })?,
            download: row.try_get("download").map_err(|error| {
                DbError::QueryFailed(format!("读取规则统计 download 失败: {error}"))
            })?,
        });
    }

    Ok(rules)
}

pub(crate) async fn query_geo_traffic_by_ip(
    db: &DbPool,
    days: i32,
) -> Result<Vec<GeoIpTrafficRow>, DbError> {
    let pool = sqlite_pool(db)?;
    let rows = sqlx::query(
        r#"
WITH connection_counts AS (
    SELECT
        dst_ip,
        COUNT(*) AS conn_count
    FROM connections
    WHERE dst_ip IS NOT NULL
      AND TRIM(dst_ip) <> ''
      AND date(COALESCE(close_time, last_observed_at, start_time)) >= date('now', '-' || ? || ' days')
    GROUP BY dst_ip
),
traffic_totals AS (
    SELECT
        dst_ip,
        COALESCE(SUM(upload + download), 0) AS total_traffic
    FROM ip_traffic_daily
    WHERE date(day) >= date('now', '-' || ? || ' days')
    GROUP BY dst_ip
),
ip_keys AS (
    SELECT dst_ip FROM connection_counts
    UNION
    SELECT dst_ip FROM traffic_totals
)
SELECT
    ip_keys.dst_ip AS dst_ip,
    COALESCE(connection_counts.conn_count, 0) AS conn_count,
    COALESCE(traffic_totals.total_traffic, 0) AS total_traffic
FROM ip_keys
LEFT JOIN connection_counts ON connection_counts.dst_ip = ip_keys.dst_ip
LEFT JOIN traffic_totals ON traffic_totals.dst_ip = ip_keys.dst_ip
ORDER BY total_traffic DESC, conn_count DESC, dst_ip ASC;
"#,
    )
    .bind(i64::from(days.max(0)))
    .bind(i64::from(days.max(0)))
    .fetch_all(pool)
    .await
    .map_err(|error| DbError::QueryFailed(format!("查询 GeoIP 统计候选失败: {error}")))?;

    let mut stats = Vec::with_capacity(rows.len());
    for row in rows {
        stats.push(GeoIpTrafficRow {
            dst_ip: row
                .try_get("dst_ip")
                .map_err(|error| DbError::QueryFailed(format!("读取 dst_ip 失败: {error}")))?,
            conn_count: row.try_get("conn_count").map_err(|error| {
                DbError::QueryFailed(format!("读取 GeoIP 统计 conn_count 失败: {error}"))
            })?,
            total_traffic: row.try_get("total_traffic").map_err(|error| {
                DbError::QueryFailed(format!("读取 GeoIP 统计 total_traffic 失败: {error}"))
            })?,
        });
    }

    Ok(stats)
}

pub(crate) async fn batch_upsert_rule_stats_in_tx(
    transaction: &mut Transaction<'_, Sqlite>,
    updates: &[RuleStatsUpdate],
) -> Result<(), DbError> {
    if updates.is_empty() {
        return Ok(());
    }

    for update in updates {
        sqlx::query(
            r#"
INSERT INTO rule_stats (rule, day, hit_count, upload, download)
VALUES (?, ?, ?, ?, ?)
ON CONFLICT(rule, day)
DO UPDATE SET
    hit_count = hit_count + excluded.hit_count,
    upload = upload + excluded.upload,
    download = download + excluded.download;
"#,
        )
        .bind(&update.rule)
        .bind(&update.day)
        .bind(update.hit_count)
        .bind(update.upload)
        .bind(update.download)
        .execute(transaction.as_mut())
        .await
        .map_err(|error| {
            DbError::WriteFailed(format!(
                "写入规则统计失败: rule={}, day={}, error={error}",
                update.rule, update.day
            ))
        })?;
    }

    Ok(())
}

pub(crate) async fn persist_connection_batch(
    db: &DbPool,
    records: &[ConnectionRecord],
    observation_records: &[ConnectionRecord],
    domain_updates: &[DomainStatsUpdate],
    rule_updates: &[RuleStatsUpdate],
    ip_traffic_updates: &[IpTrafficStatsUpdate],
    traffic_samples: &[TrafficSampleInsert],
) -> Result<(), DbError> {
    if records.is_empty()
        && observation_records.is_empty()
        && domain_updates.is_empty()
        && rule_updates.is_empty()
        && ip_traffic_updates.is_empty()
        && traffic_samples.is_empty()
    {
        return Ok(());
    }

    let pool = sqlite_pool(db)?;
    let mut transaction = pool
        .begin()
        .await
        .map_err(|error| DbError::TransactionFailed(error.to_string()))?;

    batch_insert_connections_in_tx(&mut transaction, records).await?;
    batch_update_last_observed_at_in_tx(&mut transaction, observation_records).await?;
    repo_domain::batch_upsert_domain_stats_in_tx(&mut transaction, domain_updates).await?;
    batch_upsert_rule_stats_in_tx(&mut transaction, rule_updates).await?;
    repo_geoip::batch_upsert_ip_traffic_stats_in_tx(&mut transaction, ip_traffic_updates).await?;
    repo_traffic::batch_insert_traffic_samples_in_tx(&mut transaction, traffic_samples).await?;
    repo_traffic::aggregate_samples_in_tx(&mut transaction, traffic_samples).await?;
    transaction
        .commit()
        .await
        .map_err(|error| DbError::TransactionFailed(error.to_string()))?;

    Ok(())
}

async fn batch_insert_connections_in_tx(
    transaction: &mut Transaction<'_, Sqlite>,
    records: &[ConnectionRecord],
) -> Result<(), DbError> {
    for record in records {
        sqlx::query(
            r#"
INSERT OR REPLACE INTO connections (
    id,
    host,
    dst_ip,
    dst_port,
    src_ip,
    src_port,
    network,
    conn_type,
    rule,
    rule_payload,
    proxy_chain,
    upload,
    download,
    start_time,
    last_observed_at
)
VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?);
"#,
        )
        .bind(&record.id)
        .bind(&record.host)
        .bind(record.dst_ip.as_deref())
        .bind(record.dst_port)
        .bind(record.src_ip.as_deref())
        .bind(record.src_port)
        .bind(&record.network)
        .bind(&record.conn_type)
        .bind(&record.rule)
        .bind(record.rule_payload.as_deref())
        .bind(&record.proxy_chain)
        .bind(record.upload)
        .bind(record.download)
        .bind(&record.start_time)
        .bind(match record.last_observed_at.as_deref() {
            Some(value) => value,
            None => &record.start_time,
        })
        .execute(transaction.as_mut())
        .await
        .map_err(|error| {
            DbError::WriteFailed(format!(
                "批量写入连接记录失败: id={}, error={error}",
                record.id
            ))
        })?;
    }

    Ok(())
}

async fn batch_update_last_observed_at_in_tx(
    transaction: &mut Transaction<'_, Sqlite>,
    records: &[ConnectionRecord],
) -> Result<(), DbError> {
    if records.is_empty() {
        return Ok(());
    }

    for record in records {
        let observed_at = match record.last_observed_at.as_deref() {
            Some(value) => value,
            None => &record.start_time,
        };

        sqlx::query(
            r#"
UPDATE connections
SET last_observed_at = ?
WHERE id = ?;
"#,
        )
        .bind(observed_at)
        .bind(&record.id)
        .execute(transaction.as_mut())
        .await
        .map_err(|error| {
            DbError::WriteFailed(format!(
                "更新连接观测时间失败: id={}, error={error}",
                record.id
            ))
        })?;
    }

    Ok(())
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
    id               TEXT PRIMARY KEY,
    host             TEXT NOT NULL,
    dst_ip           TEXT,
    dst_port         INTEGER,
    src_ip           TEXT,
    src_port         INTEGER,
    network          TEXT NOT NULL,
    conn_type        TEXT NOT NULL,
    rule             TEXT NOT NULL,
    rule_payload     TEXT,
    proxy_chain      TEXT NOT NULL,
    upload           INTEGER NOT NULL DEFAULT 0,
    download         INTEGER NOT NULL DEFAULT 0,
    start_time       TEXT NOT NULL,
    close_time       TEXT,
    created_at       TEXT NOT NULL DEFAULT (datetime('now')),
    last_observed_at TEXT
);

CREATE TABLE traffic_daily (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    day        TEXT NOT NULL UNIQUE,
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
    download   INTEGER NOT NULL DEFAULT 0,
    UNIQUE(domain, day)
);

CREATE TABLE rule_stats (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    rule       TEXT NOT NULL,
    day        TEXT NOT NULL,
    hit_count  INTEGER NOT NULL DEFAULT 0,
    upload     INTEGER NOT NULL DEFAULT 0,
    download   INTEGER NOT NULL DEFAULT 0,
    UNIQUE(rule, day)
);

CREATE TABLE ip_traffic_daily (
    id       INTEGER PRIMARY KEY AUTOINCREMENT,
    dst_ip   TEXT NOT NULL,
    day      TEXT NOT NULL,
    upload   INTEGER NOT NULL DEFAULT 0,
    download INTEGER NOT NULL DEFAULT 0,
    UNIQUE(dst_ip, day)
);
"#,
        )
        .execute(&pool)
        .await
        .map_err(|error| error.to_string())?;

        Ok(DbPool::Sqlite(pool))
    }

    #[tokio::test]
    async fn batch_update_last_observed_at_only_updates_observation_timestamp() {
        let db = prepare_db().await;
        assert!(db.is_ok());
        let Ok(db) = db else {
            panic!("sqlite pool should be created");
        };

        let pool = sqlite_pool(&db);
        assert!(pool.is_ok());
        let Ok(pool) = pool else {
            panic!("sqlite pool should be available");
        };

        let seed = sqlx::query(
            r#"
INSERT INTO connections (
    id, host, network, conn_type, rule, proxy_chain, upload, download, start_time, last_observed_at
)
VALUES ('conn-1', 'example.com', 'tcp', 'HTTPS', 'MATCH', '["DIRECT"]', 123, 456, '2026-03-31T08:00:00Z', '2026-03-31T08:05:00Z');
"#,
        )
        .execute(pool)
        .await;
        assert!(seed.is_ok());

        let transaction = pool.begin().await;
        assert!(transaction.is_ok());
        let Ok(mut transaction) = transaction else {
            panic!("transaction should be created");
        };

        let update = batch_update_last_observed_at_in_tx(
            &mut transaction,
            &[ConnectionRecord {
                id: "conn-1".into(),
                host: "example.com".into(),
                dst_ip: None,
                dst_port: None,
                src_ip: None,
                src_port: None,
                network: "tcp".into(),
                conn_type: "HTTPS".into(),
                rule: "MATCH".into(),
                rule_payload: None,
                proxy_chain: "[\"DIRECT\"]".into(),
                upload: 999,
                download: 999,
                start_time: "2026-03-31T08:00:00Z".into(),
                last_observed_at: Some("2026-03-31T09:00:00Z".into()),
            }],
        )
        .await;
        assert!(update.is_ok());
        let commit = transaction.commit().await;
        assert!(commit.is_ok());

        let row = sqlx::query(
            r#"
SELECT upload, download, last_observed_at
FROM connections
WHERE id = 'conn-1';
"#,
        )
        .fetch_one(pool)
        .await;
        assert!(row.is_ok());
        let Ok(row) = row else {
            panic!("updated row should be queryable");
        };

        assert_eq!(row.get::<i64, _>("upload"), 123);
        assert_eq!(row.get::<i64, _>("download"), 456);
        assert_eq!(
            row.get::<Option<String>, _>("last_observed_at").as_deref(),
            Some("2026-03-31T09:00:00Z")
        );
    }

    #[tokio::test]
    async fn get_overview_and_rule_stats_return_aggregates() {
        let db = prepare_db().await;
        assert!(db.is_ok());
        let Ok(db) = db else {
            panic!("test database should be created");
        };

        let pool = sqlite_pool(&db);
        assert!(pool.is_ok());
        let Ok(pool) = pool else {
            panic!("sqlite pool should be available");
        };

        let insert = sqlx::query(
            r#"
INSERT INTO connections (
    id, host, network, conn_type, rule, proxy_chain, upload, download, start_time, close_time, last_observed_at
) VALUES
    (
        'conn-1',
        'alpha.example',
        'tcp',
        'HTTPS',
        'MATCH',
        '["DIRECT"]',
        100,
        200,
        strftime('%Y-%m-%dT%H:%M:%SZ', datetime('now', '-3 days')),
        NULL,
        strftime('%Y-%m-%dT%H:%M:%SZ', datetime('now'))
    ),
    (
        'conn-2',
        'beta.example',
        'tcp',
        'HTTPS',
        'DOMAIN-SUFFIX',
        '["Proxy"]',
        50,
        25,
        strftime('%Y-%m-%dT%H:%M:%SZ', datetime('now', '-3 hours')),
        strftime('%Y-%m-%dT%H:%M:%SZ', datetime('now', '-2 hours')),
        strftime('%Y-%m-%dT%H:%M:%SZ', datetime('now', '-2 hours'))
    ),
    (
        'conn-3',
        'legacy.example',
        'udp',
        'QUIC',
        'LEGACY',
        '["DIRECT"]',
        5,
        6,
        strftime('%Y-%m-%dT%H:%M:%SZ', datetime('now', '-5 days')),
        strftime('%Y-%m-%dT%H:%M:%SZ', datetime('now', '-4 days')),
        strftime('%Y-%m-%dT%H:%M:%SZ', datetime('now', '-4 days'))
    );

INSERT INTO traffic_daily (day, upload, download, conn_count) VALUES
    (date('now'), 70, 90, 2),
    (date('now', '-1 day'), 999, 999, 9);

INSERT INTO domain_stats (domain, day, hit_count, upload, download) VALUES
    ('alpha.example', date('now'), 1, 40, 60),
    ('legacy.example', date('now', '-1 day'), 1, 999, 999);

INSERT INTO rule_stats (rule, day, hit_count, upload, download) VALUES
    ('MATCH', date('now'), 2, 50, 60),
    ('DOMAIN-SUFFIX', date('now'), 1, 20, 30),
    ('LEGACY', date('now', '-1 day'), 9, 999, 999);
"#,
        )
        .execute(pool)
        .await;
        assert!(insert.is_ok());

        let overview = get_overview(&db, 0).await;
        assert!(overview.is_ok());
        let Ok(overview) = overview else {
            panic!("overview should be queryable");
        };

        assert_eq!(overview.total_connections, 2);
        assert_eq!(overview.total_upload, 70);
        assert_eq!(overview.total_download, 90);
        assert_eq!(overview.active_connections, 1);
        assert_eq!(overview.unique_domains, 1);

        let rules = query_rule_stats(&db, 0, 10).await;
        assert!(rules.is_ok());
        let Ok(rules) = rules else {
            panic!("rule stats should be queryable");
        };

        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].rule, "MATCH");
        assert_eq!(rules[0].hit_count, 2);
        assert_eq!(rules[1].rule, "DOMAIN-SUFFIX");
    }

    #[tokio::test]
    async fn query_geo_traffic_by_ip_groups_connections_by_destination_ip() {
        let db = prepare_db().await;
        assert!(db.is_ok());
        let Ok(db) = db else {
            panic!("test database should be created");
        };

        let pool = sqlite_pool(&db);
        assert!(pool.is_ok());
        let Ok(pool) = pool else {
            panic!("sqlite pool should be available");
        };

        let insert = sqlx::query(
            r#"
INSERT INTO connections (
    id, host, dst_ip, network, conn_type, rule, proxy_chain, upload, download, start_time, close_time, last_observed_at
) VALUES
    (
        'conn-geo-1',
        'alpha.example',
        '1.1.1.1',
        'tcp',
        'HTTPS',
        'MATCH',
        '["DIRECT"]',
        100,
        200,
        strftime('%Y-%m-%dT%H:%M:%SZ', datetime('now', '-2 hours')),
        NULL,
        strftime('%Y-%m-%dT%H:%M:%SZ', datetime('now'))
    ),
    (
        'conn-geo-2',
        'beta.example',
        '1.1.1.1',
        'tcp',
        'HTTPS',
        'MATCH',
        '["DIRECT"]',
        50,
        25,
        strftime('%Y-%m-%dT%H:%M:%SZ', datetime('now', '-1 hours')),
        strftime('%Y-%m-%dT%H:%M:%SZ', datetime('now')),
        strftime('%Y-%m-%dT%H:%M:%SZ', datetime('now'))
    ),
    (
        'conn-geo-3',
        'gamma.example',
        '8.8.8.8',
        'tcp',
        'HTTPS',
        'MATCH',
        '["DIRECT"]',
        10,
        15,
        strftime('%Y-%m-%dT%H:%M:%SZ', datetime('now', '-30 minutes')),
        NULL,
        strftime('%Y-%m-%dT%H:%M:%SZ', datetime('now'))
    );

INSERT INTO ip_traffic_daily (dst_ip, day, upload, download) VALUES
    ('1.1.1.1', date('now'), 15, 25),
    ('1.1.1.1', date('now', '-1 day'), 20, 30),
    ('8.8.8.8', date('now'), 4, 6);
"#,
        )
        .execute(pool)
        .await;
        assert!(insert.is_ok());

        let rows = query_geo_traffic_by_ip(&db, 1).await;
        assert!(rows.is_ok());
        let Ok(rows) = rows else {
            panic!("GeoIP traffic candidates should be queryable");
        };

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].dst_ip, "1.1.1.1");
        assert_eq!(rows[0].conn_count, 2);
        assert_eq!(rows[0].total_traffic, 90);
        assert_eq!(rows[1].dst_ip, "8.8.8.8");
        assert_eq!(rows[1].total_traffic, 10);
    }

    #[tokio::test]
    async fn query_geo_traffic_by_ip_avoids_overcounting_long_lived_connection_totals() {
        let db = prepare_db().await;
        assert!(db.is_ok());
        let Ok(db) = db else {
            panic!("test database should be created");
        };

        let pool = sqlite_pool(&db);
        assert!(pool.is_ok());
        let Ok(pool) = pool else {
            panic!("sqlite pool should be available");
        };

        let insert = sqlx::query(
            r#"
INSERT INTO connections (
    id, host, dst_ip, network, conn_type, rule, proxy_chain, upload, download, start_time, close_time, last_observed_at
) VALUES (
    'long-lived-conn',
    'stream.example',
    '9.9.9.9',
    'tcp',
    'HTTPS',
    'MATCH',
    '["DIRECT"]',
    900,
    1100,
    strftime('%Y-%m-%dT%H:%M:%SZ', datetime('now', '-30 days')),
    NULL,
    strftime('%Y-%m-%dT%H:%M:%SZ', datetime('now'))
);

INSERT INTO ip_traffic_daily (dst_ip, day, upload, download) VALUES
    ('9.9.9.9', date('now'), 12, 18);
"#,
        )
        .execute(pool)
        .await;
        assert!(insert.is_ok());

        let rows = query_geo_traffic_by_ip(&db, 0).await;
        assert!(rows.is_ok());
        let Ok(rows) = rows else {
            panic!("GeoIP traffic candidates should be queryable");
        };

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].dst_ip, "9.9.9.9");
        assert_eq!(rows[0].conn_count, 1);
        assert_eq!(rows[0].total_traffic, 30);
    }

    #[tokio::test]
    async fn get_overview_falls_back_to_domain_stats_when_daily_totals_are_missing_or_partial() {
        let db = prepare_db().await;
        assert!(db.is_ok());
        let Ok(db) = db else {
            panic!("test database should be created");
        };

        let pool = sqlite_pool(&db);
        assert!(pool.is_ok());
        let Ok(pool) = pool else {
            panic!("sqlite pool should be available");
        };

        let insert = sqlx::query(
            r#"
INSERT INTO connections (
    id, host, network, conn_type, rule, proxy_chain, upload, download, start_time, close_time, last_observed_at
) VALUES
    (
        'conn-1',
        'alpha.example',
        'tcp',
        'HTTPS',
        'MATCH',
        '["DIRECT"]',
        25,
        35,
        strftime('%Y-%m-%dT%H:%M:%SZ', datetime('now', '-1 hours')),
        NULL,
        strftime('%Y-%m-%dT%H:%M:%SZ', datetime('now'))
    ),
    (
        'conn-2',
        'beta.example',
        'tcp',
        'HTTPS',
        'MATCH',
        '["DIRECT"]',
        10,
        20,
        strftime('%Y-%m-%dT%H:%M:%SZ', datetime('now', '-1 day', '-2 hours')),
        strftime('%Y-%m-%dT%H:%M:%SZ', datetime('now', '-1 day', '-1 hours')),
        strftime('%Y-%m-%dT%H:%M:%SZ', datetime('now', '-1 day', '-1 hours'))
    );

INSERT INTO traffic_daily (day, upload, download, conn_count) VALUES
    (date('now'), 10, 15, 1);

INSERT INTO domain_stats (domain, day, hit_count, upload, download) VALUES
    ('alpha.example', date('now'), 1, 25, 35),
    ('beta.example', date('now', '-1 day'), 1, 40, 60);
"#,
        )
        .execute(pool)
        .await;
        assert!(insert.is_ok());

        let overview = get_overview(&db, 1).await;
        assert!(overview.is_ok());
        let Ok(overview) = overview else {
            panic!("overview should be queryable");
        };

        assert_eq!(overview.total_connections, 2);
        assert_eq!(overview.total_upload, 65);
        assert_eq!(overview.total_download, 95);
        assert_eq!(overview.active_connections, 1);
        assert_eq!(overview.unique_domains, 2);
    }
}
