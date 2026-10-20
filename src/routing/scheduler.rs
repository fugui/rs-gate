use crate::routing::backend::BackendInstance;
use std::sync::atomic::{AtomicUsize, Ordering};

static GLOBAL_ROUND_ROBIN: AtomicUsize = AtomicUsize::new(0);

/// 加权最小活跃长连接调度器 (Weighted Least-Active-Connections)
pub struct WlacScheduler {
    round_robin_counter: AtomicUsize,
}

impl Default for WlacScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl WlacScheduler {
    pub fn new() -> Self {
        Self {
            round_robin_counter: AtomicUsize::new(0),
        }
    }

    /// 在候选后端节点中执行加权最小活跃长连接选路 (无除法交叉乘法)
    pub fn select_backend<'a>(
        &self,
        backends: &'a [BackendInstance],
    ) -> Option<&'a BackendInstance> {
        select_wlac_with_counter(backends, &self.round_robin_counter)
    }
}

/// 执行 WLAC 调度算法的核心无除法交叉乘法选路
pub fn select_wlac_backend(backends: &[BackendInstance]) -> Option<&BackendInstance> {
    select_wlac_with_counter(backends, &GLOBAL_ROUND_ROBIN)
}

fn select_wlac_with_counter<'a>(
    backends: &'a [BackendInstance],
    rr_counter: &AtomicUsize,
) -> Option<&'a BackendInstance> {
    if backends.is_empty() {
        return None;
    }

    // 过滤出健康的后端
    let healthy_candidates: Vec<&'a BackendInstance> = backends
        .iter()
        .filter(|b| b.is_healthy.load(Ordering::Relaxed))
        .collect();

    if healthy_candidates.is_empty() {
        return None;
    }

    if healthy_candidates.len() == 1 {
        return Some(healthy_candidates[0]);
    }

    let mut best_idx = 0;
    let mut best_active = healthy_candidates[0]
        .active_connections
        .load(Ordering::Relaxed) as u64;
    let mut best_weight = healthy_candidates[0].weight.max(1) as u64;

    let mut tie_indices = Vec::with_capacity(healthy_candidates.len());
    tie_indices.push(0);

    for (i, candidate) in healthy_candidates.iter().enumerate().skip(1) {
        let curr_active = candidate.active_connections.load(Ordering::Relaxed) as u64;
        let curr_weight = candidate.weight.max(1) as u64;

        // 交叉乘法比较:
        // C_curr / W_curr < C_best / W_best <=> C_curr * W_best < C_best * W_curr
        let lhs = curr_active * best_weight;
        let rhs = best_active * curr_weight;

        if lhs < rhs
            || (lhs == rhs && curr_active == 0 && best_active == 0 && curr_weight > best_weight)
        {
            // 当前候选节点负载更轻，或在 0 连接时权重更高
            best_idx = i;
            best_active = curr_active;
            best_weight = curr_weight;
            tie_indices.clear();
            tie_indices.push(i);
        } else if lhs == rhs && (curr_active != 0 || best_active != 0 || curr_weight == best_weight)
        {
            // 负载比严格相同，加入平局候选集合
            tie_indices.push(i);
        }
    }

    let final_idx = if tie_indices.len() > 1 {
        let offset = rr_counter.fetch_add(1, Ordering::Relaxed);
        tie_indices[offset % tie_indices.len()]
    } else {
        best_idx
    };

    Some(healthy_candidates[final_idx])
}
