use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;

#[tokio::test]
async fn test_invalid_key_returns_401() {
    let app = rs_gate::server::build_test_app().await;
    let req = Request::builder()
        .uri("/v1/chat/completions")
        .method("POST")
        .header(
            "authorization",
            "Bearer sk-gate-invalidkey000000000000000000",
        )
        .header("content-type", "application/json")
        .body(Body::from(r#"{"model":"deepseek-r1"}"#))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let err_json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(err_json["error"]["type"], "authentication_error");
}

#[tokio::test]
async fn test_missing_auth_header_returns_401() {
    let app = rs_gate::server::build_test_app().await;
    let req = Request::builder()
        .uri("/v1/chat/completions")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"model":"deepseek-r1"}"#))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_cidr_ip_whitelist_rejected_returns_403() {
    let app = rs_gate::server::build_test_app_with_ip_whitelist("192.168.1.0/24").await;
    let req = Request::builder()
        .uri("/v1/chat/completions")
        .method("POST")
        .header(
            "authorization",
            "Bearer sk-gate-validkey00000000000000000000",
        )
        .header("x-forwarded-for", "10.0.0.1") // 不在白名单网段内
        .header("content-type", "application/json")
        .body(Body::from(r#"{"model":"deepseek-r1"}"#))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let err_json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(err_json["error"]["type"], "permission_denied");
}

#[tokio::test]
async fn test_disallowed_model_returns_403() {
    let app =
        rs_gate::server::build_test_app_with_allowed_models(vec!["deepseek-r1".to_string()]).await;
    let req = Request::builder()
        .uri("/v1/chat/completions")
        .method("POST")
        .header(
            "authorization",
            "Bearer sk-gate-validkey00000000000000000000",
        )
        .header("content-type", "application/json")
        .body(Body::from(r#"{"model":"unauthorized-gpt4"}"#))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let err_json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(err_json["error"]["type"], "permission_denied");
}
