# rs-gate M5 作战卡：生产就绪与综合发布加固 (Production Hardening & Release)

- **文档编号**：`06-m5-production-hardening.md`
- **文档版本**：`v1.0.0`
- **编制日期**：2026-10-19
- **当前阶段**：阶段三：接口契约与任务分解 (第 4 步：M5 独立作战卡篇)
- **基准依赖**：
  - 实施总纲：[`docs/04-tasks/01-roadmap.md`](file:///home/fugui/rusts/rs-gate/docs/04-tasks/01-roadmap.md)
  - 架构设计：[`docs/02-design/01-system-architecture.md`](file:///home/fugui/rusts/rs-gate/docs/02-design/01-system-architecture.md), [`docs/02-design/04-limiting-and-audit.md`](file:///home/fugui/rusts/rs-gate/docs/02-design/04-limiting-and-audit.md)
  - 接口契约：[`docs/03-api/01-gateway-api.md`](file:///home/fugui/rusts/rs-gate/docs/03-api/01-gateway-api.md), [`docs/03-api/02-admin-api.md`](file:///home/fugui/rusts/rs-gate/docs/03-api/02-admin-api.md)
- **状态**：定稿待审 (Ready for Review)

---

## 1. 本里程碑目标与文件变更树 (Target & File Tree)

### 1.1 核心目标与范围边界
本阶段聚焦于网关系统的生产可用性保障、可观测性监控、历史数据生命周期治理与全链路闭环加固发布：
1. **Prometheus 可观测性度量导出**：在 `GET /metrics` 暴露标准指标（QPS 计数器、长连接实时 Gauge、重试次数 Counter、请求耗时直方图），并提供 `GET /health/ready` 就绪探针；
2. **审计日志 TTL 滚动清理与增量收缩**：实现后台轻量定时任务分批清除超过保留期（如 30 天）的历史审计流水，并触发 SQLite `incremental_vacuum` 回收物理磁盘空间；
3. **全链路端到端回归集成测试套件**：编写端到端闭环用例，覆盖“用户注册/登录 -> 颁发 Key -> 代理多模型调用 -> SSE 流式透传 -> 静默重试与故障转移 -> 批量审计落盘 -> 控制台查询大盘与导出 CSV”全生命周期；
4. **编译质量与静态加固**：执行代码格式化检查与 `cargo clippy --all-targets -- -D warnings` 零警告红线治理；
5. **跨平台单二进制静态编译打包**：支持 `x86_64-unknown-linux-musl` 静态编译链接，通过 LTO 优化与二进制剥离（`strip`），将包含前端资产的单二进制体积稳定压制在 $\le 25\text{MB}$。

### 1.2 涉及文件变更清单

```text
rs-gate/
├── Makefile                                # [新增] 一键测试、静态分析与 musl 静态编译流水线
├── scripts/
│   └── build.sh                            # [新增] 自动化构建与单二进制体积校验脚本
├── src/
│   ├── metrics.rs                          # [新增] Prometheus 指标注册中心与 /metrics 导出
│   └── audit/
│       └── retention.rs                    # [新增] 历史审计 TTL 分批清理与增量收缩任务
└── tests/
    ├── metrics_scrape_test.rs              # [新增] Prometheus 监控指标抓取与计数准确性测试
    ├── retention_cleanup_test.rs           # [新增] 过期审计数据滚动清理与磁盘收缩集成测试
    └── e2e_full_flow_test.rs               # [新增] 全链路全生命周期闭环端到端集成测试
```

---

## 2. 测试夹具与 TDD 先行用例 (TDD Test Harness & Failing Tests - Red Phase)

### 2.1 Prometheus 指标采集测试 (`tests/metrics_scrape_test.rs`)

```rust
use axum::{body::Body, http::{Request, StatusCode}};
use tower::ServiceExt;

#[tokio::test]
async fn test_metrics_scrape_and_counter_increment() {
    let app = rs_gate::server::build_test_app().await;

    // 1. 模拟触发一次代理请求
    let proxy_req = Request::builder()
        .uri("/v1/chat/completions")
        .method("POST")
        .header("authorization", "Bearer sk-gate-validkey00000000000000000000")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"model":"deepseek-r1"}"#))
        .unwrap();
    let _ = app.clone().oneshot(proxy_req).await.unwrap();

    // 2. 抓取 /metrics 端点
    let metrics_req = Request::builder()
        .uri("/metrics")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(metrics_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let metrics_text = String::from_utf8_lossy(&bytes);

    // 断言指标存在且计数递增
    assert!(metrics_text.contains("rsgate_requests_total"));
    assert!(metrics_text.contains("rsgate_active_connections"));
    assert!(metrics_text.contains("rsgate_retry_attempts_total"));
}
```

### 2.2 审计日志 TTL 滚动清理测试 (`tests/retention_cleanup_test.rs`)

```rust
use crate::common::TestDbHarness;
use rs_gate::audit::retention::RetentionCleaner;

#[tokio::test]
async fn test_retention_cleanup_removes_expired_logs() {
    let harness = TestDbHarness::new().await.unwrap();
    rs_gate::storage::migration::run_migrations(&harness.pool).await.unwrap();

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
```

### 2.3 全链路端到端闭环回归测试 (`tests/e2e_full_flow_test.rs`)

```rust
#[tokio::test]
async fn test_e2e_full_system_lifecycle() {
    // 步骤 1: 系统冷启动并自建超管
    // 步骤 2: 登录超管并创建开发者用户与模型集群
    // 步骤 3: 开发者登录自服务门户生成虚拟 API Key
    // 步骤 4: 开发者使用生成的 Key 发起流式 SSE 聊天请求
    // 步骤 5: 验证上游模拟抖动时触发网关静默重试与故障转移
    // 步骤 6: 验证批量审计 Worker 异步刷盘并同步累加配额
    // 步骤 7: 超管调用 API 导出审计 CSV 并核对内容
}
```

---

## 3. 分步工程实施任务 (Step-by-Step Implementation - Green Phase)

### 3.1 Task 5.1：Prometheus 指标集成与就绪探针
- **目标文件**：`src/metrics.rs`, `src/server.rs`
- **核心指标清单**：
  - `rsgate_requests_total{model, status, is_stream}`：请求总量计数器 (Counter)；
  - `rsgate_active_connections`：当前网关活跃在途长连接数 (Gauge)；
  - `rsgate_retry_attempts_total{reason}`：上游静默重试总次数 (Counter)；
  - `rsgate_request_duration_seconds{model}`：网关处理耗时直方图 (Histogram)；
  - `rsgate_ttft_seconds{model}`：首字附加透传延迟直方图 (Histogram)。
- **探针端点**：
  - `GET /metrics`：导出 Prometheus 文本格式数据；
  - `GET /health/ready`：就绪探针，校验数据库连接池与关键组件均就绪后返回 200 OK。

### 3.2 Task 5.2：历史审计数据分批 TTL 清理与增量收缩
- **目标文件**：`src/audit/retention.rs`
- **核心逻辑**：
  - 读取配置项 `security.audit_retention_days`（默认 30 天）；
  - 启动后台低优先级定时器（每 24 小时或凌晨 03:00 触发）；
  - 分批软删除：`DELETE FROM audit_logs WHERE created_at < ? LIMIT 1000;`，循环执行直至清空，避免单一大事务锁定 SQLite；
  - 清理完毕后触发 `PRAGMA incremental_vacuum(100);` 释放页碎片，维持数据库轻量体积。

### 3.3 Task 5.3：全链路回归集成测试套件落地
- **目标文件**：`tests/e2e_full_flow_test.rs`
- **实现要点**：
  - 采用全内存 SQLite 与 `wiremock` 模拟上游物理服务；
  - 严格编排前述 7 大生命周期步骤，确保各 Milestone 代码无任何协同断层与隐式 bug。

### 3.4 Task 5.4：代码静态分析、格式化与 Clippy 零警告加固
- **执行命令与规范**：
  - `cargo fmt --check`：确保项目代码排版一致；
  - `cargo clippy --all-targets -- -D warnings`：开启全部编译目标（含测试目标与二进制目标）的零警告红线，消除所有未处理的潜在死锁、悬垂引用与冗余克隆。

### 3.5 Task 5.5：跨平台单二进制静态编译打包优化
- **目标文件**：[`Makefile`](file:///home/fugui/rusts/rs-gate/Makefile), `Cargo.toml`, `scripts/build.sh`
- **构建配置调优 (`Cargo.toml`)**：
```toml
[profile.release]
opt-level = 3
lto = true
codegen-units = 1
panic = "abort"
strip = true
```
- **静态构建脚本**：
  - 构建前端产物：`cd web && npm run build`；
  - 静态编译二进制：`cargo build --release --target x86_64-unknown-linux-musl`；
  - 校验最终产物体积 $\le 25\text{MB}$。

---

## 4. 自动化验收标准与命令行验证手册 (Automated AC & CLI Playbook)

### 4.1 全量自动化流水线验证

```bash
# 1. 运行 Prometheus 监控抓取测试
cargo test --test metrics_scrape_test -- --nocapture

# 2. 运行历史审计 TTL 滚动清理测试
cargo test --test retention_cleanup_test -- --nocapture

# 3. 运行全链路端到端闭环回归测试
cargo test --test e2e_full_flow_test -- --nocapture

# 4. 全量运行所有集成与单元测试
cargo test --all

# 5. 严格静态代码分析与格式检查
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

**验收门禁通过准则**：
- [ ] 全部测试用例 100% 绿灯通过；
- [ ] Clippy 零警告，格式化零差异；
- [ ] 审计数据按 TTL 批次清理无阻断，磁盘增量收缩执行成功；
- [ ] musl 静态链接产物体积 $\le 25\text{MB}$，单文件无运行时动态依赖。

### 4.2 生产环境一键构建发布手册 (Production Build Playbook)

```bash
# 1. 执行全量构建流水线
make release

# 2. 验证产物文件信息
file target/x86_64-unknown-linux-musl/release/rs-gate
ls -lh target/x86_64-unknown-linux-musl/release/rs-gate

# 预期结果
# ELF 64-bit LSB pie executable, x86-64, statically linked, stripped
# 体积: <= 25MB
```
