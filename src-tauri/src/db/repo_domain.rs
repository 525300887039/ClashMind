use sqlx::{Row, Sqlite, Transaction};
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TopDomainRow {
    pub domain: String,
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

pub(crate) async fn query_top_domains(
    db: &DbPool,
    days: i32,
    limit: i32,
) -> Result<Vec<TopDomainRow>, DbError> {
    if limit <= 0 {
        return Ok(Vec::new());
    }

    let pool = sqlite_pool(db)?;
    let rows = sqlx::query(
        r#"
SELECT
    domain,
    COALESCE(SUM(hit_count), 0) AS hit_count,
    COALESCE(SUM(upload), 0) AS upload,
    COALESCE(SUM(download), 0) AS download
FROM domain_stats
WHERE date(day) >= date('now', '-' || ? || ' days')
GROUP BY domain
ORDER BY
    (COALESCE(SUM(upload), 0) + COALESCE(SUM(download), 0)) DESC,
    COALESCE(SUM(hit_count), 0) DESC,
    domain ASC
LIMIT ?;
"#,
    )
    .bind(i64::from(days.max(0)))
    .bind(i64::from(limit.max(0)))
    .fetch_all(pool)
    .await
    .map_err(|error| DbError::QueryFailed(format!("查询域名统计失败: {error}")))?;

    let mut domains = Vec::with_capacity(rows.len());
    for row in rows {
        domains.push(TopDomainRow {
            domain: row.try_get("domain").map_err(|error| {
                DbError::QueryFailed(format!("读取域名统计 domain 失败: {error}"))
            })?,
            hit_count: row.try_get("hit_count").map_err(|error| {
                DbError::QueryFailed(format!("读取域名统计 hit_count 失败: {error}"))
            })?,
            upload: row.try_get("upload").map_err(|error| {
                DbError::QueryFailed(format!("读取域名统计 upload 失败: {error}"))
            })?,
            download: row.try_get("download").map_err(|error| {
                DbError::QueryFailed(format!("读取域名统计 download 失败: {error}"))
            })?,
        });
    }

    Ok(domains)
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
CREATE TABLE domain_stats (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    domain     TEXT NOT NULL,
    day        TEXT NOT NULL,
    hit_count  INTEGER NOT NULL DEFAULT 0,
    upload     INTEGER NOT NULL DEFAULT 0,
    download   INTEGER NOT NULL DEFAULT 0,
    UNIQUE(domain, day)
);
"#,
        )
        .execute(&pool)
        .await
        .map_err(|error| error.to_string())?;

        Ok(DbPool::Sqlite(pool))
    }

    #[tokio::test]
    async fn query_top_domains_returns_sorted_aggregates() {
        let db = prepare_db().await;
        assert!(db.is_ok());
        let Ok(db) = db else {
            panic!("test database should be created");
        };

        assert!(
            upsert_domain_stats(&db, "alpha.example", "2026-03-30", 2, 100, 200)
                .await
                .is_ok()
        );
        assert!(
            upsert_domain_stats(&db, "alpha.example", "2026-03-31", 1, 10, 20)
                .await
                .is_ok()
        );
        assert!(
            upsert_domain_stats(&db, "beta.example", "2026-03-31", 5, 50, 40)
                .await
                .is_ok()
        );

        let result = query_top_domains(&db, 3650, 10).await;
        assert!(result.is_ok());
        let Ok(result) = result else {
            panic!("top domains should be queryable");
        };

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].domain, "alpha.example");
        assert_eq!(result[0].hit_count, 3);
        assert_eq!(result[0].upload, 110);
        assert_eq!(result[0].download, 220);
        assert_eq!(result[1].domain, "beta.example");
    }
}
