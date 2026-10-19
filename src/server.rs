use axum::{routing::get, Json, Router};
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct AppState {
    pub pool: Option<SqlitePool>,
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
    let state = AppState { pool };
    let app = Router::new()
        .route("/health/live", get(health_live_handler))
        .with_state(state);

    Ok(app)
}
