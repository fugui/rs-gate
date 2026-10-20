use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// 基于 AtomicU64 CAS 的高性能无锁令牌桶 (RPM)
/// 高 32 位存储最近刷新时间戳 (ms)，低 32 位存储定点缩放令牌数 (1 token = 1000 scaled)
pub struct AtomicTokenBucket {
    state: AtomicU64,
    rpm: u32,
    burst_capacity: u32,
    base_instant: Instant,
}

impl AtomicTokenBucket {
    pub fn new(rpm: u32, burst_capacity: u32) -> Self {
        let base_instant = Instant::now();
        let initial_tokens = (burst_capacity as u64) * 1000;
        let initial_state = initial_tokens & 0xFFFF_FFFF;

        Self {
            state: AtomicU64::new(initial_state),
            rpm,
            burst_capacity,
            base_instant,
        }
    }

    /// 尝试原子扣减指定数量的令牌
    pub fn try_acquire(&self, tokens: u32) -> bool {
        if self.rpm == 0 {
            return true;
        }

        let needed_scaled = (tokens as u64) * 1000;
        let capacity_scaled = (self.burst_capacity as u64) * 1000;
        let fill_rate_per_ms = (self.rpm as f64) / 60.0;

        loop {
            let now_ms = (self.base_instant.elapsed().as_millis() as u64) & 0xFFFF_FFFF;
            let current = self.state.load(Ordering::Relaxed);
            let last_ms = current >> 32;
            let current_tokens = current & 0xFFFF_FFFF;

            let delta_ms = now_ms.saturating_sub(last_ms);

            let replenished = ((delta_ms as f64) * fill_rate_per_ms) as u64;
            let total_available = (current_tokens + replenished).min(capacity_scaled);

            if total_available < needed_scaled {
                return false;
            }

            let next_tokens = total_available - needed_scaled;
            let next_state = (now_ms << 32) | (next_tokens & 0xFFFF_FFFF);

            if self
                .state
                .compare_exchange_weak(current, next_state, Ordering::SeqCst, Ordering::Relaxed)
                .is_ok()
            {
                return true;
            }
        }
    }
}
