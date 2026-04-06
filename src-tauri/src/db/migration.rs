use tauri_plugin_sql::{Migration, MigrationKind};

pub const DATABASE_URL: &str = "sqlite:clashmind.db";

pub fn get_migrations() -> Vec<Migration> {
    vec![
        Migration {
            version: 1,
            description: "create_connections_table",
            sql: r#"
CREATE TABLE IF NOT EXISTS connections (
    id           TEXT PRIMARY KEY,
    host         TEXT NOT NULL,
    dst_ip       TEXT,
    dst_port     INTEGER,
    src_ip       TEXT,
    src_port     INTEGER,
    network      TEXT NOT NULL,
    conn_type    TEXT NOT NULL,
    rule         TEXT NOT NULL,
    rule_payload TEXT,
    proxy_chain  TEXT NOT NULL,
    upload       INTEGER NOT NULL DEFAULT 0,
    download     INTEGER NOT NULL DEFAULT 0,
    start_time   TEXT NOT NULL,
    close_time   TEXT,
    created_at   TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_connections_host ON connections(host);
CREATE INDEX IF NOT EXISTS idx_connections_start_time ON connections(start_time);
CREATE INDEX IF NOT EXISTS idx_connections_rule ON connections(rule);
"#,
            kind: MigrationKind::Up,
        },
        Migration {
            version: 2,
            description: "create_traffic_hourly_table",
            sql: r#"
CREATE TABLE IF NOT EXISTS traffic_hourly (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    hour       TEXT NOT NULL,
    upload     INTEGER NOT NULL DEFAULT 0,
    download   INTEGER NOT NULL DEFAULT 0,
    conn_count INTEGER NOT NULL DEFAULT 0,
    UNIQUE(hour)
);
"#,
            kind: MigrationKind::Up,
        },
        Migration {
            version: 3,
            description: "create_traffic_daily_table",
            sql: r#"
CREATE TABLE IF NOT EXISTS traffic_daily (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    day        TEXT NOT NULL,
    upload     INTEGER NOT NULL DEFAULT 0,
    download   INTEGER NOT NULL DEFAULT 0,
    conn_count INTEGER NOT NULL DEFAULT 0,
    UNIQUE(day)
);
"#,
            kind: MigrationKind::Up,
        },
        Migration {
            version: 4,
            description: "create_domain_stats_table",
            sql: r#"
CREATE TABLE IF NOT EXISTS domain_stats (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    domain     TEXT NOT NULL,
    day        TEXT NOT NULL,
    hit_count  INTEGER NOT NULL DEFAULT 0,
    upload     INTEGER NOT NULL DEFAULT 0,
    download   INTEGER NOT NULL DEFAULT 0,
    UNIQUE(domain, day)
);

CREATE INDEX IF NOT EXISTS idx_domain_stats_day ON domain_stats(day);
"#,
            kind: MigrationKind::Up,
        },
        Migration {
            version: 5,
            description: "create_geoip_cache_table",
            sql: r#"
CREATE TABLE IF NOT EXISTS geoip_cache (
    ip           TEXT PRIMARY KEY,
    country      TEXT,
    country_code TEXT,
    city         TEXT,
    latitude     REAL,
    longitude    REAL,
    updated_at   TEXT NOT NULL DEFAULT (datetime('now'))
);
"#,
            kind: MigrationKind::Up,
        },
        Migration {
            version: 6,
            description: "create_traffic_samples_table",
            sql: r#"
CREATE TABLE IF NOT EXISTS traffic_samples (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    ts         TEXT NOT NULL,
    upload     INTEGER NOT NULL DEFAULT 0,
    download   INTEGER NOT NULL DEFAULT 0,
    conn_count INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_traffic_samples_ts ON traffic_samples(ts);
"#,
            kind: MigrationKind::Up,
        },
        Migration {
            version: 7,
            description: "add_connections_last_observed_at",
            sql: r#"
ALTER TABLE connections ADD COLUMN last_observed_at TEXT;

UPDATE connections
SET last_observed_at = CASE
    WHEN close_time IS NULL THEN strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
    ELSE COALESCE(
        close_time,
        strftime('%Y-%m-%dT%H:%M:%SZ', datetime(created_at)),
        start_time
    )
END
WHERE last_observed_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_connections_last_observed_at ON connections(last_observed_at);
"#,
            kind: MigrationKind::Up,
        },
        Migration {
            version: 8,
            description: "create_rule_stats_table",
            sql: r#"
CREATE TABLE IF NOT EXISTS rule_stats (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    rule       TEXT NOT NULL,
    day        TEXT NOT NULL,
    hit_count  INTEGER NOT NULL DEFAULT 0,
    upload     INTEGER NOT NULL DEFAULT 0,
    download   INTEGER NOT NULL DEFAULT 0,
    UNIQUE(rule, day)
);

CREATE INDEX IF NOT EXISTS idx_rule_stats_day ON rule_stats(day);
"#,
            kind: MigrationKind::Up,
        },
        Migration {
            version: 9,
            description: "repair_open_connection_observation_baseline",
            sql: r#"
UPDATE connections
SET last_observed_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
WHERE close_time IS NULL;
"#,
            kind: MigrationKind::Up,
        },
        Migration {
            version: 10,
            description: "create_ip_traffic_daily_table",
            sql: r#"
CREATE TABLE IF NOT EXISTS ip_traffic_daily (
    id       INTEGER PRIMARY KEY AUTOINCREMENT,
    dst_ip   TEXT NOT NULL,
    day      TEXT NOT NULL,
    upload   INTEGER NOT NULL DEFAULT 0,
    download INTEGER NOT NULL DEFAULT 0,
    UNIQUE(dst_ip, day)
);

CREATE INDEX IF NOT EXISTS idx_ip_traffic_daily_day ON ip_traffic_daily(day);
CREATE INDEX IF NOT EXISTS idx_ip_traffic_daily_dst_ip ON ip_traffic_daily(dst_ip);
"#,
            kind: MigrationKind::Up,
        },
        Migration {
            version: 11,
            description: "create_config_snapshots_table",
            sql: r#"
CREATE TABLE IF NOT EXISTS config_snapshots (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    content     TEXT NOT NULL,
    source      TEXT NOT NULL,
    description TEXT,
    file_path   TEXT,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_snapshots_created_at ON config_snapshots(created_at);
"#,
            kind: MigrationKind::Up,
        },
        Migration {
            version: 12,
            description: "create_ai_conversations_table",
            sql: r#"
CREATE TABLE IF NOT EXISTS ai_conversations (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    role        TEXT NOT NULL,
    content     TEXT NOT NULL,
    tool_calls  TEXT,
    tokens_used INTEGER,
    model       TEXT,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_conversations_created_at ON ai_conversations(created_at);
"#,
            kind: MigrationKind::Up,
        },
    ]
}

#[cfg(test)]
mod tests {
    use sqlx::{sqlite::SqlitePoolOptions, Row};

    use super::*;

    async fn execute_migration(version: i64, pool: &sqlx::SqlitePool) -> Result<(), String> {
        let Some(migration) = get_migrations()
            .into_iter()
            .find(|migration| migration.version == version)
        else {
            return Err(format!("migration {version} not found"));
        };

        sqlx::query(migration.sql)
            .execute(pool)
            .await
            .map_err(|error| error.to_string())?;

        Ok(())
    }

    #[tokio::test]
    async fn last_observed_at_backfill_uses_upgrade_time_for_open_connections() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await;
        assert!(pool.is_ok());
        let Ok(pool) = pool else {
            panic!("sqlite pool should be created");
        };

        let create_connections = sqlx::query(
            r#"
CREATE TABLE connections (
    id           TEXT PRIMARY KEY,
    host         TEXT NOT NULL,
    dst_ip       TEXT,
    dst_port     INTEGER,
    src_ip       TEXT,
    src_port     INTEGER,
    network      TEXT NOT NULL,
    conn_type    TEXT NOT NULL,
    rule         TEXT NOT NULL,
    rule_payload TEXT,
    proxy_chain  TEXT NOT NULL,
    upload       INTEGER NOT NULL DEFAULT 0,
    download     INTEGER NOT NULL DEFAULT 0,
    start_time   TEXT NOT NULL,
    close_time   TEXT,
    created_at   TEXT NOT NULL DEFAULT (datetime('now'))
);
"#,
        )
        .execute(&pool)
        .await;
        assert!(create_connections.is_ok());

        let seed = sqlx::query(
            r#"
INSERT INTO connections (
    id, host, network, conn_type, rule, proxy_chain, upload, download, start_time, close_time, created_at
) VALUES
    (
        'open-conn',
        'open.example',
        'tcp',
        'HTTPS',
        'MATCH',
        '["DIRECT"]',
        10,
        20,
        '2026-03-01T08:00:00Z',
        NULL,
        '2026-03-01 08:00:00'
    ),
    (
        'closed-conn',
        'closed.example',
        'tcp',
        'HTTPS',
        'MATCH',
        '["DIRECT"]',
        30,
        40,
        '2026-03-01T09:00:00Z',
        '2026-03-01T10:00:00Z',
        '2026-03-01 09:00:00'
    );
"#,
        )
        .execute(&pool)
        .await;
        assert!(seed.is_ok());

        let migration_result = execute_migration(7, &pool).await;
        assert!(migration_result.is_ok());

        let open_is_recent = sqlx::query_scalar::<_, i64>(
            r#"
SELECT CASE
    WHEN datetime(last_observed_at) >= datetime('now', '-1 minute') THEN 1
    ELSE 0
END
FROM connections
WHERE id = 'open-conn';
"#,
        )
        .fetch_one(&pool)
        .await;
        assert!(open_is_recent.is_ok());
        let Ok(open_is_recent) = open_is_recent else {
            panic!("open connection last_observed_at should be queryable");
        };

        assert_eq!(open_is_recent, 1);

        let closed_last_observed_at = sqlx::query(
            r#"
SELECT last_observed_at
FROM connections
WHERE id = 'closed-conn';
"#,
        )
        .fetch_one(&pool)
        .await;
        assert!(closed_last_observed_at.is_ok());
        let Ok(closed_last_observed_at) = closed_last_observed_at else {
            panic!("closed connection last_observed_at should be queryable");
        };

        assert_eq!(
            closed_last_observed_at.get::<String, _>("last_observed_at"),
            "2026-03-01T10:00:00Z"
        );
    }

    #[tokio::test]
    async fn repair_migration_resets_only_open_connection_baselines() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await;
        assert!(pool.is_ok());
        let Ok(pool) = pool else {
            panic!("sqlite pool should be created");
        };

        let create_connections = sqlx::query(
            r#"
CREATE TABLE connections (
    id               TEXT PRIMARY KEY,
    host             TEXT NOT NULL,
    dst_ip           TEXT,
    dst_port         INTEGER,
    src_ip           TEXT,
    src_port         INTEGER,
    network          TEXT NOT NULL,
    conn_type        TEXT NOT NULL,
    rule             TEXT NOT NULL,
    rule_payload     TEXT,
    proxy_chain      TEXT NOT NULL,
    upload           INTEGER NOT NULL DEFAULT 0,
    download         INTEGER NOT NULL DEFAULT 0,
    start_time       TEXT NOT NULL,
    close_time       TEXT,
    created_at       TEXT NOT NULL DEFAULT (datetime('now')),
    last_observed_at TEXT
);
"#,
        )
        .execute(&pool)
        .await;
        assert!(create_connections.is_ok());

        let seed = sqlx::query(
            r#"
INSERT INTO connections (
    id, host, network, conn_type, rule, proxy_chain, upload, download, start_time, close_time, created_at, last_observed_at
) VALUES
    (
        'open-conn',
        'open.example',
        'tcp',
        'HTTPS',
        'MATCH',
        '["DIRECT"]',
        10,
        20,
        '2026-03-01T08:00:00Z',
        NULL,
        '2026-03-01 08:00:00',
        '2026-03-01T08:00:00Z'
    ),
    (
        'closed-conn',
        'closed.example',
        'tcp',
        'HTTPS',
        'MATCH',
        '["DIRECT"]',
        30,
        40,
        '2026-03-01T09:00:00Z',
        '2026-03-01T10:00:00Z',
        '2026-03-01 09:00:00',
        '2026-03-01T10:00:00Z'
    );
"#,
        )
        .execute(&pool)
        .await;
        assert!(seed.is_ok());

        let migration_result = execute_migration(9, &pool).await;
        assert!(migration_result.is_ok());

        let open_is_recent = sqlx::query_scalar::<_, i64>(
            r#"
SELECT CASE
    WHEN datetime(last_observed_at) >= datetime('now', '-1 minute') THEN 1
    ELSE 0
END
FROM connections
WHERE id = 'open-conn';
"#,
        )
        .fetch_one(&pool)
        .await;
        assert!(open_is_recent.is_ok());
        let Ok(open_is_recent) = open_is_recent else {
            panic!("open connection last_observed_at should be queryable");
        };

        assert_eq!(open_is_recent, 1);

        let closed_last_observed_at = sqlx::query(
            r#"
SELECT last_observed_at
FROM connections
WHERE id = 'closed-conn';
"#,
        )
        .fetch_one(&pool)
        .await;
        assert!(closed_last_observed_at.is_ok());
        let Ok(closed_last_observed_at) = closed_last_observed_at else {
            panic!("closed connection last_observed_at should be queryable");
        };

        assert_eq!(
            closed_last_observed_at.get::<String, _>("last_observed_at"),
            "2026-03-01T10:00:00Z"
        );
    }

    #[tokio::test]
    async fn config_snapshots_migration_is_idempotent() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await;
        assert!(pool.is_ok());
        let Ok(pool) = pool else {
            panic!("sqlite pool should be created");
        };

        let first_run = execute_migration(11, &pool).await;
        assert!(first_run.is_ok());
        let second_run = execute_migration(11, &pool).await;
        assert!(second_run.is_ok());

        let table_exists = sqlx::query_scalar::<_, i64>(
            r#"
SELECT COUNT(*)
FROM sqlite_master
WHERE type = 'table' AND name = 'config_snapshots';
"#,
        )
        .fetch_one(&pool)
        .await;
        assert!(table_exists.is_ok());
        let Ok(table_exists) = table_exists else {
            panic!("config_snapshots table should be queryable");
        };

        assert_eq!(table_exists, 1);
    }

    #[tokio::test]
    async fn ai_conversations_migration_is_idempotent() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await;
        assert!(pool.is_ok());
        let Ok(pool) = pool else {
            panic!("sqlite pool should be created");
        };

        let first_run = execute_migration(12, &pool).await;
        assert!(first_run.is_ok());
        let second_run = execute_migration(12, &pool).await;
        assert!(second_run.is_ok());

        let index_exists = sqlx::query_scalar::<_, i64>(
            r#"
SELECT COUNT(*)
FROM sqlite_master
WHERE type = 'index' AND name = 'idx_conversations_created_at';
"#,
        )
        .fetch_one(&pool)
        .await;
        assert!(index_exists.is_ok());
        let Ok(index_exists) = index_exists else {
            panic!("ai_conversations index should be queryable");
        };

        assert_eq!(index_exists, 1);
    }
}
