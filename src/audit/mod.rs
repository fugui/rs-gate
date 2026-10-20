pub mod worker;

use crate::proxy::stream::TokenUsage;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use worker::AuditWorker;

/// 全链路结构化审计事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub id: String,
    pub trace_id: String,
    pub key_id: String,
    pub user_id: Option<String>,
    pub model: String,
    pub backend_id: String,
    pub protocol_str: String,
    pub client_ip: String,
    pub http_status: u16,
    pub usage: TokenUsage,
    pub duration_ms: u64,
    pub ttft_ms: Option<u64>,
    pub is_stream: bool,
    pub retry_count: u32,
    pub failover_path: Option<String>,
    pub error_message: Option<String>,
    pub created_at_timestamp: i64,
}

impl AuditEvent {
    pub fn dummy(i: u64) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            trace_id: format!("trace-dummy-{}", i),
            key_id: "test-key-0".to_string(),
            user_id: Some("user-0".to_string()),
            model: "deepseek-r1".to_string(),
            backend_id: "mock-backend".to_string(),
            protocol_str: "chat".to_string(),
            client_ip: "127.0.0.1".to_string(),
            http_status: 200,
            usage: TokenUsage {
                prompt_tokens: 10,
                completion_tokens: 20,
                total_tokens: 30,
            },
            duration_ms: 100,
            ttft_ms: Some(25),
            is_stream: false,
            retry_count: 0,
            failover_path: None,
            error_message: None,
            created_at_timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }
}

/// 审计日志投递器，基于 Tokio MPSC 有界通道解耦
#[derive(Clone)]
pub struct AuditLogger {
    sender: Sender<AuditEvent>,
}

#[derive(Debug, thiserror::Error)]
pub enum AuditQueueError {
    #[error("Audit queue is full, event dropped")]
    QueueFull,
    #[error("Audit channel is closed")]
    ChannelClosed,
}

impl AuditLogger {
    pub fn new(capacity: usize) -> (Self, Receiver<AuditEvent>) {
        let (tx, rx) = channel(capacity);
        (Self { sender: tx }, rx)
    }

    pub fn new_test(pool: SqlitePool, batch_size: usize) -> (Self, AuditWorker) {
        let (tx, rx) = channel(10_000);
        let worker = AuditWorker::new(rx, pool, batch_size, 500);
        (Self { sender: tx }, worker)
    }

    /// 代理热路径纳秒级非阻塞投递
    pub fn try_send(&self, event: AuditEvent) -> Result<(), AuditQueueError> {
        self.sender.try_send(event).map_err(|e| match e {
            tokio::sync::mpsc::error::TrySendError::Full(_) => AuditQueueError::QueueFull,
            tokio::sync::mpsc::error::TrySendError::Closed(_) => AuditQueueError::ChannelClosed,
        })
    }

    /// 异步投递
    pub async fn send(&self, event: AuditEvent) -> Result<(), AuditQueueError> {
        self.sender
            .send(event)
            .await
            .map_err(|_| AuditQueueError::ChannelClosed)
    }
}
