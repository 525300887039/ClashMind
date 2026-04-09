use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sqlx::{Row, Sqlite, Transaction};
use tauri_plugin_sql::DbPool;

use super::{retention_modifier, sqlite_pool, try_col, validate_positive_i32, DbError};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NodeHealthSnapshot {
    pub id: i64,
    pub node_name: String,
    pub delay_ms: Option<i32>,
    pub success: bool,
    pub tested_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NodeHealthInsert {
    pub node_name: String,
    pub delay_ms: Option<i32>,
    pub success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NodeHealthAggregate {
    pub node_name: String,
    pub total_tests: i64,
    pub success_count: i64,
    pub avg_delay_ms: Option<f64>,
    pub min_delay_ms: Option<i32>,
    pub max_delay_ms: Option<i32>,
    pub p95_delay_ms: Option<i32>,
}

#[allow(dead_code)]
pub async fn insert_health_snapshot(
    db: &DbPool,
    snapshot: &NodeHealthInsert,
) -> Result<i64, DbError> {
    let pool = sqlite_pool(db)?;
    let prepared = PreparedNodeHealthInsert::try_from(snapshot)?;
    let result = sqlx::query(
        r#"
INSERT INTO node_health_snapshots (node_name, delay_ms, success)
VALUES (?, ?, ?);
"#,
    )
    .bind(&prepared.node_name)
    .bind(prepared.delay_ms)
    .bind(prepared.success)
    .execute(pool)
    .await
    .map_err(|error| DbError::WriteFailed(format!("写入节点健康快照失败: {error}")))?;

    Ok(result.last_insert_rowid())
}

#[allow(dead_code)]
pub async fn insert_health_snapshots_batch(
    db: &DbPool,
    snapshots: &[NodeHealthInsert],
) -> Result<usize, DbError> {
    if snapshots.is_empty() {
        return Ok(0);
    }

    let pool = sqlite_pool(db)?;
    let mut transaction = pool
        .begin()
        .await
        .map_err(|error| DbError::TransactionFailed(error.to_string()))?;

    for snapshot in snapshots {
        insert_health_snapshot_in_tx(&mut transaction, snapshot).await?;
    }

    transaction
        .commit()
        .await
        .map_err(|error| DbError::TransactionFailed(error.to_string()))?;

    Ok(snapshots.len())
}

#[allow(dead_code)]
pub async fn get_health_snapshot_by_id(
    db: &DbPool,
    id: i64,
) -> Result<Option<NodeHealthSnapshot>, DbError> {
    let pool = sqlite_pool(db)?;
    let row = sqlx::query(
        r#"
SELECT id, node_name, delay_ms, success, tested_at
FROM node_health_snapshots
WHERE id = ?;
"#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|error| DbError::QueryFailed(format!("查询节点健康快照失败: {error}")))?;

    row.map(map_snapshot).transpose()
}

#[allow(dead_code)]
pub async fn list_health_snapshots_for_node(
    db: &DbPool,
    node_name: &str,
    hours: i32,
    limit: i32,
) -> Result<Vec<NodeHealthSnapshot>, DbError> {
    if limit <= 0 {
        return Ok(Vec::new());
    }

    let normalized_node_name = normalize_node_name(node_name)?;
    let hours = validate_hours(hours)?;
    let pool = sqlite_pool(db)?;
    let rows = sqlx::query(
        r#"
SELECT id, node_name, delay_ms, success, tested_at
FROM node_health_snapshots
WHERE TRIM(node_name) = ?
  AND datetime(tested_at) >= datetime('now', '-' || ? || ' hours')
ORDER BY datetime(tested_at) DESC, id DESC
LIMIT ?;
"#,
    )
    .bind(&normalized_node_name)
    .bind(hours)
    .bind(i64::from(limit))
    .fetch_all(pool)
    .await
    .map_err(|error| DbError::QueryFailed(format!("查询节点健康快照列表失败: {error}")))?;

    rows.into_iter().map(map_snapshot).collect()
}

#[allow(dead_code)]
pub async fn update_health_snapshot(
    db: &DbPool,
    id: i64,
    snapshot: &NodeHealthInsert,
) -> Result<usize, DbError> {
    let pool = sqlite_pool(db)?;
    let prepared = PreparedNodeHealthInsert::try_from(snapshot)?;
    let result = sqlx::query(
        r#"
UPDATE node_health_snapshots
SET node_name = ?, delay_ms = ?, success = ?, tested_at = datetime('now')
WHERE id = ?;
"#,
    )
    .bind(&prepared.node_name)
    .bind(prepared.delay_ms)
    .bind(prepared.success)
    .bind(id)
    .execute(pool)
    .await
    .map_err(|error| DbError::WriteFailed(format!("更新节点健康快照失败: {error}")))?;

    usize::try_from(result.rows_affected())
        .map_err(|error| DbError::WriteFailed(format!("更新节点健康快照结果溢出: {error}")))
}

#[allow(dead_code)]
pub async fn delete_health_snapshot(db: &DbPool, id: i64) -> Result<usize, DbError> {
    let pool = sqlite_pool(db)?;
    let result = sqlx::query(
        r#"
DELETE FROM node_health_snapshots
WHERE id = ?;
"#,
    )
    .bind(id)
    .execute(pool)
    .await
    .map_err(|error| DbError::WriteFailed(format!("删除节点健康快照失败: {error}")))?;

    usize::try_from(result.rows_affected())
        .map_err(|error| DbError::WriteFailed(format!("删除节点健康快照结果溢出: {error}")))
}

#[allow(dead_code)]
pub async fn get_node_health_aggregate(
    db: &DbPool,
    node_name: &str,
    hours: i32,
) -> Result<NodeHealthAggregate, DbError> {
    let normalized_node_name = normalize_node_name(node_name)?;
    let hours = validate_hours(hours)?;
    let pool = sqlite_pool(db)?;
    let row = sqlx::query(
        r#"
SELECT
    COUNT(*) AS total_tests,
    COALESCE(
        SUM(
            CASE
                WHEN success = 1 THEN 1
                ELSE 0
            END
        ),
        0
    ) AS success_count,
    AVG(
        CASE
            WHEN success = 1 THEN delay_ms
            ELSE NULL
        END
    ) AS avg_delay_ms,
    MIN(
        CASE
            WHEN success = 1 THEN delay_ms
            ELSE NULL
        END
    ) AS min_delay_ms,
    MAX(
        CASE
            WHEN success = 1 THEN delay_ms
            ELSE NULL
        END
    ) AS max_delay_ms
FROM node_health_snapshots
WHERE TRIM(node_name) = ?
  AND datetime(tested_at) >= datetime('now', '-' || ? || ' hours');
"#,
    )
    .bind(&normalized_node_name)
    .bind(hours)
    .fetch_one(pool)
    .await
    .map_err(|error| DbError::QueryFailed(format!("查询节点健康聚合失败: {error}")))?;

    let delays = query_delay_samples_for_node(db, &normalized_node_name, hours).await?;

    Ok(NodeHealthAggregate {
        node_name: normalized_node_name,
        total_tests: try_col!(row, "total_tests", "节点健康聚合"),
        success_count: try_col!(row, "success_count", "节点健康聚合"),
        avg_delay_ms: try_col!(row, "avg_delay_ms", "节点健康聚合"),
        min_delay_ms: optional_i32_from_row(&row, "min_delay_ms", "节点健康聚合")?,
        max_delay_ms: optional_i32_from_row(&row, "max_delay_ms", "节点健康聚合")?,
        p95_delay_ms: compute_p95_delay(&delays),
    })
}

pub async fn get_all_nodes_health(
    db: &DbPool,
    hours: i32,
) -> Result<Vec<NodeHealthAggregate>, DbError> {
    let hours = validate_hours(hours)?;
    let pool = sqlite_pool(db)?;
    let rows = sqlx::query(
        r#"
SELECT
    TRIM(node_name) AS node_name,
    COUNT(*) AS total_tests,
    COALESCE(
        SUM(
            CASE
                WHEN success = 1 THEN 1
                ELSE 0
            END
        ),
        0
    ) AS success_count,
    AVG(
        CASE
            WHEN success = 1 THEN delay_ms
            ELSE NULL
        END
    ) AS avg_delay_ms,
    MIN(
        CASE
            WHEN success = 1 THEN delay_ms
            ELSE NULL
        END
    ) AS min_delay_ms,
    MAX(
        CASE
            WHEN success = 1 THEN delay_ms
            ELSE NULL
        END
    ) AS max_delay_ms
FROM node_health_snapshots
WHERE TRIM(node_name) <> ''
  AND datetime(tested_at) >= datetime('now', '-' || ? || ' hours')
GROUP BY TRIM(node_name)
ORDER BY node_name ASC;
"#,
    )
    .bind(hours)
    .fetch_all(pool)
    .await
    .map_err(|error| DbError::QueryFailed(format!("查询全部节点健康聚合失败: {error}")))?;

    let delay_samples = query_delay_samples_for_all_nodes(db, hours).await?;
    let mut aggregates = Vec::with_capacity(rows.len());

    for row in rows {
        let node_name: String = try_col!(row, "node_name", "节点健康聚合");
        let p95_delay_ms = delay_samples
            .get(&node_name)
            .and_then(|delays| compute_p95_delay(delays));

        aggregates.push(NodeHealthAggregate {
            node_name,
            total_tests: try_col!(row, "total_tests", "节点健康聚合"),
            success_count: try_col!(row, "success_count", "节点健康聚合"),
            avg_delay_ms: try_col!(row, "avg_delay_ms", "节点健康聚合"),
            min_delay_ms: optional_i32_from_row(&row, "min_delay_ms", "节点健康聚合")?,
            max_delay_ms: optional_i32_from_row(&row, "max_delay_ms", "节点健康聚合")?,
            p95_delay_ms,
        });
    }

    Ok(aggregates)
}

pub async fn cleanup_old_snapshots(db: &DbPool, retain_days: i32) -> Result<usize, DbError> {
    let modifier = retention_modifier(retain_days)?;
    let pool = sqlite_pool(db)?;
    let result = sqlx::query(
        r#"
DELETE FROM node_health_snapshots
WHERE datetime(tested_at) < datetime('now', ?);
"#,
    )
    .bind(modifier)
    .execute(pool)
    .await
    .map_err(|error| DbError::WriteFailed(format!("清理节点健康快照失败: {error}")))?;

    usize::try_from(result.rows_affected())
        .map_err(|error| DbError::WriteFailed(format!("清理节点健康快照结果溢出: {error}")))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PreparedNodeHealthInsert {
    node_name: String,
    delay_ms: Option<i32>,
    success: i64,
}

impl TryFrom<&NodeHealthInsert> for PreparedNodeHealthInsert {
    type Error = DbError;

    fn try_from(value: &NodeHealthInsert) -> Result<Self, Self::Error> {
        let node_name = normalize_node_name(&value.node_name)?;
        let delay_ms = normalize_delay_ms(value.delay_ms)?;

        Ok(Self {
            node_name,
            delay_ms: if value.success { delay_ms } else { None },
            success: if value.success { 1 } else { 0 },
        })
    }
}

async fn insert_health_snapshot_in_tx(
    transaction: &mut Transaction<'_, Sqlite>,
    snapshot: &NodeHealthInsert,
) -> Result<(), DbError> {
    let prepared = PreparedNodeHealthInsert::try_from(snapshot)?;

    sqlx::query(
        r#"
INSERT INTO node_health_snapshots (node_name, delay_ms, success)
VALUES (?, ?, ?);
"#,
    )
    .bind(&prepared.node_name)
    .bind(prepared.delay_ms)
    .bind(prepared.success)
    .execute(transaction.as_mut())
    .await
    .map_err(|error| DbError::WriteFailed(format!("批量写入节点健康快照失败: {error}")))?;

    Ok(())
}

fn normalize_node_name(node_name: &str) -> Result<String, DbError> {
    let normalized = node_name.trim();
    if normalized.is_empty() {
        return Err(DbError::WriteFailed("node_name 不能为空".to_string()));
    }

    Ok(normalized.to_string())
}

fn normalize_delay_ms(delay_ms: Option<i32>) -> Result<Option<i32>, DbError> {
    match delay_ms {
        Some(value) if value < 0 => Err(DbError::WriteFailed("delay_ms 不能为负数".to_string())),
        other_value => Ok(other_value),
    }
}

fn validate_hours(hours: i32) -> Result<i64, DbError> {
    validate_positive_i32(hours, "hours")
}

fn optional_i32_from_row(
    row: &sqlx::sqlite::SqliteRow,
    column: &str,
    context: &str,
) -> Result<Option<i32>, DbError> {
    let value = row
        .try_get::<Option<i64>, _>(column)
        .map_err(|error| DbError::QueryFailed(format!("读取{context} {column} 失败: {error}")))?;

    value
        .map(|value| {
            i32::try_from(value).map_err(|error| {
                DbError::QueryFailed(format!("转换{context} {column} 失败: {error}"))
            })
        })
        .transpose()
}

fn map_snapshot(row: sqlx::sqlite::SqliteRow) -> Result<NodeHealthSnapshot, DbError> {
    let success: i64 = try_col!(row, "success", "节点健康快照");

    Ok(NodeHealthSnapshot {
        id: try_col!(row, "id", "节点健康快照"),
        node_name: try_col!(row, "node_name", "节点健康快照"),
        delay_ms: optional_i32_from_row(&row, "delay_ms", "节点健康快照")?,
        success: success != 0,
        tested_at: try_col!(row, "tested_at", "节点健康快照"),
    })
}

#[allow(dead_code)]
async fn query_delay_samples_for_node(
    db: &DbPool,
    node_name: &str,
    hours: i64,
) -> Result<Vec<i32>, DbError> {
    let pool = sqlite_pool(db)?;
    let rows = sqlx::query(
        r#"
SELECT delay_ms
FROM node_health_snapshots
WHERE TRIM(node_name) = ?
  AND success = 1
  AND delay_ms IS NOT NULL
  AND datetime(tested_at) >= datetime('now', '-' || ? || ' hours')
ORDER BY delay_ms ASC;
"#,
    )
    .bind(node_name)
    .bind(hours)
    .fetch_all(pool)
    .await
    .map_err(|error| DbError::QueryFailed(format!("查询节点健康延迟样本失败: {error}")))?;

    rows.into_iter()
        .map(|row| {
            row.try_get::<i64, _>("delay_ms")
                .map_err(|error| DbError::QueryFailed(format!("读取节点健康延迟样本失败: {error}")))
                .and_then(|value| {
                    i32::try_from(value).map_err(|error| {
                        DbError::QueryFailed(format!("转换节点健康延迟样本失败: {error}"))
                    })
                })
        })
        .collect()
}

async fn query_delay_samples_for_all_nodes(
    db: &DbPool,
    hours: i64,
) -> Result<BTreeMap<String, Vec<i32>>, DbError> {
    let pool = sqlite_pool(db)?;
    let rows = sqlx::query(
        r#"
SELECT TRIM(node_name) AS node_name, delay_ms
FROM node_health_snapshots
WHERE TRIM(node_name) <> ''
  AND success = 1
  AND delay_ms IS NOT NULL
  AND datetime(tested_at) >= datetime('now', '-' || ? || ' hours')
ORDER BY node_name ASC, delay_ms ASC;
"#,
    )
    .bind(hours)
    .fetch_all(pool)
    .await
    .map_err(|error| DbError::QueryFailed(format!("查询全部节点健康延迟样本失败: {error}")))?;

    let mut grouped = BTreeMap::new();
    for row in rows {
        let node_name: String = try_col!(row, "node_name", "节点健康延迟样本");
        let delay_ms = row
            .try_get::<i64, _>("delay_ms")
            .map_err(|error| {
                DbError::QueryFailed(format!("读取节点健康延迟样本 delay_ms 失败: {error}"))
            })
            .and_then(|value| {
                i32::try_from(value).map_err(|error| {
                    DbError::QueryFailed(format!("转换节点健康延迟样本 delay_ms 失败: {error}"))
                })
            })?;

        grouped
            .entry(node_name)
            .or_insert_with(Vec::new)
            .push(delay_ms);
    }

    Ok(grouped)
}

fn compute_p95_delay(delays: &[i32]) -> Option<i32> {
    if delays.is_empty() {
        return None;
    }

    let rank = (delays.len() * 95).div_ceil(100);
    let index = rank.saturating_sub(1);
    delays.get(index).copied()
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
CREATE TABLE node_health_snapshots (
    id        INTEGER PRIMARY KEY AUTOINCREMENT,
    node_name TEXT NOT NULL,
    delay_ms  INTEGER,
    success   INTEGER NOT NULL,
    tested_at TEXT NOT NULL DEFAULT (datetime('now'))
);
"#,
        )
        .execute(&pool)
        .await
        .map_err(|error| error.to_string())?;

        Ok(DbPool::Sqlite(pool))
    }

    #[tokio::test]
    async fn insert_update_list_and_delete_snapshot_work() {
        let db = prepare_db().await.expect("test database should be created");

        let snapshot_id = insert_health_snapshot(
            &db,
            &NodeHealthInsert {
                node_name: " Proxy-A ".to_string(),
                delay_ms: Some(120),
                success: true,
            },
        )
        .await
        .expect("snapshot should be inserted");

        let snapshot = get_health_snapshot_by_id(&db, snapshot_id)
            .await
            .expect("snapshot should be queryable")
            .expect("snapshot should exist");
        assert_eq!(snapshot.node_name, "Proxy-A");
        assert_eq!(snapshot.delay_ms, Some(120));
        assert!(snapshot.success);

        let updated = update_health_snapshot(
            &db,
            snapshot_id,
            &NodeHealthInsert {
                node_name: "Proxy-B".to_string(),
                delay_ms: Some(250),
                success: false,
            },
        )
        .await
        .expect("snapshot should be updated");
        assert_eq!(updated, 1);

        let updated_snapshot = get_health_snapshot_by_id(&db, snapshot_id)
            .await
            .expect("updated snapshot should be queryable")
            .expect("updated snapshot should exist");
        assert_eq!(updated_snapshot.node_name, "Proxy-B");
        assert_eq!(updated_snapshot.delay_ms, None);
        assert!(!updated_snapshot.success);

        let snapshots = list_health_snapshots_for_node(&db, "Proxy-B", 24, 10)
            .await
            .expect("snapshot list should be queryable");
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].id, snapshot_id);

        let deleted = delete_health_snapshot(&db, snapshot_id)
            .await
            .expect("snapshot should be deleted");
        assert_eq!(deleted, 1);
        assert!(get_health_snapshot_by_id(&db, snapshot_id)
            .await
            .expect("deleted snapshot lookup should succeed")
            .is_none());
    }

    #[tokio::test]
    async fn aggregates_respect_window_and_compute_p95() {
        let db = prepare_db().await.expect("test database should be created");

        let mut batch = Vec::new();
        for delay_ms in (10..=200).step_by(10) {
            batch.push(NodeHealthInsert {
                node_name: "Proxy-A".to_string(),
                delay_ms: Some(delay_ms),
                success: true,
            });
        }
        for _ in 0..5 {
            batch.push(NodeHealthInsert {
                node_name: "Proxy-A".to_string(),
                delay_ms: Some(999),
                success: false,
            });
        }
        batch.push(NodeHealthInsert {
            node_name: "Proxy-B".to_string(),
            delay_ms: Some(80),
            success: true,
        });
        batch.push(NodeHealthInsert {
            node_name: "Proxy-B".to_string(),
            delay_ms: Some(100),
            success: true,
        });

        let inserted = insert_health_snapshots_batch(&db, &batch)
            .await
            .expect("snapshot batch should be inserted");
        assert_eq!(inserted, batch.len());

        let pool = sqlite_pool(&db).expect("sqlite pool should be available");
        sqlx::query(
            r#"
INSERT INTO node_health_snapshots (node_name, delay_ms, success, tested_at)
VALUES ('Proxy-A', 15, 1, datetime('now', '-30 hours'));
"#,
        )
        .execute(pool)
        .await
        .expect("expired snapshot should be inserted");

        let aggregate = get_node_health_aggregate(&db, "Proxy-A", 24)
            .await
            .expect("node aggregate should be queryable");
        assert_eq!(aggregate.node_name, "Proxy-A");
        assert_eq!(aggregate.total_tests, 25);
        assert_eq!(aggregate.success_count, 20);
        assert_eq!(aggregate.avg_delay_ms, Some(105.0));
        assert_eq!(aggregate.min_delay_ms, Some(10));
        assert_eq!(aggregate.max_delay_ms, Some(200));
        assert_eq!(aggregate.p95_delay_ms, Some(190));

        let all_aggregates = get_all_nodes_health(&db, 24)
            .await
            .expect("all aggregates should be queryable");
        assert_eq!(all_aggregates.len(), 2);
        assert_eq!(all_aggregates[0].node_name, "Proxy-A");
        assert_eq!(all_aggregates[1].node_name, "Proxy-B");
        assert_eq!(all_aggregates[1].p95_delay_ms, Some(100));
    }

    #[tokio::test]
    async fn empty_window_returns_zero_aggregate() {
        let db = prepare_db().await.expect("test database should be created");

        let aggregate = get_node_health_aggregate(&db, "Proxy-Z", 24)
            .await
            .expect("empty aggregate should succeed");
        assert_eq!(
            aggregate,
            NodeHealthAggregate {
                node_name: "Proxy-Z".to_string(),
                total_tests: 0,
                success_count: 0,
                avg_delay_ms: None,
                min_delay_ms: None,
                max_delay_ms: None,
                p95_delay_ms: None,
            }
        );
    }

    #[tokio::test]
    async fn cleanup_and_validation_work() {
        let db = prepare_db().await.expect("test database should be created");

        let invalid_insert = insert_health_snapshot(
            &db,
            &NodeHealthInsert {
                node_name: "   ".to_string(),
                delay_ms: Some(10),
                success: true,
            },
        )
        .await;
        assert!(matches!(invalid_insert, Err(DbError::WriteFailed(_))));

        let pool = sqlite_pool(&db).expect("sqlite pool should be available");
        sqlx::query(
            r#"
INSERT INTO node_health_snapshots (node_name, delay_ms, success, tested_at) VALUES
    ('Proxy-A', 100, 1, datetime('now', '-40 days')),
    ('Proxy-A', 110, 1, datetime('now', '-2 days'));
"#,
        )
        .execute(pool)
        .await
        .expect("seed snapshots should be inserted");

        let deleted = cleanup_old_snapshots(&db, 30)
            .await
            .expect("cleanup should succeed");
        assert_eq!(deleted, 1);

        let remaining = list_health_snapshots_for_node(&db, "Proxy-A", 24 * 24, 10)
            .await
            .expect("remaining snapshots should be queryable");
        assert_eq!(remaining.len(), 1);

        let invalid_hours = get_all_nodes_health(&db, 0).await;
        assert!(matches!(invalid_hours, Err(DbError::InvalidTimeWindow(_))));
    }
}
