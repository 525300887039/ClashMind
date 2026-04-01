use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::{
    db::{self, cleanup, repo_connection, repo_domain, repo_traffic, sqlite_pool, DbError},
    utils::geoip::{resolve_country_mmdb_path, GeoIpConfigState, GeoIpLookup},
};

const MAX_IP_API_LOOKUPS_PER_REQUEST: usize = 3;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainStat {
    pub domain: String,
    pub hit_count: i64,
    pub upload: i64,
    pub download: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrafficPoint {
    pub time: String,
    pub upload: i64,
    pub download: i64,
    pub conn_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsOverview {
    pub total_connections: i64,
    pub total_upload: i64,
    pub total_download: i64,
    pub active_connections: i64,
    pub unique_domains: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleStat {
    pub rule: String,
    pub hit_count: i64,
    pub upload: i64,
    pub download: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeoStat {
    pub country_code: String,
    pub country: String,
    pub conn_count: i64,
    pub total_traffic: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DbStats {
    pub connection_count: i64,
    pub hourly_count: i64,
    pub daily_count: i64,
    pub domain_count: i64,
    pub geoip_count: i64,
    pub db_size_bytes: i64,
    pub oldest_connection: Option<String>,
}

fn merge_geo_stat(
    aggregated: &mut HashMap<String, GeoStat>,
    row: &repo_connection::GeoIpTrafficRow,
    country_code: String,
    country: String,
) {
    let entry = aggregated
        .entry(country_code.clone())
        .or_insert_with(|| GeoStat {
            country_code,
            country,
            conn_count: 0,
            total_traffic: 0,
        });
    entry.conn_count += row.conn_count;
    entry.total_traffic += row.total_traffic;
}

impl From<repo_domain::TopDomainRow> for DomainStat {
    fn from(value: repo_domain::TopDomainRow) -> Self {
        Self {
            domain: value.domain,
            hit_count: value.hit_count,
            upload: value.upload,
            download: value.download,
        }
    }
}

impl From<repo_traffic::TrafficBucketRow> for TrafficPoint {
    fn from(value: repo_traffic::TrafficBucketRow) -> Self {
        Self {
            time: value.time,
            upload: value.upload,
            download: value.download,
            conn_count: value.conn_count,
        }
    }
}

impl From<repo_connection::ConnectionOverview> for StatsOverview {
    fn from(value: repo_connection::ConnectionOverview) -> Self {
        Self {
            total_connections: value.total_connections,
            total_upload: value.total_upload,
            total_download: value.total_download,
            active_connections: value.active_connections,
            unique_domains: value.unique_domains,
        }
    }
}

impl From<repo_connection::RuleStatRow> for RuleStat {
    fn from(value: repo_connection::RuleStatRow) -> Self {
        Self {
            rule: value.rule,
            hit_count: value.hit_count,
            upload: value.upload,
            download: value.download,
        }
    }
}

#[tauri::command]
pub async fn manual_cleanup(
    app_handle: tauri::AppHandle,
) -> Result<cleanup::CleanupReport, DbError> {
    let db = db::get_db_pool(&app_handle).await?;
    let report = cleanup::run_full_cleanup(&db).await?;
    tracing::info!("手动数据清理完成: {:?}", report);

    Ok(report)
}

#[tauri::command]
pub async fn get_db_stats(app_handle: tauri::AppHandle) -> Result<DbStats, DbError> {
    let db = db::get_db_pool(&app_handle).await?;
    let pool = sqlite_pool(&db)?;
    let row = sqlx::query(
        r#"
SELECT
    (SELECT COUNT(*) FROM connections) AS connection_count,
    (SELECT COUNT(*) FROM traffic_hourly) AS hourly_count,
    (SELECT COUNT(*) FROM traffic_daily) AS daily_count,
    (SELECT COUNT(*) FROM domain_stats) AS domain_count,
    (SELECT COUNT(*) FROM geoip_cache) AS geoip_count,
    (SELECT MIN(start_time) FROM connections) AS oldest_connection;
"#,
    )
    .fetch_one(pool)
    .await
    .map_err(|error| DbError::QueryFailed(format!("查询数据库统计失败: {error}")))?;

    let page_count = sqlx::query_scalar::<_, i64>("PRAGMA page_count;")
        .fetch_one(pool)
        .await
        .map_err(|error| DbError::QueryFailed(format!("查询数据库页数失败: {error}")))?;
    let page_size = sqlx::query_scalar::<_, i64>("PRAGMA page_size;")
        .fetch_one(pool)
        .await
        .map_err(|error| DbError::QueryFailed(format!("查询数据库页大小失败: {error}")))?;
    let db_size_bytes = page_count
        .checked_mul(page_size)
        .ok_or_else(|| DbError::QueryFailed("计算数据库大小时发生溢出".to_string()))?;

    Ok(DbStats {
        connection_count: row.try_get("connection_count").map_err(|error| {
            DbError::QueryFailed(format!("读取数据库统计 connection_count 失败: {error}"))
        })?,
        hourly_count: row.try_get("hourly_count").map_err(|error| {
            DbError::QueryFailed(format!("读取数据库统计 hourly_count 失败: {error}"))
        })?,
        daily_count: row.try_get("daily_count").map_err(|error| {
            DbError::QueryFailed(format!("读取数据库统计 daily_count 失败: {error}"))
        })?,
        domain_count: row.try_get("domain_count").map_err(|error| {
            DbError::QueryFailed(format!("读取数据库统计 domain_count 失败: {error}"))
        })?,
        geoip_count: row.try_get("geoip_count").map_err(|error| {
            DbError::QueryFailed(format!("读取数据库统计 geoip_count 失败: {error}"))
        })?,
        db_size_bytes,
        oldest_connection: row.try_get("oldest_connection").map_err(|error| {
            DbError::QueryFailed(format!("读取数据库统计 oldest_connection 失败: {error}"))
        })?,
    })
}

#[tauri::command]
pub async fn get_domain_stats(
    app_handle: tauri::AppHandle,
    days: i32,
    limit: i32,
) -> Result<Vec<DomainStat>, DbError> {
    let db = db::get_db_pool(&app_handle).await?;
    let rows = repo_domain::query_top_domains(&db, days, limit).await?;

    Ok(rows.into_iter().map(Into::into).collect())
}

#[tauri::command]
pub async fn get_traffic_hourly(
    app_handle: tauri::AppHandle,
    start: String,
    end: String,
) -> Result<Vec<TrafficPoint>, DbError> {
    let db = db::get_db_pool(&app_handle).await?;
    let rows = repo_traffic::query_hourly(&db, &start, &end).await?;

    Ok(rows.into_iter().map(Into::into).collect())
}

#[tauri::command]
pub async fn get_traffic_daily(
    app_handle: tauri::AppHandle,
    start: String,
    end: String,
) -> Result<Vec<TrafficPoint>, DbError> {
    let db = db::get_db_pool(&app_handle).await?;
    let rows = repo_traffic::query_daily(&db, &start, &end).await?;

    Ok(rows.into_iter().map(Into::into).collect())
}

#[tauri::command]
pub async fn get_stats_overview(
    app_handle: tauri::AppHandle,
    days: i32,
) -> Result<StatsOverview, DbError> {
    let db = db::get_db_pool(&app_handle).await?;
    let overview = repo_connection::get_overview(&db, days).await?;

    Ok(overview.into())
}

#[tauri::command]
pub async fn get_rule_stats(
    app_handle: tauri::AppHandle,
    days: i32,
    limit: i32,
) -> Result<Vec<RuleStat>, DbError> {
    let db = db::get_db_pool(&app_handle).await?;
    let rows = repo_connection::query_rule_stats(&db, days, limit).await?;

    Ok(rows.into_iter().map(Into::into).collect())
}

#[tauri::command]
pub async fn get_geo_stats(
    app_handle: tauri::AppHandle,
    geoip_config: tauri::State<'_, GeoIpConfigState>,
    geoip_lookup: tauri::State<'_, GeoIpLookup>,
    days: i32,
) -> Result<Vec<GeoStat>, DbError> {
    let db = db::get_db_pool(&app_handle).await?;
    let rows = repo_connection::query_geo_traffic_by_ip(&db, days).await?;
    let mmdb_path = resolve_country_mmdb_path(&geoip_config.config_dir());
    let mut aggregated = HashMap::<String, GeoStat>::with_capacity(rows.len());
    let ips: Vec<String> = rows.iter().map(|row| row.dst_ip.clone()).collect();
    let resolved_geos = geoip_lookup
        .lookup_many(
            &db,
            mmdb_path.as_deref(),
            &ips,
            MAX_IP_API_LOOKUPS_PER_REQUEST,
        )
        .await?;

    for row in rows {
        let Some(geo) = resolved_geos.get(&row.dst_ip) else {
            continue;
        };

        let Some(country_code) = geo.country_code.clone() else {
            continue;
        };
        let country = geo.country.clone().unwrap_or_else(|| country_code.clone());

        merge_geo_stat(&mut aggregated, &row, country_code, country);
    }

    let mut stats: Vec<_> = aggregated.into_values().collect();
    stats.sort_by(|left, right| {
        right
            .total_traffic
            .cmp(&left.total_traffic)
            .then_with(|| right.conn_count.cmp(&left.conn_count))
            .then_with(|| left.country.cmp(&right.country))
    });

    Ok(stats)
}
