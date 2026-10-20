use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// 虚拟 Key 瞬时在途长连接并发门限守卫 (RAII)
#[derive(Debug)]
pub struct KeyConcurrencyGuard {
    counter: Arc<AtomicUsize>,
}

impl KeyConcurrencyGuard {
    /// 尝试申领并发连接槽位
    pub fn try_acquire(
        counter: Arc<AtomicUsize>,
        max_concurrency: usize,
    ) -> Result<Self, ConcurrencyExceededError> {
        if max_concurrency == 0 {
            // 0 表示不限制在途并发
            return Ok(Self { counter });
        }

        let mut current = counter.load(Ordering::Relaxed);
        loop {
            if current >= max_concurrency {
                return Err(ConcurrencyExceededError {
                    limit: max_concurrency,
                });
            }

            match counter.compare_exchange_weak(
                current,
                current + 1,
                Ordering::SeqCst,
                Ordering::Relaxed,
            ) {
                Ok(_) => return Ok(Self { counter }),
                Err(actual) => current = actual,
            }
        }
    }
}

impl Drop for KeyConcurrencyGuard {
    fn drop(&mut self) {
        self.counter.fetch_sub(1, Ordering::SeqCst);
    }
}

#[derive(Debug, thiserror::Error)]
#[error("Key concurrency limit of {limit} exceeded")]
pub struct ConcurrencyExceededError {
    pub limit: usize,
}
