use super::AuditEvent;
use chrono::{Datelike, TimeZone, Utc};
use sqlx::SqlitePool;
use std::time::Duration;
use tokio::sync::mpsc::Receiver;
use tokio::time::interval;

/// 异步审计事件后台批量落盘与配额原子同步 Worker
pub struct AuditWorker {
    receiver: Receiver<AuditEvent>,
    db_pool: SqlitePool,
    batch_size: usize,
    flush_interval: Duration,
}

impl AuditWorker {
    pub fn new(
        receiver: Receiver<AuditEvent>,
        db_pool: SqlitePool,
        batch_size: usize,
        flush_interval_ms: u64,
    ) -> Self {
        Self {
            receiver,
            db_pool,
            batch_size,
            flush_interval: Duration::from_millis(flush_interval_ms),
        }
    }

    /// 后台主循环
    pub async fn run(mut self) {
        tracing::info!("Audit batch worker started");
        let mut buffer = Vec::with_capacity(self.batch_size);
        let mut ticker = interval(self.flush_interval);

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    if !buffer.is_empty() {
                        self.flush_batch(&mut buffer).await;
                    }
                }
                maybe_event = self.receiver.recv() => {
                    match maybe_event {
                        Some(event) => {
                            buffer.push(event);
                            if buffer.len() >= self.batch_size {
                                self.flush_batch(&mut buffer).await;
                            }
                        }
                        None => {
                            // 通道关闭，排空剩余缓冲并安全退出
                            if !buffer.is_empty() {
                                self.flush_batch(&mut buffer).await;
                            }
                            tracing::info!("Audit worker exited safely after draining remaining events");
                            break;
                        }
                    }
                }
            }
        }
    }

    /// 批量事务写入 SQLite WAL
    async fn flush_batch(&self, buffer: &mut Vec<AuditEvent>) {
        if buffer.is_empty() {
            return;
        }

        let count = buffer.len();
        let mut tx = match self.db_pool.begin().await {
            Ok(tx) => tx,
            Err(err) => {
                tracing::error!("Failed to begin transaction for audit batch: {}", err);
                return;
            }
        };

        for event in buffer.drain(..) {
            // 1. 插入 audit_logs 明细
            let is_stream_int = if event.is_stream { 1i64 } else { 0i64 };
            let insert_res = sqlx::query(
                r#"
                INSERT INTO audit_logs (
                    id, trace_id, key_id, user_id, model, backend_id, protocol,
                    client_ip, http_status, prompt_tokens, completion_tokens,
                    total_tokens, request_duration_ms, ttft_ms, is_stream,
                    retry_count, failover_path, error_message, created_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)
                "#,
            )
            .bind(&event.id)
            .bind(&event.trace_id)
            .bind(&event.key_id)
            .bind(&event.user_id)
            .bind(&event.model)
            .bind(&event.backend_id)
            .bind(&event.protocol_str)
            .bind(&event.client_ip)
            .bind(event.http_status as i64)
            .bind(event.usage.prompt_tokens as i64)
            .bind(event.usage.completion_tokens as i64)
            .bind(event.usage.total_tokens as i64)
            .bind(event.duration_ms as i64)
            .bind(event.ttft_ms.map(|t| t as i64))
            .bind(is_stream_int)
            .bind(event.retry_count as i64)
            .bind(&event.failover_path)
            .bind(&event.error_message)
            .bind(event.created_at_timestamp)
            .execute(&mut *tx)
            .await;

            if let Err(err) = insert_res {
                tracing::error!("Failed to insert audit log entry: {}", err);
                continue;
            }

            // 2. 如果存在真实有效 virtual_key，同步原子更新 quota_usages
            if !event.key_id.is_empty() {
                let key_exists: bool =
                    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM virtual_keys WHERE id = ?)")
                        .bind(&event.key_id)
                        .fetch_one(&mut *tx)
                        .await
                        .unwrap_or(false);

                if key_exists {
                    let dt = Utc
                        .timestamp_millis_opt(event.created_at_timestamp)
                        .single()
                        .unwrap_or_else(Utc::now);
                    let daily_period = dt.format("%Y-%m-%d").to_string();
                    let weekly_period = format!("{}-W{:02}", dt.year(), dt.iso_week().week());

                    // 更新日配额使用
                    let daily_id = format!("{}:daily:{}", event.key_id, daily_period);
                    let _ = sqlx::query(
                        r#"
                        INSERT INTO quota_usages (
                            id, key_id, period_type, period_start,
                            prompt_tokens, completion_tokens, total_tokens, request_count, updated_at
                        ) VALUES (?1, ?2, 'daily', ?3, ?4, ?5, ?6, 1, ?7)
                        ON CONFLICT(key_id, period_type, period_start) DO UPDATE SET
                            prompt_tokens = prompt_tokens + excluded.prompt_tokens,
                            completion_tokens = completion_tokens + excluded.completion_tokens,
                            total_tokens = total_tokens + excluded.total_tokens,
                            request_count = request_count + 1,
                            updated_at = excluded.updated_at
                        "#,
                    )
                    .bind(daily_id)
                    .bind(&event.key_id)
                    .bind(daily_period)
                    .bind(event.usage.prompt_tokens as i64)
                    .bind(event.usage.completion_tokens as i64)
                    .bind(event.usage.total_tokens as i64)
                    .bind(event.created_at_timestamp)
                    .execute(&mut *tx)
                    .await;

                    // 更新周配额使用
                    let weekly_id = format!("{}:weekly:{}", event.key_id, weekly_period);
                    let _ = sqlx::query(
                        r#"
                        INSERT INTO quota_usages (
                            id, key_id, period_type, period_start,
                            prompt_tokens, completion_tokens, total_tokens, request_count, updated_at
                        ) VALUES (?1, ?2, 'weekly', ?3, ?4, ?5, ?6, 1, ?7)
                        ON CONFLICT(key_id, period_type, period_start) DO UPDATE SET
                            prompt_tokens = prompt_tokens + excluded.prompt_tokens,
                            completion_tokens = completion_tokens + excluded.completion_tokens,
                            total_tokens = total_tokens + excluded.total_tokens,
                            request_count = request_count + 1,
                            updated_at = excluded.updated_at
                        "#,
                    )
                    .bind(weekly_id)
                    .bind(&event.key_id)
                    .bind(weekly_period)
                    .bind(event.usage.prompt_tokens as i64)
                    .bind(event.usage.completion_tokens as i64)
                    .bind(event.usage.total_tokens as i64)
                    .bind(event.created_at_timestamp)
                    .execute(&mut *tx)
                    .await;
                }
            }
        }

        if let Err(err) = tx.commit().await {
            tracing::error!("Failed to commit audit batch of {} records: {}", count, err);
        }
    }
}
