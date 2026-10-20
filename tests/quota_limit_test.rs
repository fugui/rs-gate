use rs_gate::limiting::quota::QuotaEngine;

#[tokio::test]
async fn test_quota_exhaustion_and_midnight_reset() {
    let mut quota_engine = QuotaEngine::new_test();
    // 设定日配额为 1000 Token
    let key_id = "test-key-1";
    quota_engine.set_daily_quota(key_id, 1000);

    // 预检准入门限 500
    assert!(quota_engine.check_admission(key_id, 500).is_ok());

    // 消耗 900 Token
    quota_engine.record_usage(key_id, 900).await.unwrap();

    // 再次请求 500 Token，超过剩余配额 (1000 - 900 = 100 < 500)
    assert!(
        quota_engine.check_admission(key_id, 500).is_err(),
        "Must reject due to insufficient daily quota"
    );

    // 跨过午夜模拟清零
    quota_engine.simulate_midnight_rollover();
    assert!(
        quota_engine.check_admission(key_id, 500).is_ok(),
        "Must pass after midnight rollover"
    );
}

#[tokio::test]
async fn test_weekly_quota_exhaustion_and_rollover() {
    let mut quota_engine = QuotaEngine::new_test();
    let key_id = "test-key-weekly";
    quota_engine.set_weekly_quota(key_id, 2000);

    assert!(quota_engine.check_admission(key_id, 500).is_ok());

    // 消耗 1800 Token
    quota_engine.record_usage(key_id, 1800).await.unwrap();

    // 再次请求 500 Token，超过剩余配额 (2000 - 1800 = 200 < 500)
    assert!(quota_engine.check_admission(key_id, 500).is_err());

    // 跨周轮转
    quota_engine.simulate_weekly_rollover();
    assert!(
        quota_engine.check_admission(key_id, 500).is_ok(),
        "Must pass after weekly rollover"
    );
}
