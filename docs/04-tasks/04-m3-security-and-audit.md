# rs-gate M3 作战卡：安全流控与重试审计管道 (Security Governance & Retry Pipeline)

- **文档编号**：`04-m3-security-and-audit.md`
- **文档版本**：`v1.0.0`
- **编制日期**：2026-10-19
- **当前阶段**：阶段三：接口契约与任务分解 (第 4 步：M3 独立作战卡篇)
- **基准依赖**：
  - 实施总纲：[`docs/04-tasks/01-roadmap.md`](file:///home/fugui/rusts/rs-gate/docs/04-tasks/01-roadmap.md)
  - 架构设计：[`docs/02-design/01-system-architecture.md`](file:///home/fugui/rusts/rs-gate/docs/02-design/01-system-architecture.md), [`docs/02-design/04-limiting-and-audit.md`](file:///home/fugui/rusts/rs-gate/docs/02-design/04-limiting-and-audit.md)
  - 接口契约：[`docs/03-api/01-gateway-api.md`](file:///home/fugui/rusts/rs-gate/docs/03-api/01-gateway-api.md), [`docs/03-api/03-database-schema.md`](file:///home/fugui/rusts/rs-gate/docs/03-api/03-database-schema.md)
- **状态**：定稿待审 (Ready for Review)

---

## 1. 本里程碑目标与文件变更树 (Target & File Tree)

### 1.1 核心目标与范围边界
本阶段聚焦于构建企业级多租户安全防护、多维度精细化流控配额治理以及透明重试与异步审计流水线：
1. **虚拟 Key 安全鉴权与白名单过滤**：内存加载 SHA-256 加盐 Hash 校验、CIDR IP 白名单精准匹配、逻辑模型白名单控制；
2. **多级高性能无锁流控**：基于 `AtomicU64` CAS 无锁令牌桶实现毫秒级 RPM 平滑限流；基于 `AtomicUsize` 实现连接级在途并发限额拦截；
3. **日/周双周期 Token 物理硬配额引擎**：对齐自然日午夜与周日午夜自动轮转清零，支持流式请求 500 Token 预留准入门限控制；
4. **上游静默重试与故障转移引擎**：针对上游 5xx / 429 / 连接超时 / 连接重置，支持最大 2 次自适应退避与 Full Jitter 重试；优先向健康候选池进行 Failover；
5. **流式重试安全红线控制**：首字前（Pre-first-chunk）支持静默重试与节点切换；首字吐出后（Mid-stream）坚决禁止重发，执行安全优雅中止并下发 SSE 错误帧；
6. **高吞吐异步审计流水管道**：基于 Tokio mpsc 10,000 有界通道解耦审计投递，独立后台 Worker 触发 500ms / 100 条批量事务落盘 `audit_logs` 表，同步原子更新 `quota_usages`。

### 1.2 涉及文件变更清单

```text
rs-gate/
├── src/
│   ├── auth/
│   │   ├── mod.rs                  # [新增] 统一网关鉴权中间件
│   │   ├── key.rs                  # [新增] 虚拟 Key 内存缓存与 SHA-256 加盐匹配
│   │   └── ip.rs                   # [新增] CIDR IP 白名单解析与匹配器
│   ├── limiting/
│   │   ├── mod.rs                  # [新增] 多级流控中间件组装
│   │   ├── token_bucket.rs         # [新增] AtomicU64 CAS 无锁令牌桶 (RPM)
│   │   ├── concurrency.rs          # [新增] AtomicUsize 瞬时并发门限控制器
│   │   └── quota.rs                # [新增] 日/周双周期 Token 配额与午夜轮转引擎
│   ├── proxy/
│   │   └── retry.rs                # [新增] 上游静默重试、Full Jitter 退避与 Failover
│   └── audit/
│       ├── mod.rs                  # [新增] 审计事件通道与模型定义
│       └── worker.rs               # [新增] 批量事务落盘 Worker 与配额原子同步
└── tests/
    ├── auth_middleware_test.rs     # [新增] 非法 Key/过期/CIDR拒绝/模型越权测试
    ├── rate_limit_test.rs          # [新增] 高频击发 RPM 限流与并发满载 429 测试
    ├── quota_limit_test.rs         # [新增] 日/周配额超额拦截与跨周期自动重置测试
    ├── retry_failover_test.rs      # [新增] 上游节点故障静默切换与重试计数回显测试
    ├── stream_retry_boundary_test.rs # [新增] 首字前重试与首字后优雅中止红线测试
    └── audit_batch_worker_test.rs  # [新增] 异步审计高并发批量事务落盘完整性测试
```

---

## 2. 测试夹具与 TDD 先行用例 (TDD Test Harness & Failing Tests - Red Phase)

### 2.1 鉴权与安全访问控制测试 (`tests/auth_middleware_test.rs`)

```rust
use axum::{body::Body, http::{Request, StatusCode}};
use tower::ServiceExt;

#[tokio::test]
async fn test_invalid_key_returns_401() {
    let app = rs_gate::server::build_test_app().await;
    let req = Request::builder()
        .uri("/v1/chat/completions")
        .method("POST")
        .header("authorization", "Bearer sk-gate-invalidkey000000000000000000")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"model":"deepseek-r1"}"#))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let err_json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(err_json["error"]["type"], "authentication_error");
}

#[tokio::test]
async fn test_cidr_ip_whitelist_rejected_returns_403() {
    let app = rs_gate::server::build_test_app_with_ip_whitelist("192.168.1.0/24").await;
    let req = Request::builder()
        .uri("/v1/chat/completions")
        .method("POST")
        .header("authorization", "Bearer sk-gate-validkey00000000000000000000")
        .header("x-forwarded-for", "10.0.0.1") // 不在白名单网段内
        .header("content-type", "application/json")
        .body(Body::from(r#"{"model":"deepseek-r1"}"#))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}
```

### 2.2 令牌桶 RPM 与并发限流测试 (`tests/rate_limit_test.rs`)

```rust
use rs_gate::limiting::token_bucket::AtomicTokenBucket;
use std::time::Duration;

#[test]
fn test_atomic_token_bucket_cas_rate_limiting() {
    // 限制每分钟 60 次 (1 次/秒)，初始容量 2
    let bucket = AtomicTokenBucket::new(60, 2);
    
    assert!(bucket.try_acquire(1), "First request should pass");
    assert!(bucket.try_acquire(1), "Second request should pass");
    assert!(!bucket.try_acquire(1), "Third request must be rate limited");
    
    std::thread::sleep(Duration::from_millis(1050));
    assert!(bucket.try_acquire(1), "Token replenished after 1 second");
}
```

### 2.3 日/周配额超额拦截与跨期重置测试 (`tests/quota_limit_test.rs`)

```rust
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
    assert!(quota_engine.check_admission(key_id, 500).is_err(), "Must reject due to insufficient daily quota");

    // 跨过午夜模拟清零
    quota_engine.simulate_midnight_rollover();
    assert!(quota_engine.check_admission(key_id, 500).is_ok(), "Must pass after midnight rollover");
}
```

### 2.4 上游静默重试与故障转移测试 (`tests/retry_failover_test.rs`)

```rust
use wiremock::{MockServer, Mock, ResponseTemplate, matchers::{method, path}};
use axum::{body::Body, http::{Request, StatusCode}};
use tower::ServiceExt;

#[tokio::test]
async fn test_silent_retry_and_failover_success() {
    let server_a = MockServer::start().await;
    let server_b = MockServer::start().await;

    // 节点 A 模拟 503 临时故障
    Mock::given(method("POST")).and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server_a).await;

    // 节点 B 模拟 200 正常响应
    Mock::given(method("POST")).and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(r#"{"choices":[]}"#, "application/json"))
        .mount(&server_b).await;

    let app = rs_gate::server::build_test_app_with_cluster(vec![server_a.uri(), server_b.uri()]).await;

    let req = Request::builder()
        .uri("/v1/chat/completions")
        .method("POST")
        .header("authorization", "Bearer sk-gate-validkey00000000000000000000")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"model":"deepseek-r1","messages":[]}"#))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    // 断言响应头明确回显重试 1 次
    assert_eq!(res.headers().get("x-gateway-retry-attempts").unwrap(), "1");
}
```

### 2.5 流式重试安全红线测试 (`tests/stream_retry_boundary_test.rs`)

```rust
#[tokio::test]
async fn test_stream_pre_first_chunk_retry_vs_mid_stream_abort() {
    // 场景 1: 首字未吐出前上游断开 -> 网关静默切换节点成功输出完整流
    // 场景 2: 首字吐出后上游连接断开 -> 网关坚决不重试，下发 SSE 错误帧与 [DONE]，防止 Token 重复
}
```

### 2.6 异步审计批量落盘一致性测试 (`tests/audit_batch_worker_test.rs`)

```rust
use rs_gate::audit::{AuditEvent, AuditLogger};
use crate::common::TestDbHarness;

#[tokio::test]
async fn test_audit_batch_flush_worker() {
    let harness = TestDbHarness::new().await.unwrap();
    rs_gate::storage::migration::run_migrations(&harness.pool).await.unwrap();

    let (logger, worker) = AuditLogger::new_test(harness.pool.clone(), 100);
    tokio::spawn(worker.run());

    // 并发投递 250 条审计事件
    for i in 0..250 {
        logger.send(AuditEvent::dummy(i)).await.unwrap();
    }

    // 等待批量刷新周期 (500ms)
    tokio::time::sleep(std::time::Duration::from_millis(700)).await;

    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM audit_logs;")
        .fetch_one(&harness.pool)
        .await
        .unwrap();

    assert_eq!(count.0, 250, "All 250 audit logs must be persisted in batch");
}
```

---

## 3. 分步工程实施任务 (Step-by-Step Implementation - Green Phase)

### 3.1 Task 3.1：虚拟 Key 鉴权与白名单过滤中间件
- **目标文件**：`src/auth/key.rs`, `src/auth/ip.rs`, `src/auth/mod.rs`
- **实现要点**：
  - 启动或配置刷新时，预先从 SQLite 读取活跃虚拟 Key，计算 SHA-256 哈希并在内存维护只读映射（Key Prefix -> Hash Record）；
  - 入站提取 `Authorization: Bearer sk-gate-...`，校验前缀后计算 SHA-256 加盐 Hash 进行恒定时间比较（`subtle::ConstantTimeEq`）；
  - CIDR 白名单匹配：使用 `ipnet::IpNet` 解析配置，支持多网段逗号分隔；
  - 鉴权成功后将 `AuthContext { key_id, user_id, rpm_limit, ... }` 注入请求 Extension。

### 3.2 Task 3.2：基于 CAS 的 AtomicU64 无锁令牌桶与并发门限
- **目标文件**：`src/limiting/token_bucket.rs`, `src/limiting/concurrency.rs`, `src/limiting/mod.rs`
- **核心算法**：
```rust
pub struct AtomicTokenBucket {
    // 高 32 位存储最近刷新时间戳 (秒)，低 32 位存储当前可用令牌数
    state: std::sync::atomic::AtomicU64,
    rate_per_sec: u32,
    burst_capacity: u32,
}

impl AtomicTokenBucket {
    pub fn try_acquire(&self, tokens: u32) -> bool {
        // CAS 循环原子计算时间差并补齐令牌
        // 成功扣减返回 true，否则返回 false
    }
}
```
- **并发门限**：使用 `AtomicUsize`，超过 `max_concurrency` 立即拦截并返回 429 与 `retry-after: 1`。

### 3.3 Task 3.3：日/周双周期配额与午夜对齐引擎
- **目标文件**：`src/limiting/quota.rs`
- **实现要点**：
  - 内存缓存当天已消耗总 Token 与本周已消耗总 Token；
  - 准入规则：`daily_quota == 0` 表示不限额；若 `daily_quota > 0` 且 `used + 500 > daily_quota`，直接返回 429 `Daily quota exceeded`；
  - 后台轻量任务或每次请求前懒检查自然日（UTC+8 或配置时区）是否翻转，跨天自动重置日计数器，跨周自动重置周计数器。

### 3.4 Task 3.4：上游静默重试、Full Jitter 退避与 Failover 状态机
- **目标文件**：`src/proxy/retry.rs`
- **核心规则**：
  - 判定重试条件：上游返回 HTTP 500/502/503/504、HTTP 429、TCP 连接重置、网络超时；
  - 最多重试 2 次（总尝试 3 次）；
  - 选路优先级：优先从可用后端列表中调度下一个未尝试过的健康节点（Failover）；无备用节点时原地退避；
  - 退避算法：$T_{\text{wait}} = \text{random}(0, \min(\text{max\_interval}, \text{base} \times 2^{\text{attempt}}))$；
  - 出站透传头：注入 `X-Gateway-Retry-Attempts: N`。

### 3.5 Task 3.5：流式重试首字分水岭机制
- **目标文件**：`src/proxy/stream.rs`
- **实现逻辑**：
  - 首字到达前（Pre-first-chunk）：管道尚未向下游发送任何 HTTP 响应头或字节块，若捕获上游错误/断连，直接进入 `retry.rs` 重新选路转发；
  - 首字到达后（Mid-stream）：已向下游发送 HTTP 200 OK 与部分 SSE Token，此时严禁切换节点或重置流。网关向下游追加发送一条标准 SSE 错误事件与 `data: [DONE]\n\n`，关闭下游连接。

### 3.6 Task 3.6：Tokio MPSC 有界通道与批量事务落盘 Worker
- **目标文件**：`src/audit/mod.rs`, `src/audit/worker.rs`
- **实现要点**：
  - 定义 `AuditEvent` 结构体，涵盖全链路 `trace_id`, `key_id`, `ttft_ms`, `tokens`, `retry_count`, `failover_path` 等 18 项指标；
  - 网关完成请求时通过 `tokio::sync::mpsc::Sender::try_send` 异步投递事件，通道满载时记录丢弃度量，严禁反压阻塞代理核心路径；
  - 后台 Worker 采用 `tokio::select!` 监听定时器（500ms）或缓冲区满（100 条）：
    - 开启 SQLite 独占事务；
    - 执行批量 `INSERT INTO audit_logs VALUES (...)`；
    - 同步执行 `INSERT INTO quota_usages ... ON CONFLICT(key_id, period_type, period_start) DO UPDATE SET total_tokens = total_tokens + ?`；
    - 提交事务。

---

## 4. 自动化验收标准与命令行验证手册 (Automated AC & CLI Playbook)

### 4.1 单元与集成测试自动化验证

```bash
# 1. 运行鉴权与白名单安全测试
cargo test --test auth_middleware_test -- --nocapture

# 2. 运行令牌桶与并发流控测试
cargo test --test rate_limit_test -- --nocapture

# 3. 运行日/周配额拦截与重置测试
cargo test --test quota_limit_test -- --nocapture

# 4. 运行静默重试与故障转移测试
cargo test --test retry_failover_test -- --nocapture

# 5. 运行流式重试分水岭安全红线测试
cargo test --test stream_retry_boundary_test -- --nocapture

# 6. 运行异步审计批量事务落盘测试
cargo test --test audit_batch_worker_test -- --nocapture
```

**验收门禁通过准则**：
- [ ] 鉴权非法 Key 返回 401，CIDR 越界返回 403，未授权模型返回 403；
- [ ] 突发高频请求平滑触发 429 且带有合规 `Retry-After` 头；
- [ ] 超额配额请求在网关入口准入门限被安全阻断，不产生物理上游流量；
- [ ] 上游 503 场景下网关透明切换节点并输出 200，回显 `X-Gateway-Retry-Attempts: 1`；
- [ ] 流式输出在首字后断连时杜绝二次生成；
- [ ] 批量审计 Worker 在高并发投递下无数据丢失，配额表与日志表严格一致。

### 4.2 命令行黑盒验证手册 (CLI Blackbox Playbook)

```bash
# 验证触发限流
for i in {1..10}; do
  curl -s -o /dev/null -w "%{http_code}\n" -X POST http://localhost:8080/v1/chat/completions \
    -H "Authorization: Bearer sk-gate-validkey00000000000000000000" \
    -H "Content-Type: application/json" \
    -d '{"model":"deepseek-r1","messages":[{"role":"user","content":"hi"}]}'
done
```
