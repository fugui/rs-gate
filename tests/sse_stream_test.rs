use rs_gate::proxy::guard::ActiveConnectionGuard;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[tokio::test]
async fn test_active_connection_guard_raii_drop() {
    let counter = Arc::new(AtomicUsize::new(0));
    {
        counter.fetch_add(1, Ordering::SeqCst);
        let _guard = ActiveConnectionGuard::new(counter.clone());
        assert_eq!(counter.load(Ordering::SeqCst), 1);
        // _guard 在作用域结束时自动 Drop
    }
    assert_eq!(
        counter.load(Ordering::SeqCst),
        0,
        "Counter must auto-decrement on Drop"
    );
}

#[tokio::test]
async fn test_sse_incremental_usage_parsing() {
    let chunk1 = "data: {\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}\n\n";
    let chunk2 = "data: {\"choices\":[],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":5,\"total_tokens\":15}}\n\n";
    let chunk3 = "data: [DONE]\n\n";

    let mut parser = rs_gate::proxy::stream::SseUsageParser::new();
    parser.feed_chunk(chunk1.as_bytes());
    assert!(parser.extract_usage().is_none());

    parser.feed_chunk(chunk2.as_bytes());
    let usage = parser
        .extract_usage()
        .expect("Usage must be parsed from chunk2");
    assert_eq!(usage.prompt_tokens, 10);
    assert_eq!(usage.completion_tokens, 5);
    assert_eq!(usage.total_tokens, 15);

    parser.feed_chunk(chunk3.as_bytes());
    // Usage should still be retained after stream finishes
    let usage_after_done = parser.extract_usage().expect("Usage retained after done");
    assert_eq!(usage_after_done.total_tokens, 15);
}
