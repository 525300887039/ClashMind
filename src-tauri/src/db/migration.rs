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
    ]
}
