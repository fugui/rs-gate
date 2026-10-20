use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tracing::trace;

/// 活跃长连接 RAII 生命周期守卫 (保证连接计数永不泄漏)
#[derive(Debug)]
pub struct ActiveConnectionGuard {
    counter: Arc<AtomicUsize>,
    backend_id: Option<String>,
}

impl ActiveConnectionGuard {
    /// 包装已有活跃计数 (不重复递增)
    pub fn new(counter: Arc<AtomicUsize>) -> Self {
        crate::metrics::inc_active_connections();
        Self {
            counter,
            backend_id: None,
        }
    }

    /// 原子申领并递增活跃连接计数
    pub fn acquire(counter: Arc<AtomicUsize>) -> Self {
        let prev = counter.fetch_add(1, Ordering::SeqCst);
        trace!(active_conns = prev + 1, "Active connection acquired");
        crate::metrics::inc_active_connections();
        Self {
            counter,
            backend_id: None,
        }
    }

    /// 绑定后端实例标识
    pub fn with_backend_id(mut self, backend_id: impl Into<String>) -> Self {
        self.backend_id = Some(backend_id.into());
        self
    }

    pub fn backend_id(&self) -> Option<&str> {
        self.backend_id.as_deref()
    }
}

impl Drop for ActiveConnectionGuard {
    fn drop(&mut self) {
        let prev = self.counter.fetch_sub(1, Ordering::SeqCst);
        trace!(
            backend_id = self.backend_id.as_deref().unwrap_or("unknown"),
            active_conns = prev.saturating_sub(1),
            "Active connection released via RAII Drop"
        );
        crate::metrics::dec_active_connections();
    }
}
