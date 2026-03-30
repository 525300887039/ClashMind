use std::{future::Future, path::PathBuf, str::FromStr};

use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode},
    Connection,
};
use tauri::{
    plugin::{Builder as PluginBuilder, TauriPlugin},
    AppHandle, Manager, Runtime,
};
use thiserror::Error;

use super::migration::DATABASE_URL;

const WAL_MODE: &str = "wal";

#[derive(Debug, Error)]
enum DbBootstrapError {
    #[error("invalid sqlite database url: {0}")]
    InvalidDatabaseUrl(String),
    #[error("failed to resolve app config directory")]
    ResolveAppConfigDir(#[source] tauri::Error),
    #[error("failed to create database directory at {path}")]
    CreateDatabaseDir {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("sqlite bootstrap failed")]
    Sqlite(#[from] sqlx::Error),
    #[error("sqlite journal mode is {actual}, expected wal")]
    UnexpectedJournalMode { actual: String },
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    PluginBuilder::new("db-bootstrap")
        .setup(|app, _api| {
            run_async(ensure_sqlite_wal(app))?;
            Ok(())
        })
        .build()
}

fn run_async<F: Future>(future: F) -> F::Output {
    if tokio::runtime::Handle::try_current().is_ok() {
        tokio::task::block_in_place(|| tokio::runtime::Handle::current().block_on(future))
    } else {
        tauri::async_runtime::block_on(future)
    }
}

async fn ensure_sqlite_wal<R: Runtime>(app: &AppHandle<R>) -> Result<(), DbBootstrapError> {
    let database_path = resolve_database_path(app, DATABASE_URL)?;

    if let Some(parent) = database_path.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|source| {
            DbBootstrapError::CreateDatabaseDir {
                path: parent.to_path_buf(),
                source,
            }
        })?;
    }

    let connection_url = format!("sqlite:{}", database_path.display());
    let options = SqliteConnectOptions::from_str(&connection_url)?.create_if_missing(true);
    let mut connection = sqlx::SqliteConnection::connect_with(&options).await?;
    let journal_mode: String = sqlx::query_scalar("PRAGMA journal_mode = WAL;")
        .fetch_one(&mut connection)
        .await?;

    if !journal_mode.eq_ignore_ascii_case(WAL_MODE) {
        return Err(DbBootstrapError::UnexpectedJournalMode {
            actual: journal_mode,
        });
    }

    connection.close().await?;

    let verification_options = SqliteConnectOptions::from_str(&connection_url)?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal);
    let mut verification_connection =
        sqlx::SqliteConnection::connect_with(&verification_options).await?;
    let verified_mode: String = sqlx::query_scalar("PRAGMA journal_mode;")
        .fetch_one(&mut verification_connection)
        .await?;

    if !verified_mode.eq_ignore_ascii_case(WAL_MODE) {
        return Err(DbBootstrapError::UnexpectedJournalMode {
            actual: verified_mode,
        });
    }

    verification_connection.close().await?;

    Ok(())
}

fn resolve_database_path<R: Runtime>(
    app: &AppHandle<R>,
    database_url: &str,
) -> Result<PathBuf, DbBootstrapError> {
    let relative_path = database_url
        .strip_prefix("sqlite:")
        .ok_or_else(|| DbBootstrapError::InvalidDatabaseUrl(database_url.to_string()))?;

    Ok(app
        .path()
        .app_config_dir()
        .map_err(DbBootstrapError::ResolveAppConfigDir)?
        .join(relative_path))
}

#[cfg(test)]
mod tests {
    use super::DbBootstrapError;

    #[test]
    fn rejects_non_sqlite_database_urls() {
        let result = "postgres://example"
            .strip_prefix("sqlite:")
            .ok_or_else(|| DbBootstrapError::InvalidDatabaseUrl("postgres://example".to_string()));

        assert!(matches!(
            result,
            Err(DbBootstrapError::InvalidDatabaseUrl(_))
        ));
    }
}
