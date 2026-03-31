use chrono::{DateTime, Datelike, LocalResult, SecondsFormat, TimeZone, Timelike, Utc};
use sqlx::{Row, Sqlite, Transaction};
use tauri_plugin_sql::DbPool;

use super::{sqlite_pool, DbError};

const AGGREGATE_HOURLY_SQL: &str = r#"
INSERT INTO traffic_hourly (hour, upload, download, conn_count)
SELECT
    strftime('%Y-%m-%dT%H:00:00Z', datetime(ts)) AS hour,
    SUM(upload) AS upload,
    SUM(download) AS download,
    SUM(conn_count) AS conn_count
FROM traffic_samples
WHERE datetime(ts) >= datetime(?)
  AND datetime(ts) < datetime(?)
GROUP BY strftime('%Y-%m-%dT%H:00:00Z', datetime(ts))
ON CONFLICT(hour) DO UPDATE SET
    upload = excluded.upload,
    download = excluded.download,
    conn_count = excluded.conn_count;
"#;

const AGGREGATE_DAILY_SQL: &str = r#"
INSERT INTO traffic_daily (day, upload, download, conn_count)
SELECT
    strftime('%Y-%m-%d', datetime(hour)) AS day,
    SUM(upload) AS upload,
    SUM(download) AS download,
    SUM(conn_count) AS conn_count
FROM traffic_hourly
WHERE datetime(hour) >= datetime(?)
  AND datetime(hour) < datetime(?)
