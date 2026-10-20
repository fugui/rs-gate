use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;
use wiremock::{
    matchers::{header, method, path},
    Mock, MockServer, ResponseTemplate,
};

#[tokio::test]
async fn test_chat_completions_passthrough() {
    let mock_server = MockServer::start().await;
    let expected_body =
        r#"{"id":"chatcmpl-test","choices":[{"message":{"content":"Hello来自上游"}}]}"#;

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(expected_body, "application/json"))
        .mount(&mock_server)
        .await;

    let app = rs_gate::proxy::build_test_proxy_app(&mock_server.uri()).await;

    let req = Request::builder()
        .uri("/v1/chat/completions")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"model":"mock-model","messages":[{"role":"user","content":"Hi"}]}"#,
        ))
        .unwrap();

    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&bytes), expected_body);
}

#[tokio::test]
async fn test_responses_passthrough() {
    let mock_server = MockServer::start().await;
    let expected_body =
        r#"{"id":"resp-123","output":[{"type":"message","content":"Agent output"}]}"#;

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(expected_body, "application/json"))
        .mount(&mock_server)
        .await;

    let app = rs_gate::proxy::build_test_proxy_app(&mock_server.uri()).await;

    let req = Request::builder()
        .uri("/v1/responses")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"model":"mock-model","input":"task"}"#))
        .unwrap();

    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&bytes), expected_body);
}

#[tokio::test]
async fn test_completions_passthrough() {
    let mock_server = MockServer::start().await;
    let expected_body = r#"{"id":"cmpl-456","choices":[{"text":"Completion text"}]}"#;

    Mock::given(method("POST"))
        .and(path("/v1/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(expected_body, "application/json"))
        .mount(&mock_server)
        .await;

    let app = rs_gate::proxy::build_test_proxy_app(&mock_server.uri()).await;

    let req = Request::builder()
        .uri("/v1/completions")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"model":"mock-model","prompt":"Hello"}"#))
        .unwrap();

    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&bytes), expected_body);
}

#[tokio::test]
async fn test_embeddings_passthrough() {
    let mock_server = MockServer::start().await;
    let expected_body = r#"{"object":"list","data":[{"embedding":[0.1,0.2,0.3],"index":0}]}"#;

    Mock::given(method("POST"))
        .and(path("/v1/embeddings"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(expected_body, "application/json"))
        .mount(&mock_server)
        .await;

    let app = rs_gate::proxy::build_test_proxy_app(&mock_server.uri()).await;

    let req = Request::builder()
        .uri("/v1/embeddings")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"model":"mock-model","input":"text"}"#))
        .unwrap();

    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&bytes), expected_body);
}

#[tokio::test]
async fn test_rerank_passthrough() {
    let mock_server = MockServer::start().await;
    let expected_body = r#"{"results":[{"index":0,"relevance_score":0.99}]}"#;

    Mock::given(method("POST"))
        .and(path("/v1/rerank"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(expected_body, "application/json"))
        .mount(&mock_server)
        .await;

    let app = rs_gate::proxy::build_test_proxy_app(&mock_server.uri()).await;

    let req = Request::builder()
        .uri("/v1/rerank")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"model":"mock-model","query":"test","documents":["doc1"]}"#,
        ))
        .unwrap();

    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&bytes), expected_body);
}

#[tokio::test]
async fn test_models_passthrough() {
    let mock_server = MockServer::start().await;
    let expected_body = r#"{"object":"list","data":[{"id":"mock-model","object":"model"}]}"#;

    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(expected_body, "application/json"))
        .mount(&mock_server)
        .await;

    let app = rs_gate::proxy::build_test_proxy_app(&mock_server.uri()).await;

    let req = Request::builder()
        .uri("/v1/models")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&bytes), expected_body);
}

#[tokio::test]
async fn test_header_sanitization_and_trace_id_injection() {
    let mock_server = MockServer::start().await;

    // Upstream should receive the injected upstream api key, not sk-gate-virtual
    // And should NOT receive hop-by-hop headers
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(header("authorization", "Bearer upstream-secret-token"))
        .and(header("x-request-id", "custom-req-id-123"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(r#"{"status":"ok"}"#, "application/json"),
        )
        .mount(&mock_server)
        .await;

    let app = rs_gate::proxy::build_test_proxy_app_with_auth(
        &mock_server.uri(),
        Some("upstream-secret-token"),
    )
    .await;

    let req = Request::builder()
        .uri("/v1/chat/completions")
        .method("POST")
        .header("content-type", "application/json")
        .header("authorization", "Bearer sk-gate-client-virtual-key")
        .header("x-request-id", "custom-req-id-123")
        .header("connection", "keep-alive")
        .header("upgrade", "websocket")
        .body(Body::from(r#"{"model":"mock-model","messages":[]}"#))
        .unwrap();

    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("x-request-id").unwrap(),
        "custom-req-id-123"
    );
}

#[tokio::test]
async fn test_sse_streaming_passthrough() {
    let mock_server = MockServer::start().await;
    let stream_body =
        "data: {\"choices\":[{\"delta\":{\"content\":\"Chunk1\"}}]}\n\ndata: [DONE]\n\n";

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_raw(stream_body, "text/event-stream"),
        )
        .mount(&mock_server)
        .await;

    let app = rs_gate::proxy::build_test_proxy_app(&mock_server.uri()).await;

    let req = Request::builder()
        .uri("/v1/chat/completions")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"model":"mock-model","stream":true}"#))
        .unwrap();

    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "text/event-stream"
    );

    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&bytes), stream_body);
}
