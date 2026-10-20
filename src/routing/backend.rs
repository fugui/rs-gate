use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::Arc;

/// 物理后端节点实例定义
#[derive(Debug, Clone)]
pub struct BackendInstance {
    pub id: String,
    pub base_url: String,
    pub weight: u32,
    pub upstream_api_key: Option<String>,
    pub active_connections: Arc<AtomicUsize>,
    pub is_healthy: Arc<AtomicBool>,
}

impl BackendInstance {
    pub fn new(id: impl Into<String>, base_url: impl Into<String>, weight: u32) -> Self {
        Self {
            id: id.into(),
            base_url: base_url.into(),
            weight: weight.max(1),
            upstream_api_key: None,
            active_connections: Arc::new(AtomicUsize::new(0)),
            is_healthy: Arc::new(AtomicBool::new(true)),
        }
    }

    pub fn with_api_key(mut self, key: impl Into<String>) -> Self {
        self.upstream_api_key = Some(key.into());
        self
    }

    pub fn with_health(self, healthy: bool) -> Self {
        self.is_healthy
            .store(healthy, std::sync::atomic::Ordering::SeqCst);
        self
    }
}
