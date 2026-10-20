use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use futures_util::StreamExt;
use tower::ServiceExt;
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

#[tokio::test]
async fn test_stream_pre_first_chunk_retry_success() {
    let server_a = MockServer::start().await;
    let server_b = MockServer::start().await;

    // Server A fails before stream starts (503)
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server_a)
        .await;

    let sse_body =
        "data: {\"choices\":[{\"delta\":{\"content\":\"Hello stream\"}}]}\n\ndata: [DONE]\n\n";

    // Server B responds with 200 SSE
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_raw(sse_body, "text/event-stream"),
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
        .body(Body::from(
            r#"{"model":"deepseek-r1","stream":true,"messages":[]}"#,
        ))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers().get("x-gateway-retry-attempts").unwrap(), "1");
    let content_type = res.headers().get("content-type").unwrap().to_str().unwrap();
    assert!(content_type.contains("text/event-stream"));

    let body_bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = String::from_utf8_lossy(&body_bytes);
    assert!(body_str.contains("Hello stream"));
}

#[tokio::test]
async fn test_stream_mid_stream_abort_never_retries() {
    let server_a = MockServer::start().await;
    let server_b = MockServer::start().await;

    // Server A returns first chunk
    let first_chunk = "data: {\"choices\":[{\"delta\":{\"content\":\"FirstChunk\"}}]}\n\n";
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_raw(first_chunk, "text/event-stream"),
        )
        .mount(&server_a)
        .await;

    // Server B should NEVER be called mid-stream
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
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
        .body(Body::from(
            r#"{"model":"deepseek-r1","stream":true,"messages":[]}"#,
        ))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let mut stream = res.into_body().into_data_stream();
    let mut total_output = String::new();
    while let Some(chunk_res) = stream.next().await {
        if let Ok(bytes) = chunk_res {
            total_output.push_str(&String::from_utf8_lossy(&bytes));
        }
    }

    assert!(total_output.contains("FirstChunk"));
    // Verify server_b was not invoked
    server_b.verify().await;
}
