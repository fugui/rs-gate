use crate::proxy::{client::ProxyClient, handler::ProxyState};
use crate::routing::RouterTable;
use axum::{routing::get, Json, Router};
use sqlx::SqlitePool;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub pool: Option<SqlitePool>,
    pub router_table: Arc<RouterTable>,
    pub proxy_client: ProxyClient,
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
    let proxy_state = ProxyState {
        router_table: router_table.clone(),
        proxy_client: proxy_client.clone(),
    };

    let app_state = AppState {
        pool,
        router_table,
        proxy_client,
    };

    let health_router = Router::new()
        .route("/health/live", get(health_live_handler))
        .with_state(app_state);

    let proxy_router = crate::proxy::build_proxy_router(proxy_state);

    let app = health_router.merge(proxy_router);

    Ok(app)
}
