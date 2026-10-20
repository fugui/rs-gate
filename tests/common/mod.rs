use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
use std::path::PathBuf;
use tempfile::TempDir;

pub mod mock_llm;

/// 隔离的 SQLite 测试环境夹具，在 Drop 时自动清理磁盘文件
#[allow(dead_code)]
pub struct TestDbHarness {
    pub pool: SqlitePool,
    pub temp_dir: TempDir,
    pub db_path: PathBuf,
}

impl TestDbHarness {
    pub async fn new() -> anyhow::Result<Self> {
        let temp_dir = tempfile::tempdir()?;
        let db_path = temp_dir.path().join("test_gate.db");
        let conn_str = format!("sqlite://{}?mode=rwc", db_path.display());

        // 初始连接池
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&conn_str)
            .await?;

        Ok(Self {
            pool,
            temp_dir,
            db_path,
        })
    }
}
