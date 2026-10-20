use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;

#[tokio::test]
async fn test_user_cannot_access_admin_api() {
    let app = rs_gate::server::build_test_admin_app().await;
    let user_token = rs_gate::admin::auth::generate_test_token("usr-1", "user");

    let req = Request::builder()
        .uri("/api/admin/users")
        .method("GET")
        .header("authorization", format!("Bearer {}", user_token))
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    // 普通 user 角色必须被 403 强拦截
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_admin_can_access_admin_api() {
    let app = rs_gate::server::build_test_admin_app().await;
    let admin_token = rs_gate::admin::auth::generate_test_token("usr-admin", "admin");

    let req = Request::builder()
        .uri("/api/admin/users")
        .method("GET")
        .header("authorization", format!("Bearer {}", admin_token))
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_missing_or_invalid_jwt_returns_401() {
    let app = rs_gate::server::build_test_admin_app().await;

    // 缺少 Token
    let req1 = Request::builder()
        .uri("/api/user/keys")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let res1 = app.clone().oneshot(req1).await.unwrap();
    assert_eq!(res1.status(), StatusCode::UNAUTHORIZED);

    // 伪造非法 Token
    let req2 = Request::builder()
        .uri("/api/user/keys")
        .method("GET")
        .header("authorization", "Bearer invalid.jwt.token")
        .body(Body::empty())
        .unwrap();
    let res2 = app.oneshot(req2).await.unwrap();
    assert_eq!(res2.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_tenant_data_strict_isolation() {
    let app = rs_gate::server::build_test_admin_app().await;
    let user_a_token = rs_gate::admin::auth::generate_test_token("usr-a", "user");

    // 用户 A 查询自身 Key 列表
    let req = Request::builder()
        .uri("/api/user/keys")
        .method("GET")
        .header("authorization", format!("Bearer {}", user_a_token))
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

    // 断言列表中绝对不包含归属于用户 B 的数据
    for item in json["data"]["items"].as_array().unwrap() {
        assert_eq!(item["user_id"], "usr-a");
    }
}
