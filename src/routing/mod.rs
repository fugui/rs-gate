pub mod backend;
pub mod scheduler;

use arc_swap::ArcSwap;
use backend::BackendInstance;
use std::collections::HashMap;
use std::sync::Arc;

/// 全局并发路由表管理 (Zero-Lock Read via ArcSwap)
pub struct RouterTable {
    routes: ArcSwap<HashMap<String, Vec<BackendInstance>>>,
}

impl Default for RouterTable {
    fn default() -> Self {
        Self::new()
    }
}

impl RouterTable {
    pub fn new() -> Self {
        Self {
            routes: ArcSwap::from_pointee(HashMap::new()),
        }
    }

    /// 获取指定模型的候选后端列表
    pub fn get_backends(&self, model: &str) -> Option<Vec<BackendInstance>> {
        let guard = self.routes.load();
        if let Some(backends) = guard.get(model) {
            return Some(backends.clone());
        }
        // 尝试通配符兜底
        guard.get("*").cloned()
    }

    /// 设置全局路由映射表
    pub fn set_routes(&self, new_routes: HashMap<String, Vec<BackendInstance>>) {
        self.routes.store(Arc::new(new_routes));
    }

    /// 获取所有支持的逻辑模型列表
    pub fn get_all_models(&self) -> Vec<String> {
        let guard = self.routes.load();
        guard.keys().filter(|&k| k != "*").cloned().collect()
    }

    /// 为指定模型注册后端节点
    pub fn register_backend(&self, model: impl Into<String>, backend: BackendInstance) {
        let model = model.into();
        let current = self.routes.load();
        let mut updated = (**current).clone();
        updated.entry(model).or_default().push(backend);
        self.routes.store(Arc::new(updated));
    }
}
