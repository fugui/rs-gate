use crate::audit::AuditLogger;
use crate::auth::ip::IpWhitelistMatcher;
use crate::auth::key::{hash_virtual_key, KeyStore, VirtualKeyRecord, DEFAULT_KEY_SALT};
use crate::limiting::RateLimiterManager;
use crate::proxy::{client::ProxyClient, handler::ProxyState};
use crate::routing::backend::BackendInstance;
use crate::routing::RouterTable;
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Argon2,
};
use axum::{http::StatusCode, routing::get, Json, Router};
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

#[derive(serde::Serialize)]
struct ReadyHealthResponse {
    status: &'static str,
    database: &'static str,
    active_models: usize,
    healthy_backends: usize,
}

async fn health_ready_handler(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> (StatusCode, Json<ReadyHealthResponse>) {
    let db_status = if let Some(ref p) = state.pool {
        if sqlx::query("SELECT 1;").execute(p).await.is_ok() {
            "connected"
        } else {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ReadyHealthResponse {
                    status: "unavailable",
                    database: "disconnected",
                    active_models: 0,
                    healthy_backends: 0,
                }),
            );
        }
    } else {
        "memory"
    };

    let models = state.router_table.get_all_models().len();

    (
        StatusCode::OK,
        Json(ReadyHealthResponse {
            status: "ready",
            database: db_status,
            active_models: models,
            healthy_backends: 1,
        }),
    )
}

