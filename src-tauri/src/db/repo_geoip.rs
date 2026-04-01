use std::collections::HashMap;

use sqlx::{QueryBuilder, Row, Sqlite, Transaction};
use tauri_plugin_sql::DbPool;

use crate::utils::geoip::GeoLocation;

use super::{sqlite_pool, DbError};

const SQLITE_IP_CACHE_QUERY_CHUNK_SIZE: usize = 500;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IpTrafficStatsUpdate {
    pub dst_ip: String,
    pub day: String,
    pub upload: i64,
    pub download: i64,
}

pub async fn get_cached_geos(
    db: &DbPool,
    ips: &[String],
) -> Result<HashMap<String, GeoLocation>, DbError> {
    if ips.is_empty() {
        return Ok(HashMap::new());
    }

    let pool = sqlite_pool(db)?;
    let mut cached_geos = HashMap::with_capacity(ips.len());

    for chunk in ips.chunks(SQLITE_IP_CACHE_QUERY_CHUNK_SIZE) {
        let mut query_builder = QueryBuilder::<Sqlite>::new(
            "SELECT ip, country, country_code, city, latitude, longitude FROM geoip_cache WHERE ip IN (",
        );
        let mut separated = query_builder.separated(", ");
        for ip in chunk {
            separated.push_bind(ip);
        }
        separated.push_unseparated(")");

        let rows = query_builder
            .build()
            .fetch_all(pool)
            .await
            .map_err(|error| DbError::QueryFailed(format!("批量查询 GeoIP 缓存失败: {error}")))?;

        for row in rows {
            let geo = GeoLocation {
                ip: row.try_get("ip").map_err(|error| {
                    DbError::QueryFailed(format!("读取批量 GeoIP 缓存 ip 失败: {error}"))
                })?,
                country: row.try_get("country").map_err(|error| {
                    DbError::QueryFailed(format!("读取批量 GeoIP 缓存 country 失败: {error}"))
                })?,
                country_code: row.try_get("country_code").map_err(|error| {
                    DbError::QueryFailed(format!("读取批量 GeoIP 缓存 country_code 失败: {error}"))
                })?,
                city: row.try_get("city").map_err(|error| {
                    DbError::QueryFailed(format!("读取批量 GeoIP 缓存 city 失败: {error}"))
                })?,
                latitude: row.try_get("latitude").map_err(|error| {
                    DbError::QueryFailed(format!("读取批量 GeoIP 缓存 latitude 失败: {error}"))
                })?,
                longitude: row.try_get("longitude").map_err(|error| {
                    DbError::QueryFailed(format!("读取批量 GeoIP 缓存 longitude 失败: {error}"))
                })?,
            };
            cached_geos.insert(geo.ip.clone(), geo);
        }
    }

    Ok(cached_geos)
}

pub async fn batch_cache_geo(db: &DbPool, geos: &[GeoLocation]) -> Result<(), DbError> {
    if geos.is_empty() {
        return Ok(());
    }

    let pool = sqlite_pool(db)?;
    let mut transaction = pool
        .begin()
        .await
        .map_err(|error| DbError::TransactionFailed(error.to_string()))?;

    batch_cache_geo_in_tx(&mut transaction, geos).await?;
    transaction
        .commit()
        .await
        .map_err(|error| DbError::TransactionFailed(error.to_string()))?;

    Ok(())
}

async fn batch_cache_geo_in_tx(
    transaction: &mut Transaction<'_, Sqlite>,
    geos: &[GeoLocation],
) -> Result<(), DbError> {
    for geo in geos {
        sqlx::query(
            r#"
INSERT INTO geoip_cache (ip, country, country_code, city, latitude, longitude, updated_at)
VALUES (?, ?, ?, ?, ?, ?, datetime('now'))
ON CONFLICT(ip) DO UPDATE SET
    country = excluded.country,
    country_code = excluded.country_code,
    city = excluded.city,
    latitude = excluded.latitude,
    longitude = excluded.longitude,
    updated_at = datetime('now');
"#,
        )
        .bind(&geo.ip)
        .bind(geo.country.as_deref())
        .bind(geo.country_code.as_deref())
        .bind(geo.city.as_deref())
        .bind(geo.latitude)
        .bind(geo.longitude)
        .execute(transaction.as_mut())
        .await
        .map_err(|error| {
            DbError::WriteFailed(format!("写入 GeoIP 缓存失败: ip={}, error={error}", geo.ip))
        })?;
    }

    Ok(())
}

