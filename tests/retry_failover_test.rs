use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

#[tokio::test]
async fn test_silent_retry_and_failover_success() {
    let server_a = MockServer::start().await;
    let server_b = MockServer::start().await;

    // 节点 A 模拟 503 临时故障
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server_a)
        .await;

    // 节点 B 模拟 200 正常响应
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(r#"{"choices":[]}"#, "application/json"),
        )
        .mount(&server_b)
        .await;

    let app =
        rs_gate::server::build_test_app_with_cluster(vec![server_a.uri(), server_b.uri()]).await;

    let req = Request::builder()
        .uri("/v1/chat/completions")
        .method("POST")
        .header(
            "authorization",
            "Bearer sk-gate-validkey00000000000000000000",
        )
        .header("content-type", "application/json")
        .body(Body::from(r#"{"model":"deepseek-r1","messages":[]}"#))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    // 断言响应头明确回显重试 1 次
    assert_eq!(res.headers().get("x-gateway-retry-attempts").unwrap(), "1");
}

#[tokio::test]
async fn test_retry_exhaustion_returns_upstream_error() {
    let server_a = MockServer::start().await;
    let server_b = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server_a)
        .await;

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server_b)
        .await;

    let app =
        rs_gate::server::build_test_app_with_cluster(vec![server_a.uri(), server_b.uri()]).await;

    let req = Request::builder()
        .uri("/v1/chat/completions")
        .method("POST")
        .header(
            "authorization",
            "Bearer sk-gate-validkey00000000000000000000",
        )
        .header("content-type", "application/json")
        .body(Body::from(r#"{"model":"deepseek-r1","messages":[]}"#))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(res.headers().get("x-gateway-retry-attempts").unwrap(), "2");
}
