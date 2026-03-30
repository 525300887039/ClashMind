use sqlx::{Sqlite, Transaction};
use tauri_plugin_sql::DbPool;

use super::{sqlite_pool, DbError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainStatsUpdate {
    pub domain: String,
    pub day: String,
    pub hit_count: i64,
    pub upload: i64,
    pub download: i64,
}

#[allow(dead_code)]
pub async fn upsert_domain_stats(
    db: &DbPool,
    domain: &str,
    day: &str,
    hit_count: i64,
    upload: i64,
    download: i64,
) -> Result<(), DbError> {
    let pool = sqlite_pool(db)?;
    let mut transaction = pool
        .begin()
        .await
        .map_err(|error| DbError::TransactionFailed(error.to_string()))?;
    let update = DomainStatsUpdate {
        domain: domain.to_string(),
        day: day.to_string(),
        hit_count,
        upload,
        download,
    };

    batch_upsert_domain_stats_in_tx(&mut transaction, std::slice::from_ref(&update)).await?;
    transaction
        .commit()
        .await
        .map_err(|error| DbError::TransactionFailed(error.to_string()))?;

    Ok(())
}

pub(crate) async fn batch_upsert_domain_stats_in_tx(
    transaction: &mut Transaction<'_, Sqlite>,
    updates: &[DomainStatsUpdate],
) -> Result<(), DbError> {
    if updates.is_empty() {
        return Ok(());
    }

    for update in updates {
        sqlx::query(
            r#"
INSERT INTO domain_stats (domain, day, hit_count, upload, download)
VALUES (?, ?, ?, ?, ?)
ON CONFLICT(domain, day)
DO UPDATE SET
    hit_count = hit_count + excluded.hit_count,
    upload = upload + excluded.upload,
    download = download + excluded.download;
"#,
        )
        .bind(&update.domain)
        .bind(&update.day)
        .bind(update.hit_count)
        .bind(update.upload)
        .bind(update.download)
        .execute(transaction.as_mut())
        .await
        .map_err(|error| {
            DbError::WriteFailed(format!(
                "写入域名统计失败: domain={}, day={}, error={error}",
                update.domain, update.day
            ))
        })?;
    }

    Ok(())
}
