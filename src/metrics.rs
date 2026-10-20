use std::collections::HashMap;
use std::fmt::Write;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::{LazyLock, RwLock};

static ACTIVE_CONNECTIONS: AtomicI64 = AtomicI64::new(0);

const DURATION_BUCKETS: &[f64] = &[
    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
];

const TTFT_BUCKETS: &[f64] = &[0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.0, 5.0];

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct RequestMetricKey {
    model: String,
    status: u16,
    is_stream: bool,
}

#[derive(Debug)]
struct HistogramMetric {
    buckets: Vec<f64>,
    bucket_counts: Vec<AtomicU64>,
    sum_bits: AtomicU64,
    count: AtomicU64,
}

impl HistogramMetric {
    fn new(buckets: &[f64]) -> Self {
        let mut sorted = buckets.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let bucket_counts = (0..sorted.len()).map(|_| AtomicU64::new(0)).collect();

        Self {
            buckets: sorted,
            bucket_counts,
            sum_bits: AtomicU64::new(0),
            count: AtomicU64::new(0),
        }
    }

    fn observe(&self, val: f64) {
        for (i, upper) in self.buckets.iter().enumerate() {
            if val <= *upper {
                self.bucket_counts[i].fetch_add(1, Ordering::Relaxed);
            }
        }
        self.count.fetch_add(1, Ordering::Relaxed);

        // Atomic float sum update using CAS loop
        let mut current = self.sum_bits.load(Ordering::Relaxed);
        loop {
            let current_f64 = f64::from_bits(current);
            let new_f64 = current_f64 + val;
            let new_bits = new_f64.to_bits();
            match self.sum_bits.compare_exchange_weak(
                current,
                new_bits,
                Ordering::SeqCst,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => current = actual,
            }
        }
    }

    fn get_sum(&self) -> f64 {
        f64::from_bits(self.sum_bits.load(Ordering::Relaxed))
    }

    fn get_count(&self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }
}

struct MetricRegistry {
    requests: RwLock<HashMap<RequestMetricKey, AtomicU64>>,
    retries: RwLock<HashMap<String, AtomicU64>>,
    durations: RwLock<HashMap<String, HistogramMetric>>,
    ttfts: RwLock<HashMap<String, HistogramMetric>>,
}

impl MetricRegistry {
    fn new() -> Self {
        Self {
            requests: RwLock::new(HashMap::new()),
            retries: RwLock::new(HashMap::new()),
            durations: RwLock::new(HashMap::new()),
            ttfts: RwLock::new(HashMap::new()),
        }
    }
}

static REGISTRY: LazyLock<MetricRegistry> = LazyLock::new(MetricRegistry::new);

/// 递增网关长连接活跃计数
pub fn inc_active_connections() {
    ACTIVE_CONNECTIONS.fetch_add(1, Ordering::SeqCst);
}

/// 递减网关长连接活跃计数
pub fn dec_active_connections() {
    ACTIVE_CONNECTIONS.fetch_sub(1, Ordering::SeqCst);
}

/// 获取当前网关活跃长连接数
pub fn get_active_connections() -> usize {
    ACTIVE_CONNECTIONS.load(Ordering::Relaxed).max(0) as usize
}

/// 记录请求完成计数
pub fn record_request(model: &str, status: u16, is_stream: bool) {
    let key = RequestMetricKey {
        model: model.to_string(),
        status,
        is_stream,
    };

    if let Ok(map) = REGISTRY.requests.read() {
        if let Some(counter) = map.get(&key) {
            counter.fetch_add(1, Ordering::Relaxed);
            return;
        }
    }

    if let Ok(mut map) = REGISTRY.requests.write() {
        let counter = map.entry(key).or_insert_with(|| AtomicU64::new(0));
        counter.fetch_add(1, Ordering::Relaxed);
    }
}

/// 记录静默重试次数
pub fn record_retry(reason: &str) {
    if let Ok(map) = REGISTRY.retries.read() {
        if let Some(counter) = map.get(reason) {
            counter.fetch_add(1, Ordering::Relaxed);
            return;
        }
    }

    if let Ok(mut map) = REGISTRY.retries.write() {
        let counter = map
            .entry(reason.to_string())
            .or_insert_with(|| AtomicU64::new(0));
        counter.fetch_add(1, Ordering::Relaxed);
    }
}

/// 记录请求处理耗时 (秒)
pub fn record_request_duration(model: &str, duration_secs: f64) {
    if let Ok(map) = REGISTRY.durations.read() {
        if let Some(hist) = map.get(model) {
            hist.observe(duration_secs);
            return;
        }
    }

    if let Ok(mut map) = REGISTRY.durations.write() {
        let hist = map
            .entry(model.to_string())
            .or_insert_with(|| HistogramMetric::new(DURATION_BUCKETS));
        hist.observe(duration_secs);
    }
}

