use crate::proxy::client::ProxyClient;
use crate::proxy::guard::ActiveConnectionGuard;
use crate::proxy::stream::create_abortable_sse_stream;
use crate::routing::backend::BackendInstance;
use crate::routing::scheduler::select_wlac_backend;
use crate::routing::RouterTable;
use axum::{
    body::{to_bytes, Body},
    extract::State,
    http::{HeaderMap, HeaderName, HeaderValue, Request, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone)]
pub struct ProxyState {
    pub router_table: Arc<RouterTable>,
    pub proxy_client: ProxyClient,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OpenAiErrorResponse {
    pub error: OpenAiErrorDetail,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OpenAiErrorDetail {
    pub message: String,
    pub r#type: String,
    pub param: Option<String>,
    pub code: Option<String>,
}

impl OpenAiErrorResponse {
    pub fn new(
        message: impl Into<String>,
        err_type: impl Into<String>,
        code: impl Into<String>,
    ) -> Self {
        Self {
            error: OpenAiErrorDetail {
                message: message.into(),
                r#type: err_type.into(),
                param: None,
                code: Some(code.into()),
            },
        }
    }
}

#[derive(Deserialize)]
struct ModelRequestPayload {
    model: Option<String>,
}

/// 统一的大模型反向代理分派处理逻辑 (Zero AST Overhead)
pub async fn forward_proxy_request(state: State<ProxyState>, req: Request<Body>) -> Response {
    let (parts, body) = req.into_parts();
    let method = parts.method;
    let path_and_query = parts
        .uri
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or(parts.uri.path());

    // 读取请求体字节流
    let body_bytes = match to_bytes(body, usize::MAX).await {
        Ok(b) => b,
        Err(err) => {
            tracing::error!("Failed to read request body: {}", err);
            return (
                StatusCode::BAD_REQUEST,
                Json(OpenAiErrorResponse::new(
                    format!("Failed to read request body: {}", err),
                    "invalid_request_error",
                    "bad_request",
                )),
            )
                .into_response();
        }
    };

    // 嗅探目标模型名称
    let target_model =
        if let Ok(payload) = serde_json::from_slice::<ModelRequestPayload>(&body_bytes) {
            payload.model.unwrap_or_else(|| "*".to_string())
        } else {
            "*".to_string()
        };

    // 从路由表查询候选后端
    let backends = state.router_table.get_backends(&target_model);
    let candidates: Vec<BackendInstance> = match backends {
        Some(list) if !list.is_empty() => list,
        _ => {
            tracing::warn!(model = %target_model, "No backend found for model");
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(OpenAiErrorResponse::new(
                    format!("No active backend configured for model '{}'", target_model),
                    "server_error",
                    "no_healthy_backend",
                )),
            )
                .into_response();
        }
    };

    // WLAC 算法选路
    let selected_backend = match select_wlac_backend(&candidates) {
        Some(b) => b.clone(),
        None => {
            tracing::warn!(model = %target_model, "All backends are unhealthy");
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(OpenAiErrorResponse::new(
                    "All backends are currently unhealthy or overloaded",
                    "server_error",
                    "no_healthy_backend",
                )),
            )
                .into_response();
        }
    };

    // 申领 RAII 活跃长连接计数守卫 (自动 fetch_add(1))
    let guard = ActiveConnectionGuard::acquire(selected_backend.active_connections.clone())
        .with_backend_id(&selected_backend.id);

    // 清洗请求头并注入链路追踪与托管凭据
    let (upstream_headers, request_id) = state.proxy_client.prepare_upstream_headers(
        &parts.headers,
        selected_backend.upstream_api_key.as_deref(),
        None,
    );

    // 拼接上游目标完整 URL
    let upstream_url = format!(
        "{}{}",
        selected_backend.base_url.trim_end_matches('/'),
        path_and_query
    );

    let client = state.proxy_client.client();
    let mut req_builder = client
        .request(
            reqwest::Method::from_bytes(method.as_str().as_bytes())
                .unwrap_or(reqwest::Method::POST),
            &upstream_url,
        )
        .headers(upstream_headers);

