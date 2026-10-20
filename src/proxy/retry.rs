use axum::http::StatusCode;
use rand::Rng;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// 最大静默重试次数 (默认 2)
    pub max_retries: usize,
    /// 基础退避时间 (默认 500ms)
    pub base_interval_ms: u64,
    /// 最大退避时间 (默认 5000ms)
    pub max_interval_ms: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 2,
            base_interval_ms: 500,
            max_interval_ms: 5000,
        }
    }
}

/// 判定上游 HTTP 状态码是否属于可静默重试类型 (5xx 或 429)
pub fn is_retryable_status(status: StatusCode) -> bool {
    status.is_server_error() || status == StatusCode::TOO_MANY_REQUESTS
}

/// 计算自适应退避等待时间 (包含 Retry-After 优先与 Full Jitter)
pub fn calculate_backoff(
    attempt: usize,
    response: Option<&reqwest::Response>,
    config: &RetryConfig,
) -> Duration {
    // 优先检查上游是否携带合法 Retry-After 响应头
    if let Some(resp) = response {
        if let Some(retry_after) = resp.headers().get(reqwest::header::RETRY_AFTER) {
            if let Ok(str_val) = retry_after.to_str() {
                if let Ok(secs) = str_val.parse::<u64>() {
                    let capped_secs = secs.min(10);
                    return Duration::from_secs(capped_secs);
                }
            }
        }
    }

    // Full Jitter: Uniform(0, min(max_interval, base * 2^(attempt - 1)))
    let exp_factor = 1u64
        .checked_shl((attempt.saturating_sub(1)) as u32)
        .unwrap_or(16);
    let cap = (config.base_interval_ms * exp_factor).min(config.max_interval_ms);
    let mut rng = rand::thread_rng();
    let jitter_ms = rng.gen_range(0..=cap);

    Duration::from_millis(jitter_ms)
}
