use serde::{Deserialize, Serialize};

use crate::db::{self, repo_connection, repo_domain, repo_traffic, DbError};

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
