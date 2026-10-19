# rs-gate: 阶段三工程任务分解与 TDD 自动化验收清单编制提示词指令

- **文档编号**：`04-tasks-prompt.md`
- **编制日期**：2026-10-19
- **当前阶段**：阶段三：接口契约与任务分解 (第 4 步：工程实施与 TDD 任务分解提示词篇)
- **基准依赖**：
  - 阶段一需求范围：[`docs/01-features/01-feature-matrix.md`](file:///home/fugui/rusts/rs-gate/docs/01-features/01-feature-matrix.md)
  - 阶段二系统设计：[`docs/02-design/01-system-architecture.md`](file:///home/fugui/rusts/rs-gate/docs/02-design/01-system-architecture.md) ~ `05-web-console.md`
  - 阶段三接口契约：[`docs/03-api/01-gateway-api.md`](file:///home/fugui/rusts/rs-gate/docs/03-api/01-gateway-api.md) ~ `03-database-schema.md`
- **目标产物**：[`docs/04-tasks/01-tasks.md`](file:///home/fugui/rusts/rs-gate/docs/04-tasks/01-tasks.md)

---

## 1. 角色定位与工程原则

你是一名资深分布式系统架构师与 Rust 全栈技术专家。我们正在采用 **规范驱动开发 (Spec-Driven Development, SDD)** 模式与 **测试驱动开发 (Test-Driven Development, TDD)** 理念开发 **rs-gate（轻量级、单二进制交付的企业级内部大模型网关）**。

在前期阶段产物已全部定稿入库：
- 阶段一：特性清单与边界界定（[`docs/01-features/01-feature-matrix.md`](file:///home/fugui/rusts/rs-gate/docs/01-features/01-feature-matrix.md)）
- 阶段二：系统架构设计 5 篇核心子系统规范（[`docs/02-design/01-system-architecture.md`](file:///home/fugui/rusts/rs-gate/docs/02-design/01-system-architecture.md) ~ `05-web-console.md`）
- 阶段三：接口与数据契约 3 篇全量规范（[`docs/03-api/01-gateway-api.md`](file:///home/fugui/rusts/rs-gate/docs/03-api/01-gateway-api.md)、`02-admin-api.md`、`03-database-schema.md`）

现在正式进入阶段三的收官步骤：
**「编制 `docs/04-tasks/01-tasks.md`：工程实施任务分解与自动化验收清单」**。

---

## 2. 核心编制目标与任务组织哲学 (Core Mission & TDD Philosophy)

本任务文档是连接“上层架构与接口设计规范”与“下层物理工程代码实施”的**唯一权威执行作战蓝图**。
必须彻底杜绝“一次性堆砌业务代码”的粗糙开发模式，严格遵循 **「TDD 先行 (Test-First) + 里程碑递进 (Milestone Progression) + 自动化验收 (AC-Driven)」** 的核心工作流：

### 2.1 任务阶段递进原则 (Milestone 1 ~ 5)
后续工程落地划分为 5 大递进里程碑：
- **Milestone 1：基础骨架、配置管理与 SQLite WAL 存储引擎 (Foundation & Storage Engine)**
- **Milestone 2：原生直通代理、WLAC 调度与零缓冲 SSE 管道 (Proxy, WLAC Scheduler & SSE Engine)**
- **Milestone 3：虚拟 Key 鉴权、日周配额流控与静默重试审计管道 (Security, Quotas & Audit Pipeline)**
- **Milestone 4：Web 控制台、自服务门户与单二进制嵌入 (Web Console, Portal & Single Binary)**
- **Milestone 5：生产就绪、运维监控与综合发布加固 (Hardening, Metrics & Release)**

### 2.2 每个里程碑的标准化四段式结构 (Four-Stage Blueprint per Milestone)
在编制 `01-tasks.md` 时，**每一个 Milestone 都必须严格包含以下 4 个标准小节**：
1. **里程碑目标与文件变更树 (Target & File Tree)**：明确本阶段的目标范围、前置依赖，以及涉及的新增/修改文件树清单（精准到文件路径）。
2. **测试夹具与 TDD 先行用例 (TDD Test Harness & Failing Tests - Red Phase)**：
   - 在实现业务代码之前，先定义好测试环境所需的基础设施（如内存 SQLite 夹具、Mock LLM 服务器、Axum 测试客户端）；
   - 明确先编写的单元测试与集成测试文件（`tests/*.rs`），定义测试用例函数名、输入 Payload 与预期断言（Assert）。
3. **分步工程实施任务 (Step-by-Step Implementation - Green Phase)**：
   - 按逻辑依赖顺序列出详细实施步骤（Task 1.1, Task 1.2...）；
   - 每个任务必须精准列出目标文件路径、核心结构体/枚举/方法签名（Rust Signature）及关键实现要点。
4. **自动化验收标准与命令行验证手册 (Automated AC & CLI Playbook)**：
   - 提供可在终端复制执行的一键校验指令（如 `cargo test --test xxx`、`curl -i http://localhost:8080/...`）；
   - 明确可量化、确定性的通过准则（Pass Criteria），杜绝模糊的主观判定。

---

## 3. 各里程碑具体编制规范与技术细则 (Detailed Specifications for M1 ~ M5)

### 3.1 Milestone 1：基础骨架、配置管理与 SQLite WAL 存储引擎 (Foundation & Storage)
- **核心范围**：
  - 初始化 `Cargo.toml` 依赖包清单（`tokio`, `axum`, `sqlx` (sqlite), `serde`, `serde_json`, `tracing`, `tracing-subscriber`, `config`, `argon2`, `rand`, `thiserror`, `anyhow`, `uuid` 等）；
  - 全局多源配置系统（`src/config/mod.rs`）：支持 YAML/TOML 文件读取、`RSGATE_*` 环境变量优先级覆写、安全默认值回退；
  - 结构化日志追踪（`src/telemetry.rs`）：初始化 `tracing-subscriber`，支持 JSON / 紧凑控制台格式，支持 `RUST_LOG` 动态级别；
  - SQLite WAL 存储连接池（`src/storage/db.rs`）：启动时配置 WAL 模式、Busy Timeout 5000ms、Foreign Keys ON、内存页面缓存等关键 Pragmas；
  - 声明式数据库自初始化（`src/storage/migration.rs`）：启动自动执行 `migrations/20261019000000_init.sql`（7 张表建表）与 `20261019000001_indexes.sql`（索引创建）；
  - 默认超级管理员自动生成（`src/storage/bootstrap.rs`）：初次启动若 `users` 无记录，自动初始化默认 `admin` 账号（支持环境变量预设或 16 位强随机熵输出）；
  - HTTP 存活探针（`src/server.rs`）：Axum 服务端监听，注册 `GET /health/live` 端点返回 200 OK。
- **TDD 先行用例设计**：
  - 编写 `tests/common/mod.rs`（建立临时目录 SQLite 数据库测试夹具 `TestDbHarness`，测试结束自动清理）；
  - 编写 `tests/storage_migration_test.rs`：测试声明式迁移幂等性（反复执行不报错）、测试 7 张表结构与主外键完整性、测试索引存在性、测试默认超管是否自动注入且密码哈希可校验；
  - 编写 `tests/health_probe_test.rs`：通过 `tower::ServiceExt` 发起 GET `/health/live`，断言返回 `200 OK {"status":"alive"}`。

### 3.2 Milestone 2：核心代理、WLAC 调度与零缓冲 SSE 管道 (Proxy & WLAC)
- **核心范围**：
  - 注册 6 大原生直通端点（`POST /v1/chat/completions`、`POST /v1/responses`、`POST /v1/completions`、`POST /v1/embeddings`、`POST /v1/rerank`、`GET /v1/models`）；
  - 内存路由表与加权最小活跃长连接算法（`src/routing/`）：无除法交叉乘法比较（$C_A \times W_B \text{ vs } C_B \times W_A$），平滑轮询平局打破；
  - `ActiveConnectionGuard`（`src/proxy/guard.rs`）：利用 RAII `Drop` 机制原子管理 `active_connections.fetch_sub(1)`，防断连泄漏；
  - Reqwest 高性能长连接池（`src/proxy/client.rs`）：连接池复用、TCP KeepAlive、Hop-by-hop 头自动剥离与真实凭证注入；
  - 零缓冲 SSE 流式管道（`src/proxy/stream.rs`）：基于字节块逐 chunk 透传，首字透传附加延迟 $\le 1\text{ms}$；轻量增量解析 `stream_options.include_usage` 尾部 Usage 帧；
  - 客户端 Abort 级联中断时序：下游断开立即 Drop 管道与 Guard，上游触发 TCP RST 回收 GPU 显存。
- **TDD 先行用例设计**：
  - 编写 `tests/common/mock_llm.rs`：基于 `wiremock` 或轻量 Axum 搭建 Mock LLM 后端服务器（可模拟快速 JSON、可分块持续输出 SSE 帧、可模拟延迟）；
  - 编写 `tests/wlac_scheduler_test.rs`：并发构造多实例不同权重与不同活跃连接，断言选路结果是否严格符合无除法公式；
  - 编写 `tests/proxy_passthrough_test.rs`：验证 Chat/Responses/Completions/Embeddings/Rerank 请求体与响应体 100% 原生透传；
  - 编写 `tests/sse_stream_test.rs`：验证流式输出逐 chunk 透传时延、Usage 解析准确性、客户端中断时长连接计数立即自减。

### 3.3 Milestone 3：安全鉴权、日周配额与静默重试审计管道 (Security, Quotas & Audit)
- **核心范围**：
  - 虚拟 Key 鉴权中间件（`src/auth/`）：SHA-256 加盐 Hash 内存匹配与脱敏前缀快速点查、CIDR IP 白名单匹配、模型白名单匹配；
  - 多级流控（`src/limiting/`）：基于 `AtomicU64` 的 CAS 无锁令牌桶 RPM 限流、基于 `AtomicUsize` 的在途并发门限控制；
  - 日/周双周期 Token 配额引擎（`src/limiting/quota.rs`）：对齐自然日午夜与周日午夜自动轮转重置、流式准入门槛（最低预留门限 500 Token）；
  - 上游静默重试与故障转移引擎（`src/proxy/retry.rs`）：针对上游 5xx / 429 / 连接超时 / 连接重置，支持最大 2 次自适应退避与 Full Jitter 重试；优先 Failover 切换下一个健康节点；
  - 流式重试安全红线：严格执行首字前（Pre-first-chunk）静默重试与首字后（Mid-stream）安全优雅中止（推送 SSE 错误帧与 `[DONE]` 帧并断开，坚决禁止重发已输出 Token）；
  - 异步审计流水管道（`src/audit/`）：Tokio mpsc 有界通道（容量 10,000）投递 `AuditEvent`，独立后台 Worker 触发 500ms / 100 条批量事务落盘 `audit_logs` 表，同步原子更新 `quota_usages`，完整记录 `retry_count` 与 `failover_path`。
- **TDD 先行用例设计**：
  - 编写 `tests/auth_middleware_test.rs`：测试非法 Key 返回 401、过期 Key 返回 401、非白名单 IP 返回 403、非授权模型返回 403；
  - 编写 `tests/rate_limit_test.rs`：高频击发测试 RPM 触发 429 与 `Retry-After` 头、并发满载触发 429；
  - 编写 `tests/quota_limit_test.rs`：日配额耗尽返回 429、周配额耗尽返回 429，测试跨天与跨周自动重置逻辑；
  - 编写 `tests/retry_failover_test.rs`：Mock 节点 A 返回 503，断言网关自动切换至节点 B 并最终成功返回 200，出站头回显 `X-Gateway-Retry-Attempts: 1`，审计记录 `retry_count = 1`；
  - 编写 `tests/stream_retry_boundary_test.rs`：测试首字前节点挂掉成功静默转移；测试首字已吐出后断开，断言输出 SSE 错误帧而不重复生成；
  - 编写 `tests/audit_batch_worker_test.rs`：并发写入 500 条日志，验证批量事务刷盘完整性与配额累加一致性。

### 3.4 Milestone 4：Web 控制台、自服务门户与单二进制嵌入 (Web Console & Portal)
- **核心范围**：
  - 控制台统一 Auth 中间件（`src/admin/auth.rs`）：JWT 签发、校验与 Cookie 拦截，RBAC 角色拦截（`superadmin` / `admin` / `user`）；
  - 普通开发者自服务 REST API（`src/admin/user_api.rs`）：`/api/user/models`、`/api/user/keys`（增删改查、创建仅一次返回明文）、`/api/user/audits`；
  - 系统管理员运维管控 REST API（`src/admin/admin_api.rs`）：`/api/admin/users`、`/api/admin/keys`（全局监控、代建与一键重置配额 `/reset-quota`）、`/api/admin/backends`（列表、探活 `/probe`、徽章刷新）、`/api/admin/models`（模型管理与后端绑定热重载）、`/api/admin/audits`（多维下钻、流式 CSV 导出 `/export`）、`/api/admin/stats`（大盘总览与趋势折线）；
  - 前端工程与静态资产内嵌（`web/` 与 `src/admin/embedded.rs`）：基于 Vanilla CSS 设计系统与原生 JS 实现控制台双视角 SPA，使用 `rust-embed` 编译期内嵌 `web/dist/`，支持 MIME 嗅探、ETag 304 缓存与 SPA History 路由回退。
- **TDD 先行用例设计**：
  - 编写 `tests/admin_auth_rbac_test.rs`：测试登录返回 JWT、测试普通 user 调用 `/api/admin/*` 返回 403、测试数据强隔离（user A 查不到 user B 的 Key 与审计）；
  - 编写 `tests/user_portal_api_test.rs`：测试自建 Key 成功且仅一次返回明文、修改白名单、停用 Key；
  - 编写 `tests/admin_operations_test.rs`：测试一键重置配额立即清零已用 Token、测试即时探活更新后端状态与能力徽章、测试绑定后端触发路由热重载、测试 CSV 导出数据格式；
  - 编写 `tests/embedded_assets_test.rs`：测试根路径 `/` 返回 `index.html`、测试静态资源返回正确 MIME 与 ETag、测试任意未知路径回退至 `index.html`。

### 3.5 Milestone 5：生产就绪、运维监控与综合发布加固 (Hardening & Release)
- **核心范围**：
  - 监控端点集成（`src/metrics.rs`）：注册 Prometheus 指标，提供 `GET /metrics` 与 `GET /health/ready`；
  - 历史审计数据 TTL 滚动清理（`src/audit/retention.rs`）：后台轻量定时任务分批清理过期日志，触发增量收缩（Incremental Vacuum）；
  - 全链路回归集成测试套件（`tests/e2e_gateway_test.rs`）：贯穿鉴权、并发限流、WLAC 调度、SSE 透传、上游重试、异步审计、控制台查询与导出全流程；
  - 静态编译与发布构建脚本（`Makefile` / `scripts/build.sh`）：支持 `x86_64-unknown-linux-musl` 静态链接单二进制产物打包，产物大小控制在 $\le 25\text{MB}$。
- **TDD 先行用例设计**：
  - 编写 `tests/metrics_scrape_test.rs`：模拟请求后抓取 `/metrics`，断言 QPS 计数器、长连接 Gauge 与重试 Counter 计数正确；
  - 编写 `tests/retention_cleanup_test.rs`：向审计表插入超期数据，触发清理任务后断言旧数据被彻底清理；
  - 编写 `tests/e2e_full_flow_test.rs`：真实运行单一二进制，从登录超管、建 Key、发起 OpenAI 格式对话、触发流式 SSE、查看控制台审计记录到一键导出 CSV，全链路通过。

---

## 4. 输出格式与编制红线要求 (Format & Redlines)

1. **结构清晰严密**：输出的 `docs/04-tasks/01-tasks.md` 必须具备详尽的文件变更树、每个任务的精准方法签名、完整的测试夹具定义与命令行验证清单。
2. **红线原则**：本提示词用于指挥生成实施任务分解方案，**在任务文档被检视并定稿前，严禁直接在 `src/` 或 `tests/` 中编写实际业务实现代码**。
3. **交付闭环**：任务文档编制完成后，执行格式与自检，并直接自动提交推送至 GitHub 仓库（`origin/main`）。