async fn metrics_handler() -> (
    StatusCode,
    [(axum::http::HeaderName, &'static str); 1],
    String,
) {
    (
        StatusCode::OK,
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        crate::metrics::export_metrics_text(),
    )
}

/// 组装 Axum 应用路由与全局中间件状态
pub async fn build_app(pool: Option<SqlitePool>) -> Result<Router, anyhow::Error> {
    let router_table = Arc::new(RouterTable::new());
    let proxy_client = ProxyClient::new();
    let key_store = Arc::new(KeyStore::new(DEFAULT_KEY_SALT));
    let rate_limiter = Arc::new(RateLimiterManager::new());

    let audit_logger = if let Some(ref p) = pool {
        let _ = key_store.load_from_db(p).await;
        let _ = crate::admin::admin_api::reload_router_table(p, &router_table).await;
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
        .route("/health/ready", get(health_ready_handler))
        .route("/metrics", get(metrics_handler))
        .with_state(app_state.clone());

    let admin_router = crate::admin::build_admin_router(app_state);
    let proxy_router = crate::proxy::build_proxy_router(proxy_state);

    let app = health_router
        .merge(admin_router)
        .merge(proxy_router)
        .fallback(crate::admin::embedded::static_handler);

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

    let proxy_client = ProxyClient::new();
    let rate_limiter = Arc::new(RateLimiterManager::new());

    let state = ProxyState {
        router_table: router_table.clone(),
        proxy_client: proxy_client.clone(),
        key_store: Some(key_store.clone()),
        rate_limiter: Some(rate_limiter.clone()),
        audit_logger: None,
    };

    let app_state = AppState {
        pool: None,
        router_table,
        proxy_client,
        key_store: Some(key_store),
        rate_limiter: Some(rate_limiter),
        audit_logger: None,
    };

    let health_router = Router::new()
        .route("/health/live", get(health_live_handler))
        .route("/health/ready", get(health_ready_handler))
        .route("/metrics", get(metrics_handler))
        .with_state(app_state);

    let proxy_router = crate::proxy::build_proxy_router(state);
    health_router
        .merge(proxy_router)
        .fallback(crate::admin::embedded::static_handler)
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
    let proxy_client = ProxyClient::new();

    let state = ProxyState {
        router_table: router_table.clone(),
        proxy_client: proxy_client.clone(),
        key_store: Some(key_store.clone()),
        rate_limiter: Some(rate_limiter.clone()),
        audit_logger: None,
    };

    let app_state = AppState {
        pool: None,
        router_table,
        proxy_client,
        key_store: Some(key_store),
        rate_limiter: Some(rate_limiter),
        audit_logger: None,
    };

    let health_router = Router::new()
        .route("/health/live", get(health_live_handler))
        .route("/health/ready", get(health_ready_handler))
        .route("/metrics", get(metrics_handler))
        .with_state(app_state);

    let proxy_router = crate::proxy::build_proxy_router(state);
    health_router
        .merge(proxy_router)
        .fallback(crate::admin::embedded::static_handler)
}

/// 快速构建管理控制台与自服务门户的集成测试应用实例 (挂载完整数据库与初始数据)
pub async fn build_test_admin_app() -> Router {
    let db_path = std::env::temp_dir().join(format!("test_admin_{}.db", uuid::Uuid::new_v4()));
    let conn_str = format!("sqlite://{}?mode=rwc", db_path.display());

    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&conn_str)
        .await
        .expect("Failed to connect SQLite");

    crate::storage::run_migrations(&pool)
        .await
        .expect("Migrations failed");

    let now_ms = chrono::Utc::now().timestamp_millis();
    let argon2 = Argon2::default();
    let salt = SaltString::generate(&mut OsRng);
    let pass_hash = argon2
        .hash_password(b"AdminPass123!", &salt)
        .unwrap()
        .to_string();

    // 1. Seed users
    sqlx::query(
        r#"
        INSERT INTO users (id, username, password_hash, role, display_name, email, is_active, created_at, updated_at)
        VALUES 
            ('usr-admin', 'admin_user', ?, 'admin', 'System Admin', 'admin@example.com', 1, ?, ?),
            ('usr-1', 'user_1', ?, 'user', 'Dev One', 'usr1@example.com', 1, ?, ?),
            ('usr-a', 'user_a', ?, 'user', 'Dev A', 'usra@example.com', 1, ?, ?),
            ('usr-b', 'user_b', ?, 'user', 'Dev B', 'usrb@example.com', 1, ?, ?);
        "#,
    )
    .bind(&pass_hash)
    .bind(now_ms)
    .bind(now_ms)
    .bind(&pass_hash)
    .bind(now_ms)
    .bind(now_ms)
    .bind(&pass_hash)
    .bind(now_ms)
    .bind(now_ms)
    .bind(&pass_hash)
    .bind(now_ms)
    .bind(now_ms)
    .execute(&pool)
    .await
    .unwrap();

    // 2. Seed keys
    let key001_hash = hash_virtual_key("sk-gate-key001", DEFAULT_KEY_SALT);
    let keyb_hash = hash_virtual_key("sk-gate-keyb", DEFAULT_KEY_SALT);

    sqlx::query(
        r#"
        INSERT INTO virtual_keys (id, user_id, name, key_hash, key_prefix, allowed_models, rpm_limit, max_concurrency, daily_token_quota, weekly_token_quota, is_active, created_at, updated_at)
        VALUES
            ('key-001', 'usr-a', 'Key 001', ?, 'sk-gate-001***', '["*"]', 60, 10, 1000000, 5000000, 1, ?, ?),
            ('key-002', 'usr-b', 'Key B', ?, 'sk-gate-b00***', '["*"]', 60, 10, 1000000, 5000000, 1, ?, ?);
        "#,
    )
    .bind(&key001_hash)
    .bind(now_ms)
    .bind(now_ms)
    .bind(&keyb_hash)
    .bind(now_ms)
    .bind(now_ms)
    .execute(&pool)
    .await
    .unwrap();

    // 3. Seed quota_usages for key-001
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    sqlx::query(
        r#"
        INSERT INTO quota_usages (id, key_id, period_type, period_start, prompt_tokens, completion_tokens, total_tokens, request_count, updated_at)
        VALUES ('qu-001', 'key-001', 'daily', ?, 500, 500, 1000, 2, ?);
        "#,
    )
    .bind(&today)
    .bind(now_ms)
    .execute(&pool)
    .await
    .unwrap();

    // 4. Seed backend and model
    sqlx::query(
        r#"
        INSERT INTO backend_servers (id, name, base_url, weight, capabilities_mask, health_status, consecutive_failures, is_active, created_at, updated_at)
        VALUES ('be-001', 'GPU-Cluster-Node-01', 'http://127.0.0.1:8000', 10, 3, 'healthy', 0, 1, ?, ?);
        "#,
    )
    .bind(now_ms)
    .bind(now_ms)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        r#"
        INSERT INTO models (id, display_name, description, max_retries, retry_interval_ms, is_active, created_at, updated_at)
        VALUES 
            ('deepseek-r1', 'DeepSeek R1', '企业级推理模型', 2, 500, 1, ?, ?),
            ('qwen-2.5-72b', 'Qwen 2.5 72B', '通用大模型', 2, 500, 1, ?, ?);
        "#,
    )
    .bind(now_ms)
    .bind(now_ms)
    .bind(now_ms)
    .bind(now_ms)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        r#"
        INSERT INTO model_backends (id, model_id, backend_id, weight_override, priority, is_active, created_at)
        VALUES ('mb-001', 'deepseek-r1', 'be-001', 10, 1, 1, ?);
        "#,
    )
    .bind(now_ms)
    .execute(&pool)
    .await
    .unwrap();

    // 5. Seed audit log
    sqlx::query(
        r#"
        INSERT INTO audit_logs (id, trace_id, key_id, user_id, model, backend_id, protocol, client_ip, http_status, prompt_tokens, completion_tokens, total_tokens, request_duration_ms, is_stream, retry_count, created_at)
        VALUES ('aud-001', 'trace-001', 'key-001', 'usr-a', 'deepseek-r1', 'be-001', 'chat', '127.0.0.1', 200, 100, 200, 300, 120, 0, 0, ?);
        "#,
    )
    .bind(now_ms)
    .execute(&pool)
    .await
    .unwrap();

    build_app(Some(pool)).await.expect("Failed to build app")
}
