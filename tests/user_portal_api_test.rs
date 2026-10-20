use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;

#[tokio::test]
async fn test_create_virtual_key_returns_secret_once() {
    let app = rs_gate::server::build_test_admin_app().await;
    let user_token = rs_gate::admin::auth::generate_test_token("usr-1", "user");

    let create_payload = r#"{"name":"Dev Key","daily_token_quota":100000}"#;
    let req = Request::builder()
        .uri("/api/user/keys")
        .method("POST")
        .header("authorization", format!("Bearer {}", user_token))
        .header("content-type", "application/json")
        .body(Body::from(create_payload))
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

    // 创建响应必须包含一次性完整明文 Key
    let plaintext_key = json["data"]["plaintext_key"].as_str().unwrap();
    assert!(plaintext_key.starts_with("sk-gate-"));

    // 随后再次发起列表查询
    let list_req = Request::builder()
        .uri("/api/user/keys")
        .method("GET")
        .header("authorization", format!("Bearer {}", user_token))
        .body(Body::empty())
        .unwrap();

    let list_res = app.oneshot(list_req).await.unwrap();
    let list_bytes = axum::body::to_bytes(list_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let list_json: serde_json::Value = serde_json::from_slice(&list_bytes).unwrap();

    // 列表中只包含脱敏 prefix，明文字段必须不存在或为空
    let item = &list_json["data"]["items"][0];
    assert!(item.get("plaintext_key").is_none());
    assert!(item["key_prefix"].as_str().unwrap().starts_with("sk-gate-"));
}

#[tokio::test]
async fn test_user_available_models_and_snippets() {
    let app = rs_gate::server::build_test_admin_app().await;
    let user_token = rs_gate::admin::auth::generate_test_token("usr-1", "user");

    let req = Request::builder()
        .uri("/api/user/models")
        .method("GET")
        .header("authorization", format!("Bearer {}", user_token))
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(json["code"], 0);
    let models = json["data"].as_array().unwrap();
    assert!(!models.is_empty());
    assert!(models[0]["code_snippets"]["python"]
        .as_str()
        .unwrap()
        .contains("client = OpenAI"));
}

#[tokio::test]
async fn test_user_update_and_delete_key() {
    let app = rs_gate::server::build_test_admin_app().await;
    let user_token = rs_gate::admin::auth::generate_test_token("usr-1", "user");

    // 1. 创建 key
    let create_payload = r#"{"name":"To Be Modified","model_whitelist":["deepseek-r1"]}"#;
    let req = Request::builder()
        .uri("/api/user/keys")
        .method("POST")
        .header("authorization", format!("Bearer {}", user_token))
        .header("content-type", "application/json")
        .body(Body::from(create_payload))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let key_id = json["data"]["id"].as_str().unwrap();

    // 2. 更新 key
    let update_payload =
        r#"{"name":"Modified Name","model_whitelist":["deepseek-r1","qwen-2.5-72b"]}"#;
    let req = Request::builder()
        .uri(format!("/api/user/keys/{}", key_id))
        .method("PUT")
        .header("authorization", format!("Bearer {}", user_token))
        .header("content-type", "application/json")
        .body(Body::from(update_payload))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 3. 删除 key
    let req = Request::builder()
        .uri(format!("/api/user/keys/{}", key_id))
        .method("DELETE")
        .header("authorization", format!("Bearer {}", user_token))
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}
