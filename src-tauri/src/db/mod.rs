pub mod cleanup;
pub mod migration;
pub mod plugin;
pub mod repo_connection;
pub mod repo_conversation;
pub mod repo_domain;
pub mod repo_geoip;
pub mod repo_snapshot;
pub mod repo_traffic;

use serde::Serialize;
use sqlx::SqlitePool;
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_sql::{DbInstances, DbPool};
use thiserror::Error;

use self::migration::DATABASE_URL;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("数据库未加载: {0}")]
    NotLoaded(String),
    #[error("不支持的数据库驱动: {0}")]
    UnsupportedDriver(String),
    #[error("数据库事务失败: {0}")]
    TransactionFailed(String),
    #[error("数据库写入失败: {0}")]
    WriteFailed(String),
    #[error("时间窗口无效: {0}")]
    InvalidTimeWindow(String),
    #[error("数据库查询失败: {0}")]
    QueryFailed(String),
}

impl Serialize for DbError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

pub async fn get_db_pool<R: Runtime>(app: &AppHandle<R>) -> Result<DbPool, DbError> {
    let instances = app.state::<DbInstances>();
    let guard = instances.0.read().await;
    let db = guard
        .get(DATABASE_URL)
        .ok_or_else(|| DbError::NotLoaded(DATABASE_URL.to_string()))?;

    #[allow(unreachable_patterns)]
    match db {
        DbPool::Sqlite(pool) => Ok(DbPool::Sqlite(pool.clone())),
        _ => Err(DbError::UnsupportedDriver(DATABASE_URL.to_string())),
    }
}

pub(crate) fn sqlite_pool(db: &DbPool) -> Result<&SqlitePool, DbError> {
    #[allow(unreachable_patterns)]
    match db {
        DbPool::Sqlite(pool) => Ok(pool),
        _ => Err(DbError::UnsupportedDriver(DATABASE_URL.to_string())),
    }
}

macro_rules! try_col {
    ($row:expr, $col:literal) => {
        $row.try_get($col).map_err(|error| {
            $crate::db::DbError::QueryFailed(format!("读取列 {} 失败: {error}", $col))
        })?
    };
    ($row:expr, $col:literal, $ctx:literal) => {
        $row.try_get($col).map_err(|error| {
            $crate::db::DbError::QueryFailed(format!("读取{} {} 失败: {error}", $ctx, $col))
        })?
    };
}

pub(crate) use try_col;
