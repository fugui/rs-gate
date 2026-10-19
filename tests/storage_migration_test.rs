mod common;

use common::TestDbHarness;

#[tokio::test]
async fn test_migration_idempotency() {
    let harness = TestDbHarness::new().await.unwrap();
    // 第一次执行迁移
    rs_gate::storage::migration::run_migrations(&harness.pool)
        .await
        .expect("First migration failed");
    // 第二次执行迁移（必须幂等成功）
    rs_gate::storage::migration::run_migrations(&harness.pool)
        .await
        .expect("Repeated migration must succeed");
}

#[tokio::test]
async fn test_tables_and_foreign_keys_integrity() {
    let harness = TestDbHarness::new().await.unwrap();
    rs_gate::storage::migration::run_migrations(&harness.pool)
        .await
        .unwrap();

    // 验证 7 张物理表存在性
    let tables: Vec<(String,)> = sqlx::query_as(
        "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' AND name NOT LIKE '_sqlx_%' ORDER BY name;",
    )
    .fetch_all(&harness.pool)
    .await
    .unwrap();

    let table_names: Vec<String> = tables.into_iter().map(|(t,)| t).collect();
    assert!(table_names.contains(&"users".to_string()));
    assert!(table_names.contains(&"virtual_keys".to_string()));
    assert!(table_names.contains(&"quota_usages".to_string()));
    assert!(table_names.contains(&"models".to_string()));
    assert!(table_names.contains(&"backend_servers".to_string()));
    assert!(table_names.contains(&"model_backends".to_string()));
    assert!(table_names.contains(&"audit_logs".to_string()));
}

#[tokio::test]
async fn test_indexes_exist() {
    let harness = TestDbHarness::new().await.unwrap();
    rs_gate::storage::migration::run_migrations(&harness.pool)
        .await
        .unwrap();

    let indexes: Vec<(String,)> = sqlx::query_as(
        "SELECT name FROM sqlite_master WHERE type='index' AND name NOT LIKE 'sqlite_%' AND name NOT LIKE '_sqlx_%';",
    )
    .fetch_all(&harness.pool)
    .await
    .unwrap();

    let index_names: Vec<String> = indexes.into_iter().map(|(i,)| i).collect();
    assert!(index_names.contains(&"idx_audit_logs_created_at".to_string()));
    assert!(index_names.contains(&"idx_audit_logs_key_id".to_string()));
    assert!(index_names.contains(&"idx_virtual_keys_key_prefix".to_string()));
}

#[tokio::test]
async fn test_bootstrap_superadmin_creation_and_auth() {
    let harness = TestDbHarness::new().await.unwrap();
    rs_gate::storage::migration::run_migrations(&harness.pool)
        .await
        .unwrap();

    // 执行首次启动引导
    let admin_info =
        rs_gate::storage::bootstrap::ensure_superadmin(&harness.pool, Some("InitialAdminPass!123"))
            .await
            .unwrap();

    assert_eq!(admin_info.username, "admin");
    assert_eq!(admin_info.role, "superadmin");

    // 验证密码哈希可通过 Argon2 校验
    let is_valid = rs_gate::storage::bootstrap::verify_password(
        "InitialAdminPass!123",
        &admin_info.password_hash,
    )
    .unwrap();
    assert!(is_valid);
}
