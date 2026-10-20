mod common;
use common::TestDbHarness;
use rs_gate::audit::retention::RetentionCleaner;

#[tokio::test]
async fn test_retention_cleanup_removes_expired_logs() {
    let harness = TestDbHarness::new().await.unwrap();
    rs_gate::storage::migration::run_migrations(&harness.pool)
        .await
        .unwrap();

    let now_ms = chrono::Utc::now().timestamp_millis();
    let expired_ms = now_ms - (35 * 86400 * 1000); // 35 天前（已超 30 天保留期）

    // 插入一条超期日志和一条近期日志
    sqlx::query(
        "INSERT INTO audit_logs (id, trace_id, key_id, model, backend_id, protocol, client_ip, http_status, request_duration_ms, is_stream, created_at) \
         VALUES ('log-expired', 't1', 'k1', 'm1', 'b1', 'chat', '127.0.0.1', 200, 100, 0, ?), \
                ('log-fresh', 't2', 'k1', 'm1', 'b1', 'chat', '127.0.0.1', 200, 100, 0, ?);"
    )
    .bind(expired_ms)
    .bind(now_ms)
    .execute(&harness.pool)
    .await
    .unwrap();

    // 执行清理任务（保留 30 天）
    let cleaner = RetentionCleaner::new(harness.pool.clone(), 30);
    let purged_count = cleaner.clean_expired_logs().await.unwrap();
    assert_eq!(purged_count, 1, "Must purge exactly 1 expired log");

    // 验证近期日志安然无恙
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM audit_logs WHERE id = 'log-fresh';")
        .fetch_one(&harness.pool)
        .await
        .unwrap();
    assert_eq!(count.0, 1);
}
