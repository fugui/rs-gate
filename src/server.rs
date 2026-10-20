use crate::audit::AuditLogger;
use crate::auth::ip::IpWhitelistMatcher;
use crate::auth::key::{hash_virtual_key, KeyStore, VirtualKeyRecord, DEFAULT_KEY_SALT};
use crate::limiting::RateLimiterManager;
use crate::proxy::{client::ProxyClient, handler::ProxyState};
use crate::routing::backend::BackendInstance;
use crate::routing::RouterTable;
use axum::{routing::get, Json, Router};
use sqlx::SqlitePool;
use std::sync::Arc;

pub const TEST_VALID_KEY: &str = "sk-gate-validkey00000000000000000000";

#[derive(Clone)]
pub struct AppState {
    pub pool: Option<SqlitePool>,
    pub router_table: Arc<RouterTable>,
    pub proxy_client: ProxyClient,
    pub key_store: Option<Arc<KeyStore>>,
    pub rate_limiter: Option<Arc<RateLimiterManager>>,
    pub audit_logger: Option<AuditLogger>,
}

#[derive(serde::Serialize)]
struct LiveHealthResponse {
    status: &'static str,
    timestamp: i64,
}

async fn health_live_handler() -> Json<LiveHealthResponse> {
    Json(LiveHealthResponse {
        status: "alive",
        timestamp: chrono::Utc::now().timestamp_millis(),
    })
}

/// 组装 Axum 应用路由与全局中间件状态
pub async fn build_app(pool: Option<SqlitePool>) -> Result<Router, anyhow::Error> {
    let router_table = Arc::new(RouterTable::new());
    let proxy_client = ProxyClient::new();
    let key_store = Arc::new(KeyStore::new(DEFAULT_KEY_SALT));
    let rate_limiter = Arc::new(RateLimiterManager::new());

    let audit_logger = if let Some(ref p) = pool {
        let _ = key_store.load_from_db(p).await;
        let (logger, worker) = AuditLogger::new_test(p.clone(), 100);
        tokio::spawn(worker.run());
        Some(logger)
    } else {
        None
    };

    let proxy_state = ProxyState {
        router_table: router_table.clone(),
        proxy_client: proxy_client.clone(),
        key_store: Some(key_store.clone()),
        rate_limiter: Some(rate_limiter.clone()),
        audit_logger: audit_logger.clone(),
    };

    let app_state = AppState {
        pool,
        router_table,
        proxy_client,
        key_store: Some(key_store),
        rate_limiter: Some(rate_limiter),
        audit_logger,
    };

    let health_router = Router::new()
        .route("/health/live", get(health_live_handler))
        .with_state(app_state);

    let proxy_router = crate::proxy::build_proxy_router(proxy_state);

    let app = health_router.merge(proxy_router);

    Ok(app)
}

/// 快速构建集成测试应用实例 (默认挂载标准测试 Key 与无限制白名单)
pub async fn build_test_app() -> Router {
    build_test_app_with_custom_key(None, vec!["*".to_string()], 1000).await
}

/// 构建携带指定 CIDR IP 白名单的集成测试应用
pub async fn build_test_app_with_ip_whitelist(cidr: &str) -> Router {
    let matcher = IpWhitelistMatcher::parse(cidr).ok();
    build_test_app_with_custom_key(matcher, vec!["*".to_string()], 1000).await
}

/// 构建携带限定模型权限集的集成测试应用
pub async fn build_test_app_with_allowed_models(models: Vec<String>) -> Router {
    build_test_app_with_custom_key(None, models, 1000).await
}

/// 构建携带严格 RPM 限制的集成测试应用
pub async fn build_test_app_with_rpm_limit(rpm: u32) -> Router {
    build_test_app_with_custom_key(None, vec!["*".to_string()], rpm).await
}

/// 快速构建带有后端多节点集群的集成测试应用
pub async fn build_test_app_with_cluster(cluster_uris: Vec<String>) -> Router {
    let router_table = Arc::new(RouterTable::new());
    for (idx, uri) in cluster_uris.iter().enumerate() {
        let weight = (100usize.saturating_sub(idx)).max(1) as u32;
        let backend = BackendInstance::new(format!("node-{}", idx), uri, weight);
        router_table.register_backend("*", backend.clone());
        router_table.register_backend("deepseek-r1", backend);
    }

    let key_store = Arc::new(KeyStore::new(DEFAULT_KEY_SALT));
    let valid_record = VirtualKeyRecord {
        id: "test-key-valid".to_string(),
        user_id: "test-user-1".to_string(),
        name: "Test Valid Key".to_string(),
        key_hash: hash_virtual_key(TEST_VALID_KEY, DEFAULT_KEY_SALT),
        key_prefix: "sk-gate-valid".to_string(),
        allowed_models: vec!["*".to_string()],
        rpm_limit: 10_000,
        max_concurrency: 100,
        daily_token_quota: 0,
        weekly_token_quota: 0,
        ip_whitelist: None,
        is_active: true,
        expires_at: None,
    };
    key_store.insert(valid_record);

    let state = ProxyState {
        router_table,
        proxy_client: ProxyClient::new(),
        key_store: Some(key_store),
        rate_limiter: Some(Arc::new(RateLimiterManager::new())),
        audit_logger: None,
    };

    crate::proxy::build_proxy_router(state)
}

async fn build_test_app_with_custom_key(
    ip_whitelist: Option<IpWhitelistMatcher>,
    allowed_models: Vec<String>,
    rpm: u32,
) -> Router {
    let key_store = Arc::new(KeyStore::new(DEFAULT_KEY_SALT));
    let valid_record = VirtualKeyRecord {
        id: "test-key-valid".to_string(),
        user_id: "test-user-1".to_string(),
        name: "Test Valid Key".to_string(),
        key_hash: hash_virtual_key(TEST_VALID_KEY, DEFAULT_KEY_SALT),
        key_prefix: "sk-gate-valid".to_string(),
        allowed_models,
        rpm_limit: rpm,
        max_concurrency: 10,
        daily_token_quota: 0,
        weekly_token_quota: 0,
        ip_whitelist,
        is_active: true,
        expires_at: None,
    };
    key_store.insert(valid_record);

    let router_table = Arc::new(RouterTable::new());
    let rate_limiter = Arc::new(RateLimiterManager::new());

    let state = ProxyState {
        router_table,
        proxy_client: ProxyClient::new(),
        key_store: Some(key_store),
        rate_limiter: Some(rate_limiter),
        audit_logger: None,
    };

    crate::proxy::build_proxy_router(state)
}
