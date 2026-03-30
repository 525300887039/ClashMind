use std::collections::HashMap;

use sqlx::{Row, Sqlite, Transaction};
use tauri_plugin_sql::DbPool;

use crate::collector::ws_client::ConnectionRecord;

use super::{
    repo_domain::{self, DomainStatsUpdate},
    sqlite_pool, DbError,
};

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
    start_time
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
        };
        records.insert(record.id.clone(), record);
    }

    Ok(records)
}

pub(crate) async fn persist_connection_batch(
    db: &DbPool,
    records: &[ConnectionRecord],
    domain_updates: &[DomainStatsUpdate],
) -> Result<(), DbError> {
    if records.is_empty() && domain_updates.is_empty() {
        return Ok(());
    }

    let pool = sqlite_pool(db)?;
    let mut transaction = pool
        .begin()
        .await
        .map_err(|error| DbError::TransactionFailed(error.to_string()))?;

    batch_insert_connections_in_tx(&mut transaction, records).await?;
    repo_domain::batch_upsert_domain_stats_in_tx(&mut transaction, domain_updates).await?;
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
    start_time
)
VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?);
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
