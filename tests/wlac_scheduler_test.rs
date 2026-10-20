use rs_gate::routing::backend::BackendInstance;
use rs_gate::routing::scheduler::select_wlac_backend;
use std::sync::atomic::Ordering;

#[test]
fn test_wlac_cross_multiplication_selection() {
    // 节点 A: 权重 10，活跃连接 2 (负载比 2/10 = 0.2)
    // 节点 B: 权重 20，活跃连接 3 (负载比 3/20 = 0.15) -> 理论上更空闲，应选 B
    let backend_a = BackendInstance::new("be-a", "http://127.0.0.1:8001", 10);
    let backend_b = BackendInstance::new("be-b", "http://127.0.0.1:8002", 20);

    backend_a.active_connections.store(2, Ordering::SeqCst);
    backend_b.active_connections.store(3, Ordering::SeqCst);

    let candidates = vec![backend_a.clone(), backend_b.clone()];
    let selected = select_wlac_backend(&candidates).expect("Must select a backend");

    assert_eq!(
        selected.id, "be-b",
        "Should pick backend B due to 3*10 < 2*20"
    );
}

#[test]
fn test_wlac_tie_breaking_round_robin() {
    // 节点 A: 权重 10，活跃连接 1 (1/10 = 0.1)
    // 节点 B: 权重 20，活跃连接 2 (2/20 = 0.1) -> 负载比相同，平局打破
    let backend_a = BackendInstance::new("be-a", "http://127.0.0.1:8001", 10);
    let backend_b = BackendInstance::new("be-b", "http://127.0.0.1:8002", 20);

    backend_a.active_connections.store(1, Ordering::SeqCst);
    backend_b.active_connections.store(2, Ordering::SeqCst);

    let candidates = vec![backend_a.clone(), backend_b.clone()];
    let first = select_wlac_backend(&candidates).unwrap();
    let second = select_wlac_backend(&candidates).unwrap();

    assert_ne!(
        first.id, second.id,
        "Tie must be broken smoothly by round robin"
    );
}

#[test]
fn test_wlac_empty_candidates() {
    let candidates: Vec<BackendInstance> = vec![];
    assert!(select_wlac_backend(&candidates).is_none());
}

#[test]
fn test_wlac_unhealthy_backend_skipped() {
    let backend_a = BackendInstance::new("be-a", "http://127.0.0.1:8001", 10);
    let backend_b = BackendInstance::new("be-b", "http://127.0.0.1:8002", 20);

    // be-a has 0 active connections but is unhealthy
    backend_a.is_healthy.store(false, Ordering::SeqCst);
    backend_b.active_connections.store(5, Ordering::SeqCst);

    let candidates = vec![backend_a.clone(), backend_b.clone()];
    let selected = select_wlac_backend(&candidates).expect("Must select healthy backend");

    assert_eq!(selected.id, "be-b", "Unhealthy backend must be skipped");
}
