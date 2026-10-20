use sqlx::SqlitePool;
use std::time::Duration;
use tracing::{error, info};

/// 历史审计日志 TTL 滚动清理与磁盘碎片增量收缩执行器
#[derive(Clone, Debug)]
pub struct RetentionCleaner {
    pool: SqlitePool,
    retention_days: u32,
    batch_size: usize,
}

impl RetentionCleaner {
    pub fn new(pool: SqlitePool, retention_days: u32) -> Self {
        Self {
            pool,
            retention_days,
            batch_size: 1000,
        }
    }

    pub fn with_batch_size(mut self, batch_size: usize) -> Self {
        self.batch_size = batch_size;
        self
    }

    /// 执行单次过期审计日志分批清理并触发增量磁盘页收缩
    pub async fn clean_expired_logs(&self) -> Result<u64, sqlx::Error> {
        let cutoff_ms =
            chrono::Utc::now().timestamp_millis() - (self.retention_days as i64 * 86_400 * 1000);

        let mut total_deleted = 0u64;

        loop {
            // 分批软删除：基于子查询限制单次删除规模，避免锁住 SQLite WAL 读写通道
            let res = sqlx::query(
                "DELETE FROM audit_logs WHERE id IN (SELECT id FROM audit_logs WHERE created_at < ? LIMIT ?);"
            )
            .bind(cutoff_ms)
            .bind(self.batch_size as i64)
            .execute(&self.pool)
            .await?;

            let affected = res.rows_affected();
            total_deleted += affected;

            if affected < self.batch_size as u64 || affected == 0 {
                break;
            }
        }

        if total_deleted > 0 {
            info!(
                purged_count = total_deleted,
                retention_days = self.retention_days,
                "Expired audit logs purged successfully"
            );
        }

        // 执行增量页碎片回收
        let _ = sqlx::query("PRAGMA incremental_vacuum(100);")
            .execute(&self.pool)
            .await;

        Ok(total_deleted)
    }

    /// 启动后台低优先级定时清理协程 (每 24 小时唤醒一次)
    pub async fn start_background_worker(self) {
        let mut interval = tokio::time::interval(Duration::from_secs(24 * 3600));
        loop {
            interval.tick().await;
            if let Err(err) = self.clean_expired_logs().await {
                error!("Failed to clean expired audit logs: {}", err);
            }
        }
    }
}
