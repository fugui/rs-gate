# rs-gate M2 作战卡：直通代理与 WLAC 调度管道 (Proxy Pipeline & WLAC Scheduler)

- **文档编号**：`03-m2-proxy-and-wlac.md`
- **文档版本**：`v1.0.0`
- **编制日期**：2026-10-19
- **当前阶段**：阶段三：接口契约与任务分解 (第 4 步：M2 独立作战卡篇)
- **基准依赖**：
  - 实施总纲：[`docs/04-tasks/01-roadmap.md`](file:///home/fugui/rusts/rs-gate/docs/04-tasks/01-roadmap.md)
  - 架构设计：[`docs/02-design/01-system-architecture.md`](file:///home/fugui/rusts/rs-gate/docs/02-design/01-system-architecture.md), [`docs/02-design/03-routing-and-proxy.md`](file:///home/fugui/rusts/rs-gate/docs/02-design/03-routing-and-proxy.md)
  - 接口契约：[`docs/03-api/01-gateway-api.md`](file:///home/fugui/rusts/rs-gate/docs/03-api/01-gateway-api.md)
- **状态**：就绪待实施 (Ready for Implementation)

---

## 1. 本里程碑目标与文件变更树 (Target & File Tree)

### 1.1 核心目标与范围边界
本阶段聚焦于构建高性能、低时延的大模型反向代理核心管道与智能选路调度器：
1. **6 大原生直通端点**：实现全面兼容 OpenAI/原生规范的 6 大代理入口（`chat/completions`、`responses`、`completions`、`embeddings`、`rerank`、`models`）；
2. **WLAC 加权最小活跃长连接调度**：实现基于整数无除法交叉乘法（$C_A \times W_B \text{ vs } C_B \times W_A$）的最优选路算法与平滑轮询平局打破；
3. **RAII 级联长连接计数守卫**：实现 `ActiveConnectionGuard`，利用 Rust 所有权与 `Drop` 机制保障在正常完成、上游异常或客户端提前断开时原子回收连接计数，彻底杜绝计数泄漏；
4. **Reqwest 长连接池与凭证隔离**：统一管理对物理后端集群的长连接复用，自动剥离 Hop-by-hop 头并安全注入后端真实 API Key；
5. **逐 Chunk 零缓冲流式管道**：基于字节流透传机制，实现首字附加延迟 $\le 1\text{ms}$，轻量增量解析 `stream_options.include_usage` 尾部 Usage 帧；
6. **客户端 Abort 级联中断**：下游客户端主动关闭连接时，立即 Drop 上游异步 Request/Stream，向上游物理实例触发 TCP RST 中断推理，即时释放 GPU 显存。

### 1.2 涉及文件变更清单

```text
rs-gate/
├── src/
│   ├── routing/
│   │   ├── mod.rs                  # [新增] 路由表管理与读写并发锁
│   │   ├── backend.rs              # [新增] 物理后端节点实例定义与原子计数
│   │   └── scheduler.rs            # [新增] WLAC 无除法交叉乘法调度器实现
│   └── proxy/
│       ├── mod.rs                  # [新增] 代理路由挂载与分派
│       ├── client.rs               # [新增] Reqwest 上游长连接池与凭据脱敏注入
│       ├── guard.rs                # [新增] ActiveConnectionGuard (RAII Drop 计数器)
│       ├── stream.rs               # [新增] 零缓冲 SSE 管道与增量 Usage 解析器
│       └── handler.rs              # [新增] 6 大多协议直通端点处理函数
└── tests/
    ├── common/
    │   └── mock_llm.rs             # [新增] Mock LLM 后端服务器 (wiremock 模拟器)
    ├── wlac_scheduler_test.rs      # [新增] WLAC 交叉乘法调度与平局测试
    ├── proxy_passthrough_test.rs   # [新增] 6 大端点非流式原生透传契约测试
    └── sse_stream_test.rs          # [新增] 逐 Chunk 零缓冲、Usage 解析与客户端中断测试
```

---

## 2. 测试夹具与 TDD 先行用例 (TDD Test Harness & Failing Tests - Red Phase)

### 2.1 Mock LLM 后端服务夹具 (`tests/common/mock_llm.rs`)

```rust
use wiremock::{MockServer, ResponseTemplate, matchers::{method, path}};
use wiremock::ResponseTemplate as Response;
use std::time::Duration;

pub struct MockLlmCluster {
    pub server_a: MockServer,
    pub server_b: MockServer,
}

impl MockLlmCluster {
    pub async fn start() -> Self {
        let server_a = MockServer::start().await;
        let server_b = MockServer::start().await;
        Self { server_a, server_b }
    }
}
```

### 2.2 WLAC 调度算法交叉乘法单元测试 (`tests/wlac_scheduler_test.rs`)

```rust
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
    
    assert_eq!(selected.id, "be-b", "Should pick backend B due to 3*10 < 2*20");
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

    assert_ne!(first.id, second.id, "Tie must be broken smoothly by round robin");
}
```

### 2.3 代理透传与响应契约测试 (`tests/proxy_passthrough_test.rs`)

```rust
use wiremock::{Mock, MockServer, ResponseTemplate, matchers::{method, path}};
use axum::{body::Body, http::{Request, StatusCode}};
use tower::ServiceExt;

#[tokio::test]
async fn test_chat_completions_passthrough() {
    let mock_server = MockServer::start().await;
    let expected_body = r#"{"id":"chatcmpl-test","choices":[{"message":{"content":"Hello"来自上游}}]}"#;

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(expected_body, "application/json"))
        .mount(&mock_server)
        .await;

    let app = rs_gate::proxy::build_test_proxy_app(&mock_server.uri()).await;

    let req = Request::builder()
        .uri("/v1/chat/completions")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"model":"mock-model","messages":[{"role":"user","content":"Hi"}]}"#))
        .unwrap();

    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(String::from_utf8_lossy(&bytes), expected_body);
}
```

### 2.4 SSE 零缓冲流式与连接回收测试 (`tests/sse_stream_test.rs`)

```rust
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
    assert_eq!(counter.load(Ordering::SeqCst), 0, "Counter must auto-decrement on Drop");
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
    let usage = parser.extract_usage().expect("Usage must be parsed from chunk2");
    assert_eq!(usage.prompt_tokens, 10);
    assert_eq!(usage.completion_tokens, 5);
    assert_eq!(usage.total_tokens, 15);

    parser.feed_chunk(chunk3.as_bytes());
}
```

---

## 3. 分步工程实施任务 (Step-by-Step Implementation - Green Phase)

### 3.1 Task 2.1：物理后端节点模型与路由表并发管理
- **目标文件**：`src/routing/backend.rs`, `src/routing/mod.rs`
- **核心结构定义**：
```rust
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct BackendInstance {
    pub id: String,
    pub base_url: String,
    pub weight: u32,
    pub upstream_api_key: Option<String>,
    pub active_connections: Arc<AtomicUsize>,
    pub is_healthy: Arc<std::sync::atomic::AtomicBool>,
}

pub struct RouterTable {
    // model_id -> List of BackendInstance
    routes: arc_swap::ArcSwap<std::collections::HashMap<String, Vec<BackendInstance>>>,
}
```
- **关键要点**：
  - 使用 `arc_swap::ArcSwap` 存储全局路由表，保证网关代理热路径为无锁只读（Zero-Lock Read），更新由后台配置/探针统一写入并原子替换指针。

### 3.2 Task 2.2：WLAC 无除法交叉乘法调度器
- **目标文件**：`src/routing/scheduler.rs`
- **算法核心逻辑**：
```rust
pub fn select_wlac_backend<'a>(
    backends: &'a [BackendInstance],
) -> Option<&'a BackendInstance> {
    if backends.is_empty() {
        return None;
    }

    let mut best: Option<&'a BackendInstance> = None;

    for candidate in backends {
        if !candidate.is_healthy.load(std::sync::atomic::Ordering::Relaxed) {
            continue;
        }

        match best {
            None => best = Some(candidate),
            Some(current) => {
                let conn_c = candidate.active_connections.load(std::sync::atomic::Ordering::Relaxed) as u64;
                let conn_curr = current.active_connections.load(std::sync::atomic::Ordering::Relaxed) as u64;
                let w_c = candidate.weight as u64;
                let w_curr = current.weight as u64;

                // 交叉乘法比较: (conn_c / w_c) < (conn_curr / w_curr) <=> conn_c * w_curr < conn_curr * w_c
                let lhs = conn_c * w_curr;
                let rhs = conn_curr * w_c;

                if lhs < rhs {
                    best = Some(candidate);
                }
            }
        }
    }

    best
}
```

### 3.3 Task 2.3：`ActiveConnectionGuard` 自动析构守卫
- **目标文件**：`src/proxy/guard.rs`
- **结构与 Drop 实现**：
```rust
pub struct ActiveConnectionGuard {
    counter: Arc<AtomicUsize>,
}

impl ActiveConnectionGuard {
    pub fn new(counter: Arc<AtomicUsize>) -> Self {
        counter.fetch_add(1, Ordering::SeqCst);
        Self { counter }
    }
}

impl Drop for ActiveConnectionGuard {
    fn drop(&mut self) {
        self.counter.fetch_sub(1, Ordering::SeqCst);
    }
}
```
- **关键保障**：在处理 Future 或 Stream 意外中断、客户端断开、panic 或正常返回时，编译器无条件调用 `drop`，永不泄漏。

### 3.4 Task 2.4：Reqwest 长连接池与凭证剥离注入
- **目标文件**：`src/proxy/client.rs`
- **实现要点**：
  - 初始化全局单一 `reqwest::Client` 实例，配置 `pool_idle_timeout(Duration::from_secs(90))` 与 `tcp_keepalive(Duration::from_secs(30))`；
  - 自动剥离 Hop-by-hop 头（`connection`, `keep-alive`, `proxy-authenticate`, `proxy-authorization`, `te`, `trailers`, `transfer-encoding`, `upgrade`）；
  - 将入站 `Authorization: Bearer sk-gate-...` 替换为目标后端配置的真实 `upstream_api_key`；若后端未设凭据则移除该头，避免私钥外泄。

### 3.5 Task 2.5：逐 Chunk 零缓冲流式管道与 Usage 增量解析
- **目标文件**：`src/proxy/stream.rs`
- **实现要点**：
  - 采用 `Body::from_stream(upstream_byte_stream)`，将上游 SSE 字节块直接作为下游 Axum 响应体输出，附加延迟 $\le 1\text{ms}$；
  - 轻量增量解析器 `SseUsageParser`：在逐 chunk 投递下游的同时，流式检测 `\"usage\":` 关键字并提取 Prompt/Completion/Total Tokens，零完整 JSON 反序列化开销。

### 3.6 Task 2.6：6 大直通端点处理器与路由挂载
- **目标文件**：`src/proxy/handler.rs`, `src/proxy/mod.rs`
- **实现端点**：
  - `POST /v1/chat/completions`
  - `POST /v1/responses`
  - `POST /v1/completions`
  - `POST /v1/embeddings`
  - `POST /v1/rerank`
  - `GET /v1/models`
- 挂载至主应用 Router，并配置客户端 Abort 级联：当下游 TCP 断开时，Tokio 自动中止驱动当前代理管道的 Future，触发 `ActiveConnectionGuard` 的 Drop 与上游 HTTP 请求的即时释放。

---

## 4. 自动化验收标准与命令行验证手册 (Automated AC & CLI Playbook)

### 4.1 单元与集成测试自动化验证

```bash
# 1. 运行 WLAC 调度算法交叉乘法测试
cargo test --test wlac_scheduler_test -- --nocapture

# 2. 运行 6 大原生端点代理透传测试
cargo test --test proxy_passthrough_test -- --nocapture

# 3. 运行零缓冲 SSE 流式管道与 ActiveConnectionGuard 回收测试
cargo test --test sse_stream_test -- --nocapture
```

**验收门禁通过准则**：
- [ ] WLAC 交叉乘法在整数范围内严格选路正确，平局时轮询打破；
- [ ] 代理透明转发测试中，请求体与响应体 100% 字节对齐，无非预期修改；
- [ ] 客户端中断测试中，`active_connections` 在断开后立即归零；
- [ ] SSE 增量 Usage 解析器成功在不破坏流式输出的前提下提取准确 Token 数。

### 4.2 命令行黑盒验证手册 (CLI Blackbox Playbook)

```bash
# 启动本地 rs-gate 服务
cargo run

# 发起聊天补全直通请求测试
curl -i -X POST http://localhost:8080/v1/chat/completions \
  -H "Authorization: Bearer sk-gate-testkey00000000000000000000" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "deepseek-r1",
    "messages": [{"role": "user", "content": "Ping"}]
  }'

# 预期返回结果
# HTTP/1.1 200 OK
# Content-Type: application/json
# x-request-id: <uuid>
# {"id":"chatcmpl-...","choices":[{"message":{"content":"Pong"}}]}
```