GROUP BY strftime('%Y-%m-%d', datetime(hour))
ON CONFLICT(day) DO UPDATE SET
    upload = excluded.upload,
    download = excluded.download,
    conn_count = excluded.conn_count;
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrafficSampleInsert {
    pub ts: String,
    pub upload: i64,
    pub download: i64,
    pub conn_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TrafficBucketRow {
    pub time: String,
    pub upload: i64,
    pub download: i64,
    pub conn_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AggregationWindow {
    from: String,
    to: String,
}

/// Persists raw traffic deltas so later aggregation can bucket long-lived connections correctly.
///
/// # Errors
///
/// Returns [`DbError`] if any sample row cannot be inserted.
pub(crate) async fn batch_insert_traffic_samples_in_tx(
    transaction: &mut Transaction<'_, Sqlite>,
    samples: &[TrafficSampleInsert],
) -> Result<(), DbError> {
    if samples.is_empty() {
        return Ok(());
    }

    for sample in samples {
        sqlx::query(
            r#"
INSERT INTO traffic_samples (ts, upload, download, conn_count)
VALUES (?, ?, ?, ?);
"#,
        )
        .bind(&sample.ts)
        .bind(sample.upload)
        .bind(sample.download)
        .bind(sample.conn_count)
        .execute(transaction.as_mut())
        .await
        .map_err(|error| {
            DbError::WriteFailed(format!(
                "写入流量增量样本失败: ts={}, error={error}",
                sample.ts
            ))
        })?;
    }

    Ok(())
}

pub(crate) async fn aggregate_samples_in_tx(
    transaction: &mut Transaction<'_, Sqlite>,
    samples: &[TrafficSampleInsert],
) -> Result<(), DbError> {
    let Some(hourly_window) = build_hourly_window_for_samples(samples)? else {
        return Ok(());
    };
    let Some(daily_window) = build_daily_window_for_samples(samples)? else {
        return Ok(());
    };

    execute_aggregate_in_tx(
        transaction,
        AGGREGATE_HOURLY_SQL,
        &hourly_window.from,
        &hourly_window.to,
        "小时级流量聚合",
    )
    .await?;
    execute_aggregate_in_tx(
        transaction,
        AGGREGATE_DAILY_SQL,
        &daily_window.from,
        &daily_window.to,
        "日级流量聚合",
    )
    .await?;
    // Keep raw samples so later backfills can safely recompute older buckets.

    Ok(())
}

/// Aggregates raw traffic sample rows into `traffic_hourly` for the given ISO 8601 window.
///
/// # Errors
///
/// Returns [`DbError`] when the time window is invalid or the UPSERT query fails.
pub async fn aggregate_hourly(db: &DbPool, from: &str, to: &str) -> Result<usize, DbError> {
    execute_aggregate(db, AGGREGATE_HOURLY_SQL, from, to, "小时级流量聚合").await
}

/// Aggregates hourly traffic rows into `traffic_daily` for the given ISO 8601 window.
///
/// # Errors
///
/// Returns [`DbError`] when the time window is invalid or the UPSERT query fails.
pub async fn aggregate_daily(db: &DbPool, from: &str, to: &str) -> Result<usize, DbError> {
    execute_aggregate(db, AGGREGATE_DAILY_SQL, from, to, "日级流量聚合").await
}

pub(crate) async fn query_hourly(
    db: &DbPool,
    from: &str,
    to: &str,
) -> Result<Vec<TrafficBucketRow>, DbError> {
    query_traffic(
        db,
        r#"
SELECT hour AS time, upload, download, conn_count
FROM traffic_hourly
WHERE datetime(hour) >= datetime(?)
  AND datetime(hour) < datetime(?)
ORDER BY datetime(hour) ASC;
"#,
        from,
        to,
        "查询小时级流量失败",
    )
    .await
}

pub(crate) async fn query_daily(
    db: &DbPool,
    from: &str,
    to: &str,
) -> Result<Vec<TrafficBucketRow>, DbError> {
    query_traffic(
        db,
        r#"
SELECT printf('%sT00:00:00Z', day) AS time, upload, download, conn_count
FROM traffic_daily
WHERE datetime(day) >= datetime(?)
  AND datetime(day) < datetime(?)
ORDER BY datetime(day) ASC;
"#,
        from,
        to,
        "查询日级流量失败",
    )
    .await
}

async fn execute_aggregate(
    db: &DbPool,
    sql: &str,
    from: &str,
    to: &str,
    operation: &str,
) -> Result<usize, DbError> {
    let pool = sqlite_pool(db)?;
    let (from, to) = normalize_window(from, to)?;
    let result = sqlx::query(sql)
        .bind(&from)
        .bind(&to)
        .execute(pool)
        .await
        .map_err(|error| DbError::WriteFailed(format!("{operation}失败: {error}")))?;

    usize::try_from(result.rows_affected())
        .map_err(|error| DbError::WriteFailed(format!("{operation}结果溢出: {error}")))
}

async fn query_traffic(
    db: &DbPool,
    sql: &str,
    from: &str,
    to: &str,
    operation: &str,
) -> Result<Vec<TrafficBucketRow>, DbError> {
    let pool = sqlite_pool(db)?;
    let (from, to) = normalize_window(from, to)?;
    let rows = sqlx::query(sql)
        .bind(&from)
        .bind(&to)
        .fetch_all(pool)
        .await
        .map_err(|error| DbError::QueryFailed(format!("{operation}: {error}")))?;

    let mut points = Vec::with_capacity(rows.len());
    for row in rows {
        points.push(TrafficBucketRow {
            time: row
                .try_get("time")
                .map_err(|error| DbError::QueryFailed(format!("读取流量时间失败: {error}")))?,
            upload: row
                .try_get("upload")
                .map_err(|error| DbError::QueryFailed(format!("读取流量 upload 失败: {error}")))?,
            download: row.try_get("download").map_err(|error| {
                DbError::QueryFailed(format!("读取流量 download 失败: {error}"))
            })?,
            conn_count: row.try_get("conn_count").map_err(|error| {
                DbError::QueryFailed(format!("读取流量 conn_count 失败: {error}"))
            })?,
        });
    }

    Ok(points)
}

async fn execute_aggregate_in_tx(
    transaction: &mut Transaction<'_, Sqlite>,
    sql: &str,
    from: &str,
    to: &str,
    operation: &str,
) -> Result<(), DbError> {
    let (from, to) = normalize_window(from, to)?;

    sqlx::query(sql)
        .bind(&from)
        .bind(&to)
        .execute(transaction.as_mut())
        .await
        .map_err(|error| DbError::WriteFailed(format!("{operation}失败: {error}")))?;

    Ok(())
}

fn build_hourly_window_for_samples(
    samples: &[TrafficSampleInsert],
) -> Result<Option<AggregationWindow>, DbError> {
    build_window_for_samples(samples, floor_to_hour, shift_hour)
}

fn build_daily_window_for_samples(
    samples: &[TrafficSampleInsert],
) -> Result<Option<AggregationWindow>, DbError> {
    build_window_for_samples(samples, floor_to_day, shift_day)
}

fn build_window_for_samples<FFloor, FShift>(
    samples: &[TrafficSampleInsert],
    floor_fn: FFloor,
    shift_fn: FShift,
) -> Result<Option<AggregationWindow>, DbError>
where
    FFloor: Fn(DateTime<Utc>) -> DateTime<Utc>,
    FShift: Fn(DateTime<Utc>) -> DateTime<Utc>,
{
    let mut min_ts: Option<DateTime<Utc>> = None;
    let mut max_ts: Option<DateTime<Utc>> = None;

    for sample in samples {
        let ts = parse_iso8601(&sample.ts, "sample.ts")?;
        min_ts = Some(match min_ts {
            Some(current) => current.min(ts),
            None => ts,
        });
        max_ts = Some(match max_ts {
            Some(current) => current.max(ts),
            None => ts,
        });
    }

    let (Some(min_ts), Some(max_ts)) = (min_ts, max_ts) else {
        return Ok(None);
    };

    let from = floor_fn(min_ts);
    let to = shift_fn(floor_fn(max_ts));

    Ok(Some(AggregationWindow {
        from: format_utc(from),
        to: format_utc(to),
    }))
}

fn floor_to_hour(value: DateTime<Utc>) -> DateTime<Utc> {
    match Utc.with_ymd_and_hms(value.year(), value.month(), value.day(), value.hour(), 0, 0) {
        LocalResult::Single(timestamp) => timestamp,
        _ => value,
    }
}

fn shift_hour(value: DateTime<Utc>) -> DateTime<Utc> {
    value + chrono::Duration::hours(1)
}

fn floor_to_day(value: DateTime<Utc>) -> DateTime<Utc> {
    match Utc.with_ymd_and_hms(value.year(), value.month(), value.day(), 0, 0, 0) {
        LocalResult::Single(timestamp) => timestamp,
        _ => value,
    }
}

fn shift_day(value: DateTime<Utc>) -> DateTime<Utc> {
    value + chrono::Duration::days(1)
}

fn normalize_window(from: &str, to: &str) -> Result<(String, String), DbError> {
    let from = parse_iso8601(from, "from")?;
    let to = parse_iso8601(to, "to")?;

    if from >= to {
        return Err(DbError::InvalidTimeWindow("from 必须早于 to".to_string()));
    }

    Ok((format_utc(from), format_utc(to)))
}

fn parse_iso8601(value: &str, label: &str) -> Result<DateTime<Utc>, DbError> {
    DateTime::parse_from_rfc3339(value)
        .map(|timestamp| timestamp.with_timezone(&Utc))
        .map_err(|error| {
            DbError::InvalidTimeWindow(format!("{label} 不是有效的 ISO 8601 时间: {error}"))
        })
}

fn format_utc(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Secs, true)
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
CREATE TABLE traffic_samples (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    ts         TEXT NOT NULL,
    upload     INTEGER NOT NULL DEFAULT 0,
    download   INTEGER NOT NULL DEFAULT 0,
    conn_count INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE traffic_hourly (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    hour       TEXT NOT NULL UNIQUE,
    upload     INTEGER NOT NULL DEFAULT 0,
    download   INTEGER NOT NULL DEFAULT 0,
    conn_count INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE traffic_daily (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    day        TEXT NOT NULL UNIQUE,
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

    async fn insert_sample(
        db: &DbPool,
        ts: &str,
        upload: i64,
        download: i64,
        conn_count: i64,
    ) -> Result<(), String> {
        let pool = sqlite_pool(db).map_err(|error| error.to_string())?;

        sqlx::query(
            r#"
INSERT INTO traffic_samples (ts, upload, download, conn_count)
VALUES (?, ?, ?, ?);
"#,
        )
        .bind(ts)
        .bind(upload)
        .bind(download)
        .bind(conn_count)
        .execute(pool)
        .await
        .map_err(|error| error.to_string())?;

        Ok(())
    }

    #[tokio::test]
    async fn aggregate_samples_in_tx_processes_backfilled_history_immediately() {
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

        let transaction = pool.begin().await;
        assert!(transaction.is_ok());
        let Ok(mut transaction) = transaction else {
            panic!("transaction should start");
        };

        let samples = vec![
            TrafficSampleInsert {
                ts: "2026-03-20T08:00:00Z".into(),
                upload: 11,
                download: 22,
                conn_count: 1,
            },
            TrafficSampleInsert {
                ts: "2026-03-20T09:00:00Z".into(),
                upload: 7,
                download: 9,
                conn_count: 0,
            },
        ];

        let insert_result = batch_insert_traffic_samples_in_tx(&mut transaction, &samples).await;
        assert!(insert_result.is_ok());
        let aggregate_result = aggregate_samples_in_tx(&mut transaction, &samples).await;
        assert!(aggregate_result.is_ok());
        let commit_result = transaction.commit().await;
        assert!(commit_result.is_ok());

        let hourly_rows = sqlx::query(
            r#"
SELECT hour, upload, download, conn_count
FROM traffic_hourly
ORDER BY hour ASC;
"#,
        )
        .fetch_all(pool)
        .await;
        assert!(hourly_rows.is_ok());
        let Ok(hourly_rows) = hourly_rows else {
            panic!("hourly rows should be queryable");
        };

        assert_eq!(hourly_rows.len(), 2);
        assert_eq!(
            hourly_rows[0].get::<String, _>("hour"),
            "2026-03-20T08:00:00Z"
        );
        assert_eq!(
            hourly_rows[1].get::<String, _>("hour"),
            "2026-03-20T09:00:00Z"
        );

        let daily_rows = sqlx::query(
            r#"
SELECT day, upload, download, conn_count
FROM traffic_daily
ORDER BY day ASC;
"#,
        )
        .fetch_all(pool)
        .await;
        assert!(daily_rows.is_ok());
        let Ok(daily_rows) = daily_rows else {
            panic!("daily rows should be queryable");
        };

        assert_eq!(daily_rows.len(), 1);
        assert_eq!(daily_rows[0].get::<String, _>("day"), "2026-03-20");
        assert_eq!(daily_rows[0].get::<i64, _>("upload"), 18);
        assert_eq!(daily_rows[0].get::<i64, _>("download"), 31);
        assert_eq!(daily_rows[0].get::<i64, _>("conn_count"), 1);

        let remaining_samples =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM traffic_samples;")
                .fetch_one(pool)
                .await;
        assert!(remaining_samples.is_ok());
        let Ok(remaining_samples) = remaining_samples else {
            panic!("remaining sample count should be queryable");
        };

        assert_eq!(remaining_samples, 2);
    }

    #[tokio::test]
    async fn aggregate_samples_in_tx_retains_history_for_future_reaggregation() {
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

        let transaction = pool.begin().await;
        assert!(transaction.is_ok());
        let Ok(mut transaction) = transaction else {
            panic!("transaction should start");
        };

        let samples = vec![
            TrafficSampleInsert {
                ts: "2026-03-20T08:15:00Z".into(),
                upload: 5,
                download: 5,
                conn_count: 1,
            },
            TrafficSampleInsert {
                ts: "2026-03-20T09:10:00Z".into(),
                upload: 9,
                download: 11,
                conn_count: 0,
            },
        ];

        let insert_result = batch_insert_traffic_samples_in_tx(&mut transaction, &samples).await;
        assert!(insert_result.is_ok());
        let aggregate_result = aggregate_samples_in_tx(&mut transaction, &samples).await;
        assert!(aggregate_result.is_ok());
        let commit_result = transaction.commit().await;
        assert!(commit_result.is_ok());

        let rows = sqlx::query("SELECT ts FROM traffic_samples ORDER BY ts ASC;")
            .fetch_all(pool)
            .await;
        assert!(rows.is_ok());
        let Ok(rows) = rows else {
            panic!("remaining raw samples should be queryable");
        };

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].get::<String, _>("ts"), "2026-03-20T08:15:00Z");
        assert_eq!(rows[1].get::<String, _>("ts"), "2026-03-20T09:10:00Z");
    }

    async fn upsert_hourly_row(
        db: &DbPool,
        hour: &str,
        upload: i64,
        download: i64,
        conn_count: i64,
    ) -> Result<(), String> {
        let pool = sqlite_pool(db).map_err(|error| error.to_string())?;

        sqlx::query(
            r#"
INSERT INTO traffic_hourly (hour, upload, download, conn_count)
VALUES (?, ?, ?, ?)
ON CONFLICT(hour) DO UPDATE SET
    upload = excluded.upload,
    download = excluded.download,
    conn_count = excluded.conn_count;
"#,
        )
        .bind(hour)
        .bind(upload)
        .bind(download)
        .bind(conn_count)
        .execute(pool)
        .await
        .map_err(|error| error.to_string())?;

        Ok(())
    }

    #[tokio::test]
    async fn aggregate_hourly_upserts_bucket_totals_from_samples() {
        let db = prepare_db().await;
        assert!(db.is_ok());
        let Ok(db) = db else {
            panic!("test database should be created");
        };

        assert!(insert_sample(&db, "2026-03-31T08:05:00Z", 10, 20, 1)
            .await
            .is_ok());
        assert!(insert_sample(&db, "2026-03-31T08:45:00Z", 5, 7, 1)
            .await
            .is_ok());
        assert!(insert_sample(&db, "2026-03-31T09:10:00Z", 3, 4, 0)
            .await
            .is_ok());

        let first_run = aggregate_hourly(&db, "2026-03-31T08:00:00Z", "2026-03-31T10:00:00Z").await;
        assert!(first_run.is_ok());

        assert!(insert_sample(&db, "2026-03-31T09:20:00Z", 7, 8, 0)
            .await
            .is_ok());

        let second_run =
            aggregate_hourly(&db, "2026-03-31T08:00:00Z", "2026-03-31T10:00:00Z").await;
        assert!(second_run.is_ok());

        let pool = sqlite_pool(&db);
        assert!(pool.is_ok());
        let Ok(pool) = pool else {
            panic!("sqlite pool should be available");
        };

        let rows = sqlx::query(
            r#"
SELECT hour, upload, download, conn_count
FROM traffic_hourly
ORDER BY hour ASC;
"#,
        )
        .fetch_all(pool)
        .await;
        assert!(rows.is_ok());
        let Ok(rows) = rows else {
            panic!("hourly aggregation rows should be queryable");
        };

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].get::<String, _>("hour"), "2026-03-31T08:00:00Z");
        assert_eq!(rows[0].get::<i64, _>("upload"), 15);
        assert_eq!(rows[0].get::<i64, _>("download"), 27);
        assert_eq!(rows[0].get::<i64, _>("conn_count"), 2);
        assert_eq!(rows[1].get::<String, _>("hour"), "2026-03-31T09:00:00Z");
        assert_eq!(rows[1].get::<i64, _>("upload"), 10);
        assert_eq!(rows[1].get::<i64, _>("download"), 12);
        assert_eq!(rows[1].get::<i64, _>("conn_count"), 0);
    }

    #[tokio::test]
    async fn aggregate_hourly_tracks_long_lived_connections_across_hours() {
        let db = prepare_db().await;
        assert!(db.is_ok());
        let Ok(db) = db else {
            panic!("test database should be created");
        };

        assert!(insert_sample(&db, "2026-03-31T08:55:00Z", 10, 20, 1)
            .await
            .is_ok());
        assert!(insert_sample(&db, "2026-03-31T09:05:00Z", 15, 25, 0)
            .await
            .is_ok());
        assert!(insert_sample(&db, "2026-03-31T09:45:00Z", 5, 10, 0)
            .await
            .is_ok());

        let result = aggregate_hourly(&db, "2026-03-31T08:00:00Z", "2026-03-31T10:00:00Z").await;
        assert!(result.is_ok());

        let pool = sqlite_pool(&db);
        assert!(pool.is_ok());
        let Ok(pool) = pool else {
            panic!("sqlite pool should be available");
        };

        let rows = sqlx::query(
            r#"
SELECT hour, upload, download, conn_count
FROM traffic_hourly
ORDER BY hour ASC;
"#,
        )
        .fetch_all(pool)
        .await;
        assert!(rows.is_ok());
        let Ok(rows) = rows else {
            panic!("hourly aggregation rows should be queryable");
        };

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].get::<String, _>("hour"), "2026-03-31T08:00:00Z");
        assert_eq!(rows[0].get::<i64, _>("upload"), 10);
        assert_eq!(rows[0].get::<i64, _>("download"), 20);
        assert_eq!(rows[0].get::<i64, _>("conn_count"), 1);
        assert_eq!(rows[1].get::<String, _>("hour"), "2026-03-31T09:00:00Z");
        assert_eq!(rows[1].get::<i64, _>("upload"), 20);
        assert_eq!(rows[1].get::<i64, _>("download"), 35);
        assert_eq!(rows[1].get::<i64, _>("conn_count"), 0);
    }

    #[tokio::test]
    async fn aggregate_daily_upserts_day_totals() {
        let db = prepare_db().await;
        assert!(db.is_ok());
        let Ok(db) = db else {
            panic!("test database should be created");
        };

        assert!(upsert_hourly_row(&db, "2026-03-31T08:00:00Z", 10, 20, 1)
            .await
            .is_ok());
        assert!(upsert_hourly_row(&db, "2026-03-31T09:00:00Z", 5, 7, 2)
            .await
            .is_ok());
        assert!(upsert_hourly_row(&db, "2026-04-01T01:00:00Z", 99, 88, 9)
            .await
            .is_ok());

        let first_run = aggregate_daily(&db, "2026-03-31T00:00:00Z", "2026-04-01T00:00:00Z").await;
        assert!(first_run.is_ok());

        assert!(upsert_hourly_row(&db, "2026-03-31T09:00:00Z", 12, 14, 3)
            .await
            .is_ok());

        let second_run = aggregate_daily(&db, "2026-03-31T00:00:00Z", "2026-04-01T00:00:00Z").await;
        assert!(second_run.is_ok());

        let pool = sqlite_pool(&db);
        assert!(pool.is_ok());
        let Ok(pool) = pool else {
            panic!("sqlite pool should be available");
        };

        let rows = sqlx::query(
            r#"
SELECT day, upload, download, conn_count
FROM traffic_daily
ORDER BY day ASC;
"#,
        )
        .fetch_all(pool)
        .await;
        assert!(rows.is_ok());
        let Ok(rows) = rows else {
            panic!("daily aggregation rows should be queryable");
        };

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].get::<String, _>("day"), "2026-03-31");
        assert_eq!(rows[0].get::<i64, _>("upload"), 22);
        assert_eq!(rows[0].get::<i64, _>("download"), 34);
        assert_eq!(rows[0].get::<i64, _>("conn_count"), 4);
    }

    #[tokio::test]
    async fn aggregate_rejects_invalid_windows() {
        let db = prepare_db().await;
        assert!(db.is_ok());
        let Ok(db) = db else {
            panic!("test database should be created");
        };

        let result = aggregate_hourly(&db, "2026-03-31T10:00:00Z", "2026-03-31T09:00:00Z").await;

        assert!(matches!(result, Err(DbError::InvalidTimeWindow(_))));
    }

    #[tokio::test]
    async fn aggregate_samples_in_tx_keeps_prior_hour_samples_for_late_backfills() {
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

        let transaction = pool.begin().await;
        assert!(transaction.is_ok());
        let Ok(mut transaction) = transaction else {
            panic!("transaction should start");
        };

        let initial_samples = vec![
            TrafficSampleInsert {
                ts: "2026-03-20T08:10:00Z".into(),
                upload: 10,
                download: 20,
                conn_count: 1,
            },
            TrafficSampleInsert {
                ts: "2026-03-20T08:20:00Z".into(),
                upload: 5,
                download: 7,
                conn_count: 0,
            },
        ];

        assert!(
            batch_insert_traffic_samples_in_tx(&mut transaction, &initial_samples)
                .await
                .is_ok()
        );
        assert!(aggregate_samples_in_tx(&mut transaction, &initial_samples)
            .await
            .is_ok());
        assert!(transaction.commit().await.is_ok());

        let late_backfill = vec![TrafficSampleInsert {
            ts: "2026-03-20T08:40:00Z".into(),
            upload: 3,
            download: 4,
            conn_count: 0,
        }];
        let transaction = pool.begin().await;
        assert!(transaction.is_ok());
        let Ok(mut transaction) = transaction else {
            panic!("transaction should start");
        };

        assert!(
            batch_insert_traffic_samples_in_tx(&mut transaction, &late_backfill)
                .await
                .is_ok()
        );
        assert!(aggregate_samples_in_tx(&mut transaction, &late_backfill)
            .await
            .is_ok());
        assert!(transaction.commit().await.is_ok());

        let row = sqlx::query(
            r#"
SELECT upload, download, conn_count
FROM traffic_hourly
WHERE hour = '2026-03-20T08:00:00Z';
"#,
        )
        .fetch_one(pool)
        .await;
        assert!(row.is_ok());
        let Ok(row) = row else {
            panic!("late-backfilled hourly row should be queryable");
        };

        assert_eq!(row.get::<i64, _>("upload"), 18);
        assert_eq!(row.get::<i64, _>("download"), 31);
        assert_eq!(row.get::<i64, _>("conn_count"), 1);
    }

    #[tokio::test]
    async fn query_hourly_and_daily_return_sorted_points() {
        let db = prepare_db().await;
        assert!(db.is_ok());
        let Ok(db) = db else {
            panic!("test database should be created");
        };

        assert!(upsert_hourly_row(&db, "2026-03-31T09:00:00Z", 5, 6, 2)
            .await
            .is_ok());
        assert!(upsert_hourly_row(&db, "2026-03-31T08:00:00Z", 1, 2, 1)
            .await
            .is_ok());

        let pool = sqlite_pool(&db);
        assert!(pool.is_ok());
        let Ok(pool) = pool else {
            panic!("sqlite pool should be available");
        };

        let insert_daily = sqlx::query(
            r#"
INSERT INTO traffic_daily (day, upload, download, conn_count)
VALUES ('2026-03-31', 10, 20, 3), ('2026-04-01', 30, 40, 5);
"#,
        )
        .execute(pool)
        .await;
        assert!(insert_daily.is_ok());

        let hourly = query_hourly(&db, "2026-03-31T08:00:00Z", "2026-03-31T10:00:00Z").await;
        assert!(hourly.is_ok());
        let Ok(hourly) = hourly else {
            panic!("hourly points should be queryable");
        };

        assert_eq!(hourly.len(), 2);
        assert_eq!(hourly[0].time, "2026-03-31T08:00:00Z");
        assert_eq!(hourly[1].time, "2026-03-31T09:00:00Z");

        let daily = query_daily(&db, "2026-03-31T00:00:00Z", "2026-04-02T00:00:00Z").await;
        assert!(daily.is_ok());
        let Ok(daily) = daily else {
            panic!("daily points should be queryable");
        };

        assert_eq!(daily.len(), 2);
        assert_eq!(daily[0].time, "2026-03-31T00:00:00Z");
        assert_eq!(daily[1].time, "2026-04-01T00:00:00Z");
    }
}
