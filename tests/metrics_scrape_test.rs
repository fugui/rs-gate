use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;

#[tokio::test]
async fn test_metrics_scrape_and_counter_increment() {
    let app = rs_gate::server::build_test_app().await;

    // 1. 模拟触发一次代理请求
    let proxy_req = Request::builder()
        .uri("/v1/chat/completions")
        .method("POST")
        .header(
            "authorization",
            "Bearer sk-gate-validkey00000000000000000000",
        )
        .header("content-type", "application/json")
        .body(Body::from(r#"{"model":"deepseek-r1"}"#))
        .unwrap();
    let _ = app.clone().oneshot(proxy_req).await.unwrap();

    // 2. 抓取 /metrics 端点
    let metrics_req = Request::builder()
        .uri("/metrics")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(metrics_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let metrics_text = String::from_utf8_lossy(&bytes);

    // 断言指标存在且计数递增
    assert!(
        metrics_text.contains("rsgate_requests_total"),
        "metrics must contain rsgate_requests_total"
    );
    assert!(
        metrics_text.contains("rsgate_active_connections"),
        "metrics must contain rsgate_active_connections"
    );
    assert!(
        metrics_text.contains("rsgate_retry_attempts_total"),
        "metrics must contain rsgate_retry_attempts_total"
    );
}
