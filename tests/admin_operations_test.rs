use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;

#[tokio::test]
async fn test_admin_one_click_reset_quota() {
    let app = rs_gate::server::build_test_admin_app().await;
    let admin_token = rs_gate::admin::auth::generate_test_token("usr-admin", "admin");

    let req = Request::builder()
        .uri("/api/admin/keys/key-001/reset-quota")
        .method("POST")
        .header("authorization", format!("Bearer {}", admin_token))
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_admin_streaming_csv_export() {
    let app = rs_gate::server::build_test_admin_app().await;
    let admin_token = rs_gate::admin::auth::generate_test_token("usr-admin", "admin");

    let req = Request::builder()
        .uri("/api/admin/audits/export?start_time=0")
        .method("GET")
        .header("authorization", format!("Bearer {}", admin_token))
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        res.headers().get("content-type").unwrap(),
        "text/csv; charset=utf-8"
    );
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let csv_content = String::from_utf8_lossy(&bytes);
    assert!(csv_content.starts_with("id,trace_id,key_id,model,backend_id"));
}

#[tokio::test]
async fn test_admin_stats_overview() {
    let app = rs_gate::server::build_test_admin_app().await;
    let admin_token = rs_gate::admin::auth::generate_test_token("usr-admin", "admin");

    let req = Request::builder()
        .uri("/api/admin/stats/overview")
        .method("GET")
        .header("authorization", format!("Bearer {}", admin_token))
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["code"], 0);
    assert!(json["data"].get("healthy_backends_count").is_some());
}
