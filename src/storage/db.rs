use crate::config::DatabaseConfig;
use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
    SqlitePool,
};
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

/// 初始化高性能 SQLite WAL 连接池并注入关键 Pragmas
pub async fn init_db_pool(config: &DatabaseConfig) -> Result<SqlitePool, sqlx::Error> {
    // 若指向磁盘物理路径，确保父级目录存在
    if let Some(path_str) = extract_sqlite_filepath(&config.url) {
        if let Some(parent) = Path::new(&path_str).parent() {
            if !parent.as_os_str().is_empty() {
                let _ = std::fs::create_dir_all(parent);
            }
        }
    }

    let connect_opts = SqliteConnectOptions::from_str(&config.url)?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .busy_timeout(Duration::from_millis(config.busy_timeout_ms))
        .foreign_keys(true)
        .pragma("temp_store", "MEMORY")
        .pragma("cache_size", "-64000");

    let pool = SqlitePoolOptions::new()
        .max_connections(config.max_connections)
        .min_connections(config.min_connections)
        .connect_with(connect_opts)
        .await?;

    Ok(pool)
}

fn extract_sqlite_filepath(url: &str) -> Option<String> {
    let clean = url.strip_prefix("sqlite://").unwrap_or(url);
    let path_part = clean.split('?').next().unwrap_or(clean);
    if path_part == ":memory:" || path_part.is_empty() {
        None
    } else {
        Some(path_part.to_string())
    }
}
