use serde::{Deserialize, Serialize};
use sqlx::Row;
use tauri_plugin_sql::DbPool;

use super::{sqlite_pool, try_col, DbError};

pub struct ConversationRepo;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConversationMessage {
    pub id: i64,
    pub role: String,
    pub content: String,
    pub tool_calls: Option<String>,
    pub tokens_used: Option<i32>,
    pub model: Option<String>,
    pub created_at: String,
}

impl ConversationRepo {
    pub async fn save_message(
        db: &DbPool,
        role: &str,
        content: &str,
        tool_calls: Option<&str>,
        tokens_used: Option<i32>,
        model: Option<&str>,
    ) -> Result<i64, DbError> {
        let pool = sqlite_pool(db)?;
        let result = sqlx::query(
            r#"
INSERT INTO ai_conversations (role, content, tool_calls, tokens_used, model)
VALUES (?, ?, ?, ?, ?);
"#,
        )
        .bind(role)
        .bind(content)
        .bind(tool_calls)
        .bind(tokens_used)
        .bind(model)
        .execute(pool)
        .await
        .map_err(|error| DbError::WriteFailed(format!("写入 AI 对话失败: {error}")))?;

        Ok(result.last_insert_rowid())
    }

    #[allow(dead_code)]
    pub async fn get_recent(db: &DbPool, limit: i32) -> Result<Vec<ConversationMessage>, DbError> {
        if limit <= 0 {
            return Ok(Vec::new());
        }

        let pool = sqlite_pool(db)?;
        let rows = sqlx::query(
            r#"
SELECT id, role, content, tool_calls, tokens_used, model, created_at
FROM ai_conversations
ORDER BY datetime(created_at) DESC, id DESC
LIMIT ?;
"#,
        )
        .bind(i64::from(limit.max(0)))
        .fetch_all(pool)
        .await
        .map_err(|error| DbError::QueryFailed(format!("查询 AI 对话失败: {error}")))?;

        let mut messages = Vec::with_capacity(rows.len());
        for row in rows {
            messages.push(ConversationMessage {
                id: try_col!(row, "id", "AI 对话"),
                role: try_col!(row, "role", "AI 对话"),
                content: try_col!(row, "content", "AI 对话"),
                tool_calls: try_col!(row, "tool_calls", "AI 对话"),
                tokens_used: try_col!(row, "tokens_used", "AI 对话"),
                model: try_col!(row, "model", "AI 对话"),
                created_at: try_col!(row, "created_at", "AI 对话"),
            });
        }
        messages.reverse();

        Ok(messages)
    }

    pub async fn cleanup(db: &DbPool) -> Result<u64, DbError> {
        let pool = sqlite_pool(db)?;
        let result = sqlx::query(
            r#"
DELETE FROM ai_conversations
WHERE datetime(created_at) < datetime('now', '-7 days');
"#,
        )
        .execute(pool)
        .await
        .map_err(|error| DbError::WriteFailed(format!("清理 AI 对话失败: {error}")))?;

        Ok(result.rows_affected())
    }
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
CREATE TABLE ai_conversations (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    role        TEXT NOT NULL,
    content     TEXT NOT NULL,
    tool_calls  TEXT,
    tokens_used INTEGER,
    model       TEXT,
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
    async fn conversation_repo_save_and_get_recent_work() {
        let db = prepare_db().await;
        assert!(db.is_ok());
        let Ok(db) = db else {
            panic!("conversation test database should be created");
        };

        let first_id = ConversationRepo::save_message(
            &db,
            "user",
            "请帮我检查当前配置",
            None,
            None,
            Some("gpt-4o-mini"),
        )
        .await;
        assert!(first_id.is_ok());

        let second_id = ConversationRepo::save_message(
            &db,
            "assistant",
            "我先检查代理组和规则。",
            Some(r#"[{"name":"get_config"}]"#),
            Some(128),
            Some("gpt-4o-mini"),
        )
        .await;
        assert!(second_id.is_ok());

        let messages = ConversationRepo::get_recent(&db, 10).await;
        assert!(messages.is_ok());
        let Ok(messages) = messages else {
            panic!("recent conversations should be queryable");
        };

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[1].role, "assistant");
        assert_eq!(messages[1].tokens_used, Some(128));
    }

    #[tokio::test]
    async fn conversation_repo_cleanup_removes_rows_older_than_seven_days() {
        let db = prepare_db().await;
        assert!(db.is_ok());
        let Ok(db) = db else {
            panic!("conversation test database should be created");
        };

        let recent = ConversationRepo::save_message(&db, "user", "recent", None, None, None).await;
        assert!(recent.is_ok());

        let pool = sqlite_pool(&db);
        assert!(pool.is_ok());
        let Ok(pool) = pool else {
            panic!("sqlite pool should be available");
        };

        let insert_old = sqlx::query(
            r#"
INSERT INTO ai_conversations (role, content, created_at)
VALUES ('assistant', 'old', datetime('now', '-8 days'));
"#,
        )
        .execute(pool)
        .await;
        assert!(insert_old.is_ok());

        let deleted = ConversationRepo::cleanup(&db).await;
        assert!(deleted.is_ok());
        let Ok(deleted) = deleted else {
            panic!("conversation cleanup should succeed");
        };

        assert_eq!(deleted, 1);

        let remaining = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM ai_conversations;")
            .fetch_one(pool)
            .await;
        assert!(remaining.is_ok());
        let Ok(remaining) = remaining else {
            panic!("remaining conversation count should be queryable");
        };

        assert_eq!(remaining, 1);

        let content = sqlx::query("SELECT content FROM ai_conversations LIMIT 1;")
            .fetch_one(pool)
            .await;
        assert!(content.is_ok());
        let Ok(content) = content else {
            panic!("remaining conversation should be queryable");
        };

        assert_eq!(content.get::<String, _>("content"), "recent");
    }
}