pub(crate) async fn batch_upsert_ip_traffic_stats_in_tx(
    transaction: &mut Transaction<'_, Sqlite>,
    updates: &[IpTrafficStatsUpdate],
) -> Result<(), DbError> {
    if updates.is_empty() {
        return Ok(());
    }

    for update in updates {
        sqlx::query(
            r#"
INSERT INTO ip_traffic_daily (dst_ip, day, upload, download)
VALUES (?, ?, ?, ?)
ON CONFLICT(dst_ip, day)
DO UPDATE SET
    upload = upload + excluded.upload,
    download = download + excluded.download;
"#,
        )
        .bind(&update.dst_ip)
        .bind(&update.day)
        .bind(update.upload)
        .bind(update.download)
        .execute(transaction.as_mut())
        .await
        .map_err(|error| {
            DbError::WriteFailed(format!(
                "写入 IP 日流量统计失败: dst_ip={}, day={}, error={error}",
                update.dst_ip, update.day
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
CREATE TABLE geoip_cache (
    ip           TEXT PRIMARY KEY,
    country      TEXT,
    country_code TEXT,
    city         TEXT,
    latitude     REAL,
    longitude    REAL,
    updated_at   TEXT NOT NULL DEFAULT (datetime('now'))
);
"#,
        )
        .execute(&pool)
        .await
        .map_err(|error| error.to_string())?;

        Ok(DbPool::Sqlite(pool))
    }

    #[tokio::test]
    async fn cache_geo_round_trips_location() {
        let db = prepare_db().await;
        assert!(db.is_ok());
        let Ok(db) = db else {
            panic!("test database should be created");
        };

        let geo = GeoLocation {
            ip: "1.1.1.1".into(),
            country: Some("Australia".into()),
            country_code: Some("AU".into()),
            city: Some("Sydney".into()),
            latitude: Some(-33.8688),
            longitude: Some(151.2093),
        };

        let cache_result = batch_cache_geo(&db, std::slice::from_ref(&geo)).await;
        assert!(cache_result.is_ok());

        let cached = get_cached_geos(&db, &["1.1.1.1".into()]).await;
        assert!(cached.is_ok());
        let Ok(cached) = cached else {
            panic!("GeoIP cache should be queryable");
        };

        assert_eq!(cached.get("1.1.1.1"), Some(&geo));
    }

    #[tokio::test]
    async fn get_cached_geos_returns_batch_results() {
        let db = prepare_db().await;
        assert!(db.is_ok());
        let Ok(db) = db else {
            panic!("test database should be created");
        };

        let geos = vec![
            GeoLocation {
                ip: "1.1.1.1".into(),
                country: Some("Australia".into()),
                country_code: Some("AU".into()),
                city: None,
                latitude: None,
                longitude: None,
            },
            GeoLocation {
                ip: "8.8.8.8".into(),
                country: Some("United States".into()),
                country_code: Some("US".into()),
                city: None,
                latitude: None,
                longitude: None,
            },
        ];

        let cache_result = batch_cache_geo(&db, &geos).await;
        assert!(cache_result.is_ok());

        let cached =
            get_cached_geos(&db, &["1.1.1.1".into(), "8.8.8.8".into(), "9.9.9.9".into()]).await;
        assert!(cached.is_ok());
        let Ok(cached) = cached else {
            panic!("GeoIP batch cache should be queryable");
        };

        assert_eq!(cached.len(), 2);
        assert_eq!(cached.get("1.1.1.1"), Some(&geos[0]));
        assert_eq!(cached.get("8.8.8.8"), Some(&geos[1]));
    }

    #[tokio::test]
    async fn batch_upsert_ip_traffic_stats_accumulates_daily_totals() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await;
        assert!(pool.is_ok());
        let Ok(pool) = pool else {
            panic!("sqlite pool should be created");
        };

        let create_table = sqlx::query(
            r#"
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
        .await;
        assert!(create_table.is_ok());

        let begin = pool.begin().await;
        assert!(begin.is_ok());
        let Ok(mut transaction) = begin else {
            panic!("transaction should be created");
        };

        let write_result = batch_upsert_ip_traffic_stats_in_tx(
            &mut transaction,
            &[
                IpTrafficStatsUpdate {
                    dst_ip: "1.1.1.1".into(),
                    day: "2026-04-01".into(),
                    upload: 10,
                    download: 20,
                },
                IpTrafficStatsUpdate {
                    dst_ip: "1.1.1.1".into(),
                    day: "2026-04-01".into(),
                    upload: 5,
                    download: 7,
                },
            ],
        )
        .await;
        assert!(write_result.is_ok());
        let commit = transaction.commit().await;
        assert!(commit.is_ok());

        let row = sqlx::query(
            r#"
SELECT upload, download
FROM ip_traffic_daily
WHERE dst_ip = '1.1.1.1' AND day = '2026-04-01';
"#,
        )
        .fetch_one(&pool)
        .await;
        assert!(row.is_ok());
        let Ok(row) = row else {
            panic!("ip traffic daily row should be queryable");
        };

        assert_eq!(row.get::<i64, _>("upload"), 15);
        assert_eq!(row.get::<i64, _>("download"), 27);
    }
}
