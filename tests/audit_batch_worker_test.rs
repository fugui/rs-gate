mod common;

use common::TestDbHarness;
use rs_gate::audit::{AuditEvent, AuditLogger};

#[tokio::test]
async fn test_audit_batch_flush_worker() {
    let harness = TestDbHarness::new().await.unwrap();
    rs_gate::storage::migration::run_migrations(&harness.pool)
        .await
        .unwrap();

    let (logger, worker) = AuditLogger::new_test(harness.pool.clone(), 100);
    tokio::spawn(worker.run());

    // 并发投递 250 条审计事件
    for i in 0..250 {
        logger.send(AuditEvent::dummy(i)).await.unwrap();
    }

    // 等待批量刷新周期 (500ms)
    tokio::time::sleep(std::time::Duration::from_millis(800)).await;

    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM audit_logs;")
        .fetch_one(&harness.pool)
        .await
        .unwrap();

    assert_eq!(
        count.0, 250,
        "All 250 audit logs must be persisted in batch"
    );
}
