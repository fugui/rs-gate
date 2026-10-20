use crate::audit::{AuditEvent, AuditLogger};
use crate::auth::key::KeyStore;
use crate::limiting::{GatewayRateLimitError, RateLimiterManager};
use crate::proxy::client::ProxyClient;
use crate::proxy::guard::ActiveConnectionGuard;
use crate::proxy::retry::{calculate_backoff, is_retryable_status, RetryConfig};
use crate::proxy::stream::{create_abortable_sse_stream, TokenUsage};
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
use std::time::Instant;

#[derive(Clone)]
pub struct ProxyState {
    pub router_table: Arc<RouterTable>,
    pub proxy_client: ProxyClient,
    pub key_store: Option<Arc<KeyStore>>,
    pub rate_limiter: Option<Arc<RateLimiterManager>>,
    pub audit_logger: Option<AuditLogger>,
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
    let start_time = Instant::now();
    let (parts, body) = req.into_parts();
    let method = parts.method;
    let path_and_query = parts
        .uri
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or(parts.uri.path())
        .to_string();

    // 提取客户端 IP (从 X-Forwarded-For 获取首个客户端 IP)
    let client_ip = parts
        .headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "127.0.0.1".to_string());

    // 1. 虚拟 Key 鉴权校验 (若已启用 KeyStore)
    let authenticated_key = if let Some(ref key_store) = state.key_store {
        let auth_header = parts
            .headers
            .get("authorization")
            .and_then(|v| v.to_str().ok());

        let raw_key = match auth_header {
            Some(val) if val.starts_with("Bearer ") => val["Bearer ".len()..].trim(),
            _ => {
                crate::metrics::record_request("unknown", StatusCode::UNAUTHORIZED.as_u16(), false);
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(OpenAiErrorResponse::new(
                        "Missing or invalid Authorization header",
                        "authentication_error",
                        "invalid_api_key",
                    )),
                )
                    .into_response();
            }
        };

        match key_store.authenticate(raw_key) {
            Some(record) => {
                // CIDR IP 白名单核验
                if !record.is_ip_allowed(&client_ip) {
                    crate::metrics::record_request(
                        "unknown",
                        StatusCode::FORBIDDEN.as_u16(),
                        false,
                    );
                    return (
                        StatusCode::FORBIDDEN,
                        Json(OpenAiErrorResponse::new(
                            format!(
                                "Client IP '{}' is not in the allowed CIDR whitelist",
                                client_ip
                            ),
                            "permission_denied",
                            "ip_forbidden",
                        )),
                    )
                        .into_response();
                }
                Some(record)
            }
            None => {
                crate::metrics::record_request("unknown", StatusCode::UNAUTHORIZED.as_u16(), false);
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(OpenAiErrorResponse::new(
                        "Incorrect API key provided or key has expired/inactive",
                        "authentication_error",
                        "invalid_api_key",
                    )),
                )
                    .into_response();
            }
        }
    } else {
        None
    };

    // 2. 读取请求体字节流
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

    // 3. 嗅探目标模型名称
    let target_model =
        if let Ok(payload) = serde_json::from_slice::<ModelRequestPayload>(&body_bytes) {
            payload.model.unwrap_or_else(|| "*".to_string())
        } else {
            "*".to_string()
        };

    // 4. 逻辑模型权限校验
    if let Some(ref key) = authenticated_key {
        if !key.is_model_allowed(&target_model) {
            return (
                StatusCode::FORBIDDEN,
                Json(OpenAiErrorResponse::new(
                    format!("Key is not authorized to access model '{}'", target_model),
                    "permission_denied",
                    "model_forbidden",
                )),
            )
                .into_response();
        }
    }

    // 5. 无锁令牌桶 RPM、在途并发门限与日/周硬配额准入预检
    let _key_guard =
        if let (Some(ref limiter), Some(ref key)) = (&state.rate_limiter, &authenticated_key) {
            match limiter.acquire_admission(
                &key.id,
                key.rpm_limit,
                key.max_concurrency,
                500, // 500 Token 预留门限
            ) {
                Ok(guard) => Some(guard),
                Err(GatewayRateLimitError::RpmLimitExceeded) => {
                    let mut res = (
                        StatusCode::TOO_MANY_REQUESTS,
                        Json(OpenAiErrorResponse::new(
                            "Rate limit exceeded (RPM limit reached)",
                            "rate_limit_error",
                            "rate_limit_exceeded",
                        )),
                    )
                        .into_response();
                    res.headers_mut().insert(
                        HeaderName::from_static("retry-after"),
                        HeaderValue::from_static("1"),
                    );
                    return res;
                }
                Err(GatewayRateLimitError::ConcurrencyLimitExceeded(limit)) => {
                    let mut res = (
                        StatusCode::TOO_MANY_REQUESTS,
                        Json(OpenAiErrorResponse::new(
                            format!("Concurrency limit of {} exceeded", limit),
                            "rate_limit_error",
                            "concurrency_limit_exceeded",
                        )),
                    )
                        .into_response();
                    res.headers_mut().insert(
                        HeaderName::from_static("retry-after"),
                        HeaderValue::from_static("1"),
                    );
                    return res;
                }
                Err(GatewayRateLimitError::QuotaExceeded(quota_err)) => {
                    return (
                        StatusCode::TOO_MANY_REQUESTS,
                        Json(OpenAiErrorResponse::new(
                            format!("Insufficient quota: {}", quota_err),
                            "insufficient_quota",
                            "quota_exceeded",
                        )),
                    )
                        .into_response();
                }
            }
        } else {
            None
        };

    // 6. 从路由表查询候选后端
    let backends = state.router_table.get_backends(&target_model);
    let candidates: Vec<BackendInstance> = match backends {
        Some(list) if !list.is_empty() => list,
        _ => {
            tracing::warn!(model = %target_model, "No backend found for model");
            crate::metrics::record_request(
                &target_model,
                StatusCode::SERVICE_UNAVAILABLE.as_u16(),
                false,
            );
            crate::metrics::record_request_duration(
                &target_model,
                start_time.elapsed().as_secs_f64(),
            );
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

    let retry_config = RetryConfig::default();
    let mut attempts = 0usize;
    let mut tried_backend_ids = Vec::new();
    let mut failover_path = Vec::new();

    // 7. 上游故障转移与静默重试循环 (Max 2 次重试，总计最多 3 次尝试)
    let (upstream_res, guard, last_request_id, last_selected_backend_id) = loop {
        // 优先调度尚未尝试过的候选后端 (Failover)
        let untried: Vec<BackendInstance> = candidates
            .iter()
            .filter(|b| !tried_backend_ids.contains(&b.id))
            .cloned()
            .collect();

        let selected = if !untried.is_empty() {
            select_wlac_backend(&untried)
        } else {
            select_wlac_backend(&candidates)
        };

        let selected_backend = match selected {
            Some(b) => b.clone(),
            None => {
                tracing::warn!(model = %target_model, "All backends are unhealthy");
                crate::metrics::record_request(
                    &target_model,
                    StatusCode::SERVICE_UNAVAILABLE.as_u16(),
                    false,
                );
                crate::metrics::record_request_duration(
                    &target_model,
                    start_time.elapsed().as_secs_f64(),
                );
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

        tried_backend_ids.push(selected_backend.id.clone());
        failover_path.push(selected_backend.id.clone());

        // 申领 RAII 活跃长连接计数守卫
        let guard = ActiveConnectionGuard::acquire(selected_backend.active_connections.clone())
            .with_backend_id(&selected_backend.id);

        let (upstream_headers, request_id) = state.proxy_client.prepare_upstream_headers(
            &parts.headers,
            selected_backend.upstream_api_key.as_deref(),
            Some(&client_ip),
        );

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
            req_builder = req_builder.body(body_bytes.clone());
        }

        let send_result = req_builder.send().await;

        match send_result {
            Ok(res) => {
                let status = StatusCode::from_u16(res.status().as_u16())
                    .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

                // 检查是否为可静默重试状态 (5xx 或 429)
                if is_retryable_status(status) && attempts < retry_config.max_retries {
                    attempts += 1;
                    crate::metrics::record_retry(&format!("status_{}", status.as_u16()));
                    let backoff = calculate_backoff(attempts, Some(&res), &retry_config);
                    drop(guard);
                    tokio::time::sleep(backoff).await;
                    continue;
                }

                // 正常响应或重试次数已耗尽
                break (Ok(res), guard, request_id, selected_backend.id);
            }
            Err(err) => {
                tracing::warn!(
                    backend = %selected_backend.id,
                    url = %upstream_url,
                    attempt = attempts,
                    "Upstream connection failed: {}", err
                );

                if attempts < retry_config.max_retries {
                    attempts += 1;
                    crate::metrics::record_retry("network_error");
                    let backoff = calculate_backoff(attempts, None, &retry_config);
                    drop(guard);
                    tokio::time::sleep(backoff).await;
                    continue;
                }

                // 网络错误且重试耗尽
                break (Err(err), guard, request_id, selected_backend.id);
            }
        }
    };

    let upstream_res = match upstream_res {
        Ok(res) => res,
        Err(err) => {
            crate::metrics::record_request(&target_model, StatusCode::BAD_GATEWAY.as_u16(), false);
            crate::metrics::record_request_duration(
                &target_model,
                start_time.elapsed().as_secs_f64(),
            );
            let mut response = (
                StatusCode::BAD_GATEWAY,
                Json(OpenAiErrorResponse::new(
                    format!("Failed to connect to upstream backend: {}", err),
                    "server_error",
                    "bad_gateway",
                )),
            )
                .into_response();

            inject_meta_headers(response.headers_mut(), &last_request_id, attempts);
            return response;
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

    // 协议端点识别
    let protocol = if path_and_query.contains("/chat/completions") {
        "chat"
    } else if path_and_query.contains("/responses") {
        "responses"
    } else if path_and_query.contains("/completions") {
        "completions"
    } else if path_and_query.contains("/embeddings") {
        "embeddings"
    } else if path_and_query.contains("/rerank") {
        "rerank"
    } else {
        "models"
    };

    if is_sse {
        crate::metrics::record_request(&target_model, status.as_u16(), true);
        crate::metrics::record_request_duration(&target_model, start_time.elapsed().as_secs_f64());
        let mut response = create_abortable_sse_stream(upstream_res, guard);
        inject_meta_headers(response.headers_mut(), &last_request_id, attempts);

        // 投递初始流式审计事件
        if let Some(ref logger) = state.audit_logger {
            let event = AuditEvent {
                id: uuid::Uuid::new_v4().to_string(),
                trace_id: last_request_id,
                key_id: authenticated_key
                    .as_ref()
                    .map(|k| k.id.clone())
                    .unwrap_or_default(),
                user_id: authenticated_key.as_ref().map(|k| k.user_id.clone()),
                model: target_model,
                backend_id: last_selected_backend_id,
                protocol_str: protocol.to_string(),
                client_ip,
                http_status: status.as_u16(),
                usage: TokenUsage {
                    prompt_tokens: 0,
                    completion_tokens: 0,
                    total_tokens: 0,
                },
                duration_ms: start_time.elapsed().as_millis() as u64,
                ttft_ms: None,
                is_stream: true,
                retry_count: attempts as u32,
                failover_path: serde_json::to_string(&failover_path).ok(),
                error_message: None,
                created_at_timestamp: chrono::Utc::now().timestamp_millis(),
            };
            let _ = logger.try_send(event);
        }

        return response;
    }

    // 非流式响应处理
    let res_headers = upstream_res.headers().clone();
    let res_bytes = match upstream_res.bytes().await {
        Ok(b) => b,
        Err(err) => {
            tracing::error!("Failed to read upstream response body: {}", err);
            let mut response = (
                StatusCode::BAD_GATEWAY,
                Json(OpenAiErrorResponse::new(
                    "Error reading upstream response body",
                    "server_error",
                    "bad_gateway",
                )),
            )
                .into_response();
            inject_meta_headers(response.headers_mut(), &last_request_id, attempts);
            return response;
        }
    };

    // 显式释放连接守卫
    drop(guard);

    let mut response_builder = Response::builder().status(status);

    if let Some(headers_mut) = response_builder.headers_mut() {
        copy_downstream_headers(&res_headers, headers_mut, &last_request_id);
        inject_meta_headers(headers_mut, &last_request_id, attempts);
    }

    // 投递非流式审计事件
    if let Some(ref logger) = state.audit_logger {
        let event = AuditEvent {
            id: uuid::Uuid::new_v4().to_string(),
            trace_id: last_request_id,
            key_id: authenticated_key
                .as_ref()
                .map(|k| k.id.clone())
                .unwrap_or_default(),
            user_id: authenticated_key.as_ref().map(|k| k.user_id.clone()),
            model: target_model.clone(),
            backend_id: last_selected_backend_id,
            protocol_str: protocol.to_string(),
            client_ip,
            http_status: status.as_u16(),
            usage: TokenUsage {
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
            },
            duration_ms: start_time.elapsed().as_millis() as u64,
            ttft_ms: None,
            is_stream: false,
            retry_count: attempts as u32,
            failover_path: serde_json::to_string(&failover_path).ok(),
            error_message: if status.is_client_error() || status.is_server_error() {
                Some(format!("HTTP {}", status.as_u16()))
            } else {
                None
            },
            created_at_timestamp: chrono::Utc::now().timestamp_millis(),
        };
        let _ = logger.try_send(event);
    }

    crate::metrics::record_request(&target_model, status.as_u16(), false);
    crate::metrics::record_request_duration(&target_model, start_time.elapsed().as_secs_f64());

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

fn inject_meta_headers(headers: &mut HeaderMap, request_id: &str, attempts: usize) {
    if let Ok(val) = HeaderValue::from_str(request_id) {
        headers.insert(HeaderName::from_static("x-request-id"), val);
    }
    if let Ok(val) = HeaderValue::from_str(&attempts.to_string()) {
        headers.insert(HeaderName::from_static("x-gateway-retry-attempts"), val);
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
