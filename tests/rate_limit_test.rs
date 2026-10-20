use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use rs_gate::limiting::concurrency::KeyConcurrencyGuard;
use rs_gate::limiting::token_bucket::AtomicTokenBucket;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;
use std::time::Duration;
use tower::ServiceExt;

#[test]
fn test_atomic_token_bucket_cas_rate_limiting() {
    // 限制每分钟 60 次 (1 次/秒)，初始容量 2
    let bucket = AtomicTokenBucket::new(60, 2);

    assert!(bucket.try_acquire(1), "First request should pass");
    assert!(bucket.try_acquire(1), "Second request should pass");
    assert!(!bucket.try_acquire(1), "Third request must be rate limited");

    std::thread::sleep(Duration::from_millis(1050));
    assert!(bucket.try_acquire(1), "Token replenished after 1 second");
}

#[test]
fn test_key_concurrency_guard_raii() {
    let counter = Arc::new(AtomicUsize::new(0));
    let guard1 = KeyConcurrencyGuard::try_acquire(counter.clone(), 2).unwrap();
    assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 1);

    let guard2 = KeyConcurrencyGuard::try_acquire(counter.clone(), 2).unwrap();
    assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 2);

    // Exceeds max concurrency 2
    let err = KeyConcurrencyGuard::try_acquire(counter.clone(), 2);
    assert!(
        err.is_err(),
        "Must be rejected when concurrency reached limit"
    );

    drop(guard1);
    assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 1);

    let guard3 = KeyConcurrencyGuard::try_acquire(counter.clone(), 2).unwrap();
    assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 2);
    drop(guard2);
    drop(guard3);
    assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 0);
}

#[tokio::test]
async fn test_rate_limit_rpm_returns_429() {
    let app = rs_gate::server::build_test_app_with_rpm_limit(1).await;

    // First request should succeed
    let req1 = Request::builder()
        .uri("/v1/chat/completions")
        .method("POST")
        .header(
            "authorization",
            "Bearer sk-gate-validkey00000000000000000000",
        )
        .header("content-type", "application/json")
        .body(Body::from(r#"{"model":"deepseek-r1"}"#))
        .unwrap();

    let res1 = app.clone().oneshot(req1).await.unwrap();
    assert!(
        res1.status() != StatusCode::TOO_MANY_REQUESTS,
        "First request within limit"
    );

    // Second immediate request should be rate limited to 429
    let req2 = Request::builder()
        .uri("/v1/chat/completions")
        .method("POST")
        .header(
            "authorization",
            "Bearer sk-gate-validkey00000000000000000000",
        )
        .header("content-type", "application/json")
        .body(Body::from(r#"{"model":"deepseek-r1"}"#))
        .unwrap();

    let res2 = app.oneshot(req2).await.unwrap();
    assert_eq!(res2.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        res2.headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok()),
        Some("1")
    );
    let bytes = axum::body::to_bytes(res2.into_body(), usize::MAX)
        .await
        .unwrap();
    let err_json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(err_json["error"]["type"], "rate_limit_error");
}
