use serde::{Deserialize, Serialize};
use sqlx::Row;
use tauri_plugin_sql::DbPool;

use super::{sqlite_pool, try_col, DbError};

const SNAPSHOT_RETAIN_LIMIT: i64 = 100;

pub struct SnapshotRepo;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConfigSnapshot {
    pub id: i64,
    pub content: String,
    pub source: String,
    pub description: Option<String>,
    pub file_path: Option<String>,
    pub created_at: String,
}

impl SnapshotRepo {
    pub async fn create(
        db: &DbPool,
        content: &str,
        source: &str,
        description: Option<&str>,
        file_path: Option<&str>,
    ) -> Result<i64, DbError> {
        let pool = sqlite_pool(db)?;
        let result = sqlx::query(
            r#"
INSERT INTO config_snapshots (content, source, description, file_path)
VALUES (?, ?, ?, ?);
"#,
        )
        .bind(content)
        .bind(source)
        .bind(description)
        .bind(file_path)
        .execute(pool)
        .await
        .map_err(|error| DbError::WriteFailed(format!("写入配置快照失败: {error}")))?;

        Ok(result.last_insert_rowid())
    }

    pub async fn list(
        db: &DbPool,
        limit: i32,
        offset: i32,
    ) -> Result<Vec<ConfigSnapshot>, DbError> {
        if limit <= 0 {
            return Ok(Vec::new());
        }

        let pool = sqlite_pool(db)?;
        let rows = sqlx::query(
            r#"
SELECT id, content, source, description, file_path, created_at
FROM config_snapshots
ORDER BY datetime(created_at) DESC, id DESC
LIMIT ? OFFSET ?;
"#,
        )
        .bind(i64::from(limit.max(0)))
        .bind(i64::from(offset.max(0)))
        .fetch_all(pool)
        .await
        .map_err(|error| DbError::QueryFailed(format!("查询配置快照失败: {error}")))?;

        let mut snapshots = Vec::with_capacity(rows.len());
        for row in rows {
            snapshots.push(ConfigSnapshot {
                id: try_col!(row, "id", "配置快照"),
                content: try_col!(row, "content", "配置快照"),
                source: try_col!(row, "source", "配置快照"),
                description: try_col!(row, "description", "配置快照"),
                file_path: try_col!(row, "file_path", "配置快照"),
                created_at: try_col!(row, "created_at", "配置快照"),
            });
        }

        Ok(snapshots)
    }

    pub async fn get_by_id(db: &DbPool, id: i64) -> Result<Option<ConfigSnapshot>, DbError> {
        let pool = sqlite_pool(db)?;
        let row = sqlx::query(
            r#"
SELECT id, content, source, description, file_path, created_at
FROM config_snapshots
WHERE id = ?;
"#,
        )
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(|error| DbError::QueryFailed(format!("查询配置快照详情失败: {error}")))?;

        if let Some(row) = row {
            return Ok(Some(ConfigSnapshot {
                id: try_col!(row, "id", "配置快照"),
                content: try_col!(row, "content", "配置快照"),
                source: try_col!(row, "source", "配置快照"),
                description: try_col!(row, "description", "配置快照"),
                file_path: try_col!(row, "file_path", "配置快照"),
                created_at: try_col!(row, "created_at", "配置快照"),
            }));
        }

        Ok(None)
    }

    pub async fn cleanup(db: &DbPool) -> Result<u64, DbError> {
        let pool = sqlite_pool(db)?;
        let result = sqlx::query(
            r#"
DELETE FROM config_snapshots
WHERE id NOT IN (
    SELECT id
    FROM config_snapshots
    ORDER BY datetime(created_at) DESC, id DESC
    LIMIT ?
);
"#,
        )
        .bind(SNAPSHOT_RETAIN_LIMIT)
        .execute(pool)
        .await
        .map_err(|error| DbError::WriteFailed(format!("清理配置快照失败: {error}")))?;

        Ok(result.rows_affected())
    }
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
CREATE TABLE config_snapshots (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    content     TEXT NOT NULL,
    source      TEXT NOT NULL,
    description TEXT,
    file_path   TEXT,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);
"#,
        )
        .execute(&pool)
        .await
        .map_err(|error| error.to_string())?;

        Ok(DbPool::Sqlite(pool))
    }

    #[tokio::test]
    async fn snapshot_repo_create_list_and_get_by_id_work() {
        let db = prepare_db().await;
        assert!(db.is_ok());
        let Ok(db) = db else {
            panic!("snapshot test database should be created");
        };

        let first_id = SnapshotRepo::create(
            &db,
            "mixed-port: 7890\n",
            "manual",
            Some("手动备份"),
            Some("C:/config.yaml"),
        )
        .await;
        assert!(first_id.is_ok());
        let Ok(first_id) = first_id else {
            panic!("first snapshot should be created");
        };

        let second_id = SnapshotRepo::create(
            &db,
            "mixed-port: 7891\n",
            "ai",
            Some("AI 修改前备份"),
            Some("C:/config.yaml"),
        )
        .await;
        assert!(second_id.is_ok());
        let Ok(second_id) = second_id else {
            panic!("second snapshot should be created");
        };

        let snapshots = SnapshotRepo::list(&db, 10, 0).await;
        assert!(snapshots.is_ok());
        let Ok(snapshots) = snapshots else {
            panic!("snapshots should be listed");
        };

        assert_eq!(snapshots.len(), 2);
        assert_eq!(snapshots[0].id, second_id);
        assert_eq!(snapshots[1].id, first_id);
        assert_eq!(snapshots[0].source, "ai");

        let snapshot = SnapshotRepo::get_by_id(&db, first_id).await;
        assert!(snapshot.is_ok());
        let Ok(snapshot) = snapshot else {
            panic!("snapshot should be queryable by id");
        };

        assert!(snapshot.is_some());
        let Some(snapshot) = snapshot else {
            panic!("snapshot detail should exist");
        };

        assert_eq!(snapshot.id, first_id);
        assert_eq!(snapshot.content, "mixed-port: 7890\n");
        assert_eq!(snapshot.source, "manual");
        assert_eq!(snapshot.description.as_deref(), Some("手动备份"));
        assert_eq!(snapshot.file_path.as_deref(), Some("C:/config.yaml"));
        assert!(!snapshot.created_at.is_empty());
    }

    #[tokio::test]
    async fn snapshot_repo_cleanup_keeps_latest_hundred_items() {
        let db = prepare_db().await;
        assert!(db.is_ok());
        let Ok(db) = db else {
            panic!("snapshot test database should be created");
        };

        for index in 0..105 {
            let result =
                SnapshotRepo::create(&db, &format!("snapshot-{index}"), "manual", None, None).await;
            assert!(result.is_ok());
        }

        let deleted = SnapshotRepo::cleanup(&db).await;
        assert!(deleted.is_ok());
        let Ok(deleted) = deleted else {
            panic!("snapshot cleanup should succeed");
        };

        assert_eq!(deleted, 5);

        let snapshots = SnapshotRepo::list(&db, 200, 0).await;
        assert!(snapshots.is_ok());
        let Ok(snapshots) = snapshots else {
            panic!("snapshots should be listed after cleanup");
        };

        assert_eq!(snapshots.len(), 100);
        assert_eq!(snapshots[0].content, "snapshot-104");
        assert_eq!(snapshots[99].content, "snapshot-5");
    }
}
