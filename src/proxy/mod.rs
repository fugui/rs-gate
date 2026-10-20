pub mod client;
pub mod guard;
pub mod handler;
pub mod retry;
pub mod stream;

use crate::routing::{backend::BackendInstance, RouterTable};
use axum::{
    routing::{get, post},
    Router,
};
use client::ProxyClient;
use handler::ProxyState;
use std::sync::Arc;

/// 组装 6 大原生直通代理路由
pub fn build_proxy_router(state: ProxyState) -> Router {
    Router::new()
        .route(
            "/v1/chat/completions",
            post(handler::chat_completions_handler),
        )
        .route("/v1/responses", post(handler::responses_handler))
        .route("/v1/completions", post(handler::completions_handler))
        .route("/v1/embeddings", post(handler::embeddings_handler))
        .route("/v1/rerank", post(handler::rerank_handler))
        .route("/v1/models", get(handler::models_handler))
        .with_state(state)
}

/// 快速构建用于单元与集成测试的代理 Axum 实例
pub async fn build_test_proxy_app(mock_uri: &str) -> Router {
    build_test_proxy_app_with_auth(mock_uri, None).await
}

/// 快速构建携带托管凭据的测试代理 Axum 实例
pub async fn build_test_proxy_app_with_auth(
    mock_uri: &str,
    upstream_api_key: Option<&str>,
) -> Router {
    let router_table = Arc::new(RouterTable::new());
    let mut backend = BackendInstance::new("mock-be", mock_uri, 10);
    if let Some(key) = upstream_api_key {
        backend = backend.with_api_key(key);
    }
    router_table.register_backend("*", backend);

    let state = ProxyState {
        router_table,
        proxy_client: ProxyClient::new(),
        key_store: None,
        rate_limiter: None,
        audit_logger: None,
    };

    build_proxy_router(state)
}
