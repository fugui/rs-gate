pub mod concurrency;
pub mod quota;
pub mod token_bucket;

use concurrency::KeyConcurrencyGuard;
use quota::QuotaEngine;
use std::collections::HashMap;
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, RwLock};
use token_bucket::AtomicTokenBucket;

#[derive(Debug, thiserror::Error)]
pub enum GatewayRateLimitError {
    #[error("RPM limit exceeded")]
    RpmLimitExceeded,
    #[error("Key concurrency limit of {0} exceeded")]
    ConcurrencyLimitExceeded(usize),
    #[error("Token quota exceeded: {0}")]
    QuotaExceeded(#[from] quota::QuotaExceededError),
}

/// 统合管理虚拟 Key 的令牌桶、并发与配额门限
pub struct RateLimiterManager {
    buckets: RwLock<HashMap<String, Arc<AtomicTokenBucket>>>,
    concurrency_counters: RwLock<HashMap<String, Arc<AtomicUsize>>>,
    pub quota_engine: QuotaEngine,
}

impl Default for RateLimiterManager {
    fn default() -> Self {
        Self::new()
    }
}

impl RateLimiterManager {
    pub fn new() -> Self {
        Self {
            buckets: RwLock::new(HashMap::new()),
            concurrency_counters: RwLock::new(HashMap::new()),
            quota_engine: QuotaEngine::new(),
        }
    }

    /// 获取或创建指定虚拟 Key 的无锁令牌桶
    pub fn get_or_create_bucket(&self, key_id: &str, rpm: u32) -> Arc<AtomicTokenBucket> {
        let read_guard = self.buckets.read().unwrap_or_else(|e| e.into_inner());
        if let Some(bucket) = read_guard.get(key_id) {
            return bucket.clone();
        }
        drop(read_guard);

        let mut write_guard = self.buckets.write().unwrap_or_else(|e| e.into_inner());
        write_guard
            .entry(key_id.to_string())
            .or_insert_with(|| Arc::new(AtomicTokenBucket::new(rpm, rpm.max(1))))
            .clone()
    }

    /// 获取或创建指定虚拟 Key 的在途长连接计数器
    pub fn get_or_create_concurrency_counter(&self, key_id: &str) -> Arc<AtomicUsize> {
        let read_guard = self
            .concurrency_counters
            .read()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(counter) = read_guard.get(key_id) {
            return counter.clone();
        }
        drop(read_guard);

        let mut write_guard = self
            .concurrency_counters
            .write()
            .unwrap_or_else(|e| e.into_inner());
        write_guard
            .entry(key_id.to_string())
            .or_insert_with(|| Arc::new(AtomicUsize::new(0)))
            .clone()
    }

    /// 统一执行准入控制：RPM 令牌桶扣减 + 在途并发占位 + 日/周硬配额预检 (流式 500 Token 预留)
    pub fn acquire_admission(
        &self,
        key_id: &str,
        rpm: u32,
        max_concurrency: usize,
        reserved_tokens: u64,
    ) -> Result<KeyConcurrencyGuard, GatewayRateLimitError> {
        // 1. 无锁令牌桶 RPM 校验
        let bucket = self.get_or_create_bucket(key_id, rpm);
        if !bucket.try_acquire(1) {
            return Err(GatewayRateLimitError::RpmLimitExceeded);
        }

        // 2. 并发长连接门限校验
        let counter = self.get_or_create_concurrency_counter(key_id);
        let guard = KeyConcurrencyGuard::try_acquire(counter, max_concurrency)
            .map_err(|e| GatewayRateLimitError::ConcurrencyLimitExceeded(e.limit))?;

        // 3. 日/周硬配额预检
        self.quota_engine.check_admission(key_id, reserved_tokens)?;

        Ok(guard)
    }
}