    if !body_bytes.is_empty() {
        req_builder = req_builder.body(body_bytes);
    }

    let upstream_res = match req_builder.send().await {
        Ok(res) => res,
        Err(err) => {
            tracing::error!(
                backend = %selected_backend.id,
                url = %upstream_url,
                "Upstream connection failed: {}", err
            );
            return (
                StatusCode::BAD_GATEWAY,
                Json(OpenAiErrorResponse::new(
                    format!("Failed to connect to upstream backend: {}", err),
                    "server_error",
                    "bad_gateway",
                )),
            )
                .into_response();
        }
    };

    let status = StatusCode::from_u16(upstream_res.status().as_u16())
        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

    // 检查是否为 SSE 流式响应
    let is_sse = upstream_res
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|ct| ct.contains("text/event-stream"))
        .unwrap_or(false);

    if is_sse {
        let mut response = create_abortable_sse_stream(upstream_res, guard);
        // 确保下游响应头包含 x-request-id
        if let Ok(val) = HeaderValue::from_str(&request_id) {
            response
                .headers_mut()
                .insert(HeaderName::from_static("x-request-id"), val);
        }
        return response;
    }

    // 非流式响应处理
    let res_headers = upstream_res.headers().clone();
    let res_bytes = match upstream_res.bytes().await {
        Ok(b) => b,
        Err(err) => {
            tracing::error!("Failed to read upstream response body: {}", err);
            return (
                StatusCode::BAD_GATEWAY,
                Json(OpenAiErrorResponse::new(
                    "Error reading upstream response body",
                    "server_error",
                    "bad_gateway",
                )),
            )
                .into_response();
        }
    };

    // 显式析构 guard 回收活跃连接计数
    drop(guard);

    let mut response_builder = Response::builder().status(status);

    // 复制非 Hop-by-Hop 响应头
    if let Some(headers_mut) = response_builder.headers_mut() {
        copy_downstream_headers(&res_headers, headers_mut, &request_id);
    }

    match response_builder.body(Body::from(res_bytes)) {
        Ok(res) => res,
        Err(err) => {
            tracing::error!("Failed to build proxy response: {}", err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal Server Error Building Response",
            )
                .into_response()
        }
    }
}

fn copy_downstream_headers(
    source: &reqwest::header::HeaderMap,
    target: &mut HeaderMap,
    request_id: &str,
) {
    for (name, value) in source.iter() {
        let name_str = name.as_str().to_ascii_lowercase();
        if name_str == "connection"
            || name_str == "keep-alive"
            || name_str == "transfer-encoding"
            || name_str == "upgrade"
        {
            continue;
        }

        if let Ok(axum_name) = HeaderName::from_bytes(name.as_str().as_bytes()) {
            if let Ok(axum_val) = HeaderValue::from_bytes(value.as_bytes()) {
                target.insert(axum_name, axum_val);
            }
        }
    }

    // 确保 x-request-id 透传
    if let Ok(val) = HeaderValue::from_str(request_id) {
        target.insert(HeaderName::from_static("x-request-id"), val);
    }
}

/// 6 大直通端点独立处理器函数
pub async fn chat_completions_handler(state: State<ProxyState>, req: Request<Body>) -> Response {
    forward_proxy_request(state, req).await
}

pub async fn responses_handler(state: State<ProxyState>, req: Request<Body>) -> Response {
    forward_proxy_request(state, req).await
}

pub async fn completions_handler(state: State<ProxyState>, req: Request<Body>) -> Response {
    forward_proxy_request(state, req).await
}

pub async fn embeddings_handler(state: State<ProxyState>, req: Request<Body>) -> Response {
    forward_proxy_request(state, req).await
}

pub async fn rerank_handler(state: State<ProxyState>, req: Request<Body>) -> Response {
    forward_proxy_request(state, req).await
}

pub async fn models_handler(state: State<ProxyState>, req: Request<Body>) -> Response {
    forward_proxy_request(state, req).await
}
