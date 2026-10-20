use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;

#[tokio::test]
async fn test_embedded_root_and_spa_history_fallback() {
    let app = rs_gate::server::build_test_app().await;

    // 1. 请求根路径应返回 index.html
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/")
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        res.headers().get("content-type").unwrap(),
        "text/html; charset=utf-8"
    );

    // 2. 请求深层前端路由 /portal/keys，必须 SPA 回退至 index.html
    let res_fallback = app
        .oneshot(
            Request::builder()
                .uri("/portal/keys")
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_fallback.status(), StatusCode::OK);
    assert_eq!(
        res_fallback.headers().get("content-type").unwrap(),
        "text/html; charset=utf-8"
    );
}
