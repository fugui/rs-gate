mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use common::TestDbHarness;
use tower::ServiceExt;
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

#[tokio::test]
async fn test_e2e_full_system_lifecycle() {
    // 步骤 1: 系统冷启动并自建超管
    let harness = TestDbHarness::new().await.unwrap();
    rs_gate::storage::migration::run_migrations(&harness.pool)
        .await
        .unwrap();

    let admin =
        rs_gate::storage::bootstrap::ensure_superadmin(&harness.pool, Some("AdminPass123!"))
            .await
            .unwrap();
    assert_eq!(admin.username, "admin");

    let app = rs_gate::server::build_app(Some(harness.pool.clone()))
        .await
        .unwrap();

    // 步骤 2: 登录超管并创建开发者用户与模型集群
    // 2.1 超管登录获取 JWT Token
    let login_req = Request::builder()
        .uri("/api/auth/login")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"username":"admin","password":"AdminPass123!"}"#,
        ))
        .unwrap();

    let login_res = app.clone().oneshot(login_req).await.unwrap();
    assert_eq!(login_res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(login_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let login_json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let admin_token = login_json["data"]["token"].as_str().unwrap().to_string();

    // 2.2 超管创建开发者用户
    let create_user_req = Request::builder()
        .uri("/api/admin/users")
        .method("POST")
        .header("authorization", format!("Bearer {}", admin_token))
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{
                "username":"dev_alice",
                "password":"DevPassword123!",
                "role":"user",
                "display_name":"Alice Dev",
                "email":"alice@example.com"
            }"#,
        ))
        .unwrap();
    let user_res = app.clone().oneshot(create_user_req).await.unwrap();
    assert_eq!(user_res.status(), StatusCode::OK);

    // 2.3 启动上游 Mock 集群
    let server_a = MockServer::start().await;
    let server_b = MockServer::start().await;

    // Server A 返回 502 模拟上游物理节点抖动报错
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(502).set_body_string("Bad Gateway Error"))
        .mount(&server_a)
        .await;

    // Server B 模拟 200 流式 SSE 正常响应
    let sse_chunk = "data: {\"choices\":[{\"delta\":{\"content\":\"Hello world\"}}]}\n\ndata: {\"choices\":[],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":20,\"total_tokens\":30}}\n\ndata: [DONE]\n\n";
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(sse_chunk, "text/event-stream"))
        .mount(&server_b)
        .await;

    // 2.4 超管注册两个物理后端 (Node A 与 Node B)
    let create_be_a = Request::builder()
        .uri("/api/admin/backends")
        .method("POST")
        .header("authorization", format!("Bearer {}", admin_token))
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({
                "id": "be-a",
                "name": "Node A",
                "endpoint_url": server_a.uri(),
                "weight": 100
            })
            .to_string(),
        ))
        .unwrap();
    let res_be_a = app.clone().oneshot(create_be_a).await.unwrap();
    assert_eq!(res_be_a.status(), StatusCode::OK);

    let create_be_b = Request::builder()
        .uri("/api/admin/backends")
        .method("POST")
        .header("authorization", format!("Bearer {}", admin_token))
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({
                "id": "be-b",
                "name": "Node B",
                "endpoint_url": server_b.uri(),
                "weight": 1
            })
            .to_string(),
        ))
        .unwrap();
    let res_be_b = app.clone().oneshot(create_be_b).await.unwrap();
    assert_eq!(res_be_b.status(), StatusCode::OK);

    // 2.5 超管创建逻辑模型 deepseek-r1 (允许 2 次重试)
    let create_model_req = Request::builder()
        .uri("/api/admin/models")
        .method("POST")
        .header("authorization", format!("Bearer {}", admin_token))
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({
                "id": "deepseek-r1",
                "display_name": "DeepSeek R1",
                "description": "Full lifecycle model",
                "max_retries": 2,
                "retry_interval_ms": 10
            })
            .to_string(),
        ))
        .unwrap();
    let res_model = app.clone().oneshot(create_model_req).await.unwrap();
    assert_eq!(res_model.status(), StatusCode::OK);

    // 2.6 超管绑定模型后端
    let bind_a = Request::builder()
        .uri("/api/admin/models/deepseek-r1/backends")
        .method("POST")
        .header("authorization", format!("Bearer {}", admin_token))
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({
                "backend_id": "be-a",
                "weight": 100,
                "priority": 1
            })
            .to_string(),
        ))
        .unwrap();
    let res_bind_a = app.clone().oneshot(bind_a).await.unwrap();
    assert_eq!(res_bind_a.status(), StatusCode::OK);

    let bind_b = Request::builder()
        .uri("/api/admin/models/deepseek-r1/backends")
        .method("POST")
        .header("authorization", format!("Bearer {}", admin_token))
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({
                "backend_id": "be-b",
                "weight": 1,
                "priority": 1
            })
            .to_string(),
        ))
        .unwrap();
    let res_bind_b = app.clone().oneshot(bind_b).await.unwrap();
    assert_eq!(res_bind_b.status(), StatusCode::OK);

    // 步骤 3: 开发者登录自服务门户生成虚拟 API Key
    let dev_login_req = Request::builder()
        .uri("/api/auth/login")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"username":"dev_alice","password":"DevPassword123!"}"#,
        ))
        .unwrap();
    let dev_login_res = app.clone().oneshot(dev_login_req).await.unwrap();
    assert_eq!(dev_login_res.status(), StatusCode::OK);
    let dev_bytes = axum::body::to_bytes(dev_login_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let dev_json: serde_json::Value = serde_json::from_slice(&dev_bytes).unwrap();
    let dev_token = dev_json["data"]["token"].as_str().unwrap().to_string();

    let create_key_req = Request::builder()
        .uri("/api/user/keys")
        .method("POST")
        .header("authorization", format!("Bearer {}", dev_token))
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({
                "name": "Alice Key",
                "allowed_models": ["deepseek-r1"],
                "rpm_limit": 60,
                "max_concurrency": 10,
                "daily_token_quota": 100000,
                "weekly_token_quota": 500000
            })
            .to_string(),
        ))
        .unwrap();
    let key_res = app.clone().oneshot(create_key_req).await.unwrap();
    assert_eq!(key_res.status(), StatusCode::OK);
    let key_bytes = axum::body::to_bytes(key_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let key_json: serde_json::Value = serde_json::from_slice(&key_bytes).unwrap();
    let virtual_key = key_json["data"]["plaintext_key"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(virtual_key.starts_with("sk-gate-"));

    // 步骤 4 & 5: 开发者使用生成的 Key 发起流式 SSE 聊天请求，验证静默重试与故障转移
    let chat_req = Request::builder()
        .uri("/v1/chat/completions")
        .method("POST")
        .header("authorization", format!("Bearer {}", virtual_key))
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({
                "model": "deepseek-r1",
                "stream": true,
                "messages": [{"role": "user", "content": "Hello"}]
            })
            .to_string(),
        ))
        .unwrap();

    let chat_res = app.clone().oneshot(chat_req).await.unwrap();
    assert_eq!(chat_res.status(), StatusCode::OK);
    assert_eq!(
        chat_res.headers().get("content-type").unwrap(),
        "text/event-stream"
    );
    assert_eq!(
        chat_res.headers().get("x-gateway-retry-attempts").unwrap(),
        "1"
    );
    let chat_bytes = axum::body::to_bytes(chat_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let chat_stream_text = String::from_utf8_lossy(&chat_bytes);
    assert!(chat_stream_text.contains("Hello world"));
    assert!(chat_stream_text.contains("[DONE]"));

    // 步骤 6: 验证批量审计 Worker 异步刷盘
    let mut audit_found = false;
    for _ in 0..20 {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        let count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM audit_logs WHERE model = 'deepseek-r1';")
                .fetch_one(&harness.pool)
                .await
                .unwrap();
        if count.0 > 0 {
            audit_found = true;
            break;
        }
    }
    assert!(
        audit_found,
        "Audit worker should flush audit logs into database"
    );

    // 步骤 7: 超管调用 API 导出审计 CSV 并核对内容
    let export_req = Request::builder()
        .uri("/api/admin/audits/export?start_time=0")
        .method("GET")
        .header("authorization", format!("Bearer {}", admin_token))
        .body(Body::empty())
        .unwrap();
    let export_res = app.clone().oneshot(export_req).await.unwrap();
    assert_eq!(export_res.status(), StatusCode::OK);
    assert_eq!(
        export_res.headers().get("content-type").unwrap(),
        "text/csv; charset=utf-8"
    );
    let export_bytes = axum::body::to_bytes(export_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let csv_content = String::from_utf8_lossy(&export_bytes);
    assert!(csv_content.contains("deepseek-r1"));
    assert!(csv_content.contains("200"));
}