/// 记录首字透传延迟 TTFT (秒)
pub fn record_ttft(model: &str, ttft_secs: f64) {
    if let Ok(map) = REGISTRY.ttfts.read() {
        if let Some(hist) = map.get(model) {
            hist.observe(ttft_secs);
            return;
        }
    }

    if let Ok(mut map) = REGISTRY.ttfts.write() {
        let hist = map
            .entry(model.to_string())
            .or_insert_with(|| HistogramMetric::new(TTFT_BUCKETS));
        hist.observe(ttft_secs);
    }
}

/// 导出 Prometheus 标准纯文本度量数据
pub fn export_metrics_text() -> String {
    let mut out = String::with_capacity(4096);

    // 1. rsgate_requests_total
    out.push_str("# HELP rsgate_requests_total Total number of HTTP proxy requests processed\n");
    out.push_str("# TYPE rsgate_requests_total counter\n");
    if let Ok(requests) = REGISTRY.requests.read() {
        for (key, count) in requests.iter() {
            let val = count.load(Ordering::Relaxed);
            let _ = writeln!(
                out,
                r#"rsgate_requests_total{{model="{}",status="{}",is_stream="{}"}} {}"#,
                key.model, key.status, key.is_stream, val
            );
        }
    }

    // 2. rsgate_active_connections
    out.push_str("# HELP rsgate_active_connections Current active in-flight proxy connections\n");
    out.push_str("# TYPE rsgate_active_connections gauge\n");
    let active = get_active_connections();
    let _ = writeln!(out, "rsgate_active_connections {}", active);

    // 3. rsgate_retry_attempts_total
    out.push_str("# HELP rsgate_retry_attempts_total Total silent retry attempts performed\n");
    out.push_str("# TYPE rsgate_retry_attempts_total counter\n");
    if let Ok(retries) = REGISTRY.retries.read() {
        for (reason, count) in retries.iter() {
            let val = count.load(Ordering::Relaxed);
            let _ = writeln!(
                out,
                r#"rsgate_retry_attempts_total{{reason="{}"}} {}"#,
                reason, val
            );
        }
    }

    // 4. rsgate_request_duration_seconds
    out.push_str("# HELP rsgate_request_duration_seconds Gateway request duration in seconds\n");
    out.push_str("# TYPE rsgate_request_duration_seconds histogram\n");
    if let Ok(durations) = REGISTRY.durations.read() {
        for (model, hist) in durations.iter() {
            for (i, upper) in hist.buckets.iter().enumerate() {
                let count = hist.bucket_counts[i].load(Ordering::Relaxed);
                let _ = writeln!(
                    out,
                    r#"rsgate_request_duration_seconds_bucket{{model="{}",le="{}"}} {}"#,
                    model, upper, count
                );
            }
            let total_count = hist.get_count();
            let _ = writeln!(
                out,
                r#"rsgate_request_duration_seconds_bucket{{model="{}",le="+Inf"}} {}"#,
                model, total_count
            );
            let _ = writeln!(
                out,
                r#"rsgate_request_duration_seconds_sum{{model="{}"}} {}"#,
                model,
                hist.get_sum()
            );
            let _ = writeln!(
                out,
                r#"rsgate_request_duration_seconds_count{{model="{}"}} {}"#,
                model, total_count
            );
        }
    }

    // 5. rsgate_ttft_seconds
    out.push_str("# HELP rsgate_ttft_seconds Time to first token in seconds\n");
    out.push_str("# TYPE rsgate_ttft_seconds histogram\n");
    if let Ok(ttfts) = REGISTRY.ttfts.read() {
        for (model, hist) in ttfts.iter() {
            for (i, upper) in hist.buckets.iter().enumerate() {
                let count = hist.bucket_counts[i].load(Ordering::Relaxed);
                let _ = writeln!(
                    out,
                    r#"rsgate_ttft_seconds_bucket{{model="{}",le="{}"}} {}"#,
                    model, upper, count
                );
            }
            let total_count = hist.get_count();
            let _ = writeln!(
                out,
                r#"rsgate_ttft_seconds_bucket{{model="{}",le="+Inf"}} {}"#,
                model, total_count
            );
            let _ = writeln!(
                out,
                r#"rsgate_ttft_seconds_sum{{model="{}"}} {}"#,
                model,
                hist.get_sum()
            );
            let _ = writeln!(
                out,
                r#"rsgate_ttft_seconds_count{{model="{}"}} {}"#,
                model, total_count
            );
        }
    }

    out
}
