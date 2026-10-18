# rs-gate: 阶段三接口契约与任务分解 (SDD) 提示词指令

## 1. 角色定位与工程原则
你是一名资深分布式系统架构师与 Rust 全栈技术专家。我们正在采用 **规范驱动开发 (Spec-Driven Development, SDD)** 模式开发 **rs-gate（轻量级、单二进制交付的企业级内部大模型网关）**。

在阶段一特性清单（[`docs/01-features/01-feature-matrix.md`](file:///home/fugui/rusts/rs-gate/docs/01-features/01-feature-matrix.md)）与阶段二系统架构设计全量规范（[`docs/02-design/01-system-architecture.md`](file:///home/fugui/rusts/rs-gate/docs/02-design/01-system-architecture.md) ~ `04-limiting-and-audit.md`）已审计通过并定稿入库的基础上，现在正式进入：
**「阶段三：接口契约与任务分解编制」**。

---

## 2. 任务目标与阶段三文档矩阵 (Deliverables Matrix)

阶段三的目标是把阶段二的架构设计方案转化为**无歧义的前后端/网关客户端接口契约**以及**可执行、可检验的工程实施任务分解清单**。

| 文档路径 | 规范类型 | 核心职责与设计边界 |
| :--- | :--- | :--- |
| [`docs/03-api/01-gateway-api.md`](file:///home/fugui/rusts/rs-gate/docs/03-api/01-gateway-api.md) | **【代理协议契约】网关代理 API 契约** | 定义 OpenAI 兼容与原生直通端点契约（`/v1/chat/completions`、`/v1/responses`、`/v1/completions`、`/v1/embeddings`、`/v1/rerank`、`/v1/models`）；请求头/响应头规范；统一标准错误体；SSE 流式帧与 Usage chunk 规范。 |
| [`docs/03-api/02-admin-api.md`](file:///home/fugui/rusts/rs-gate/docs/03-api/02-admin-api.md) | **【控制台契约】管理后台 REST API 契约** | 控制台前后端交互 REST API 契约：管理员身份认证、虚拟 API Key 全生命周期与日周配额重置、模型后端集群拓扑热更新、即时能力探测触发、10万级审计日志下钻筛选、监控统计大盘与健康探针。 |
| [`docs/04-tasks/01-tasks.md`](file:///home/fugui/rusts/rs-gate/docs/04-tasks/01-tasks.md) | **【任务与验收】工程实施任务分解与 AC** | 将后续代码实现严格分解为 Milestone 1 ~ Milestone 5；定义每个里程碑的文件变更树、开发前置条件、详细开发任务步骤与自动化验收标准 (AC) 及命令行验证手册。 |

> **红线提示**：本阶段**严禁直接编写业务实现源码**，聚焦编制上述接口契约与任务验收规范。所有接口定义必须具备完整请求/响应 JSON 示例与字段约束；任务拆解必须精准到具体代码文件路径与方法签名。

---

## 3. 核心编制规范与技术维度 (Core Requirements)

### 3.1 网关代理 API 契约规范 (`docs/03-api/01-gateway-api.md`)
1. **多协议原生直通端点规范**：
   - `POST /v1/chat/completions`：非流式 JSON 与流式 SSE；
   - `POST /v1/responses`：OpenAI 新一代 Agentic Responses API（输入 items、输出 response 结构）；
   - `POST /v1/completions`：传统文本补全；
   - `POST /v1/embeddings`：文本向量嵌入（批量 inputs、返回 embedding 向量列表）；
   - `POST /v1/rerank`：文档重排端点（query, documents -> ranked results）；
   - `GET /v1/models`：网关逻辑模型聚合列表。
2. **协议头契约**：
   - 入站头：`Authorization: Bearer <sk-gate-xxx>`、`x-request-id`、`Content-Type`；
   - 出站响应头：`x-request-id`、`X-Quota-Daily-Limit`、`X-Quota-Daily-Remaining`、`X-Quota-Daily-Reset`、`X-Quota-Weekly-Limit`、`X-Quota-Weekly-Remaining`、`X-Quota-Weekly-Reset`、`X-Concurrency-Current`；
3. **错误协议规范**：
   - 统一 OpenAI 错误格式 `{"error": {"message": "...", "type": "...", "code": 429}}`；
   - 状态码精准映射：401（Key 无效/过期）、403（CIDR 拒绝）、429（RPM/并发/日周配额耗尽）、503（无可用后端）、504（上游超时）。
4. **SSE 流式帧与 Usage 规范**：
   - 逐 chunk 帧结构：`data: {"choices":[{"delta":{"content":"..."}}]}\n\n`；
   - 尾部 Usage 帧：`data: {"usage":{"prompt_tokens":12,"completion_tokens":45,"total_tokens":57}}\n\n`；
   - 结束帧：`data: [DONE]\n\n`。

### 3.2 管理控制台 REST API 契约规范 (`docs/03-api/02-admin-api.md`)
1. **统一返回包装格式**：`{"code": 0, "message": "success", "data": { ... }}`；
2. **鉴权端点**：
   - `POST /api/admin/auth/login`（用户名/密码登录，返回 JWT/Session Cookie）；
   - `POST /api/admin/auth/logout`（登出注销）；
   - `GET /api/admin/auth/me`（获取当前管理员信息与角色）。
3. **虚拟 API Key 管理端点**：
   - `GET /api/admin/keys`（Key 列表，包含日周配额已用进度）；
   - `POST /api/admin/keys`（新建 Key，仅本次返回明文 `sk-gate-...`）；
   - `PUT /api/admin/keys/:id`（修改限额、RPM、日周配额上限与模型白名单）；
   - `DELETE /api/admin/keys/:id`（软删除/禁用 Key）；
   - `POST /api/admin/keys/:id/reset-quota`（手动清零今日或本周已用 Token）。
4. **物理后端与能力探测端点**：
   - `GET /api/admin/backends`（后端服务器列表，含权重、协议能力徽章、健康状态机与实时活跃连接数）；
   - `POST /api/admin/backends`（添加后端服务器，配置地址、权重、托管凭据，并立即触发初次探活）；
   - `PUT /api/admin/backends/:id`（修改权重或托管凭据）；
   - `DELETE /api/admin/backends/:id`（下线并移除后端）；
   - `POST /api/admin/backends/:id/probe`（手动触发即时协议能力与健康探测）。
5. **逻辑模型管理端点**：
   - `GET /api/admin/models`（模型列表及挂载后端池）；
   - `POST /api/admin/models`（定义新逻辑模型）；
   - `PUT /api/admin/models/:id`（修改模型参数）；
   - `POST /api/admin/models/:id/backends`（绑定后端并设置权重，触发内存路由表热重载）；
   - `DELETE /api/admin/models/:id/backends/:backend_id`（解绑后端）。
6. **审计日志与统计大盘端点**：
   - `GET /api/admin/audits`（多维筛选：时间范围、Key、模型、状态码、耗时，支持分页与游标）；
   - `GET /api/admin/audits/export`（CSV/JSON 账单导出）；
   - `GET /api/admin/stats/overview`（今日请求总数、总 Token 消耗、活跃长连接、平均 P99 延迟）；
   - `GET /api/admin/stats/trends`（过去 24 小时 / 7 天吞吐与 Token 消耗时序折线数据）。
7. **系统监控与运维端点**：
   - `GET /health/live`（进程存活探针）；
   - `GET /health/ready`（SQLite 与核心组件就绪探针）；
   - `GET /metrics`（Prometheus OpenMetrics 指标文本输出）。

### 3.3 实施任务分解与自动化验收标准 (`docs/04-tasks/01-tasks.md`)
将后续代码实现严格拆解为 5 大里程碑，杜绝一次性盲目堆砌：
1. **Milestone 1：基础骨架与存储引擎 (Foundation & Storage)**
   - 范围：Cargo 依赖组织、全局配置管理（`src/config/`）、`tracing` 结构化日志、SQLite WAL 存储连接池与 `migrations/` 声明式自初始化、存活健康探针。
   - AC 标准：本地 `cargo test` 全过，进程启动自动建表并生成默认管理员，HTTP 访问 `/health/live` 返回 200。
2. **Milestone 2：核心代理与 SSE 调度引擎 (Proxy & WLAC)**
   - 范围：多协议直通端点注册、加权最小活跃长连接调度器（WLAC 无除法交叉乘法）、`ActiveConnectionGuard` (RAII)、Reqwest 长连接池、逐 Chunk 零缓冲流式管道、增量 Usage 提取、客户端 Abort 级联中断。
   - AC 标准：通过 Mock/真实 vLLM 实例验证，支持 Chat/Responses/Embedding 直通与流式 SSE，首字附加延迟 $\le 1\text{ms}$，客户端断开长连接立即释放。
3. **Milestone 3：流控安全与异步审计管道 (Limiting & Audit)**
   - 范围：虚拟 Key 加盐 Hash 校验、无锁原子令牌桶（RPM）、并发连接门限控制、日/周两级 Token 配额拦截与自动重置时间戳计算、Tokio mpsc 有界队列与 500ms 批量事务刷盘 Worker。
   - AC 标准：未授权 Key 快速返回 401，超限请求返回 429 与配额头，高并发下审计数据不丢不阻塞。
4. **Milestone 4：管理控制台与单二进制内嵌 (Admin Web & Single Binary)**
   - 范围：Admin REST API 全量 Handler、前端现代轻量 SPA 界面（协议徽章、日周双进度条、模型热更新、审计下钻）、`rust-embed` 单二进制内嵌打包与 SPA History 路由 Fallback。
   - AC 标准：仅运行单个二进制，浏览器访问直接展示完整控制台界面，实现配置增删改查热生效。
5. **Milestone 5：生产就绪与发布加固 (Hardening & Release)**
   - 范围：Prometheus `/metrics` 监控导出、历史数据 TTL 滚动分批清理与增量收缩、全链路端到端回归集成测试、跨平台静态编译（musl/static）。
   - AC 标准：`cargo clippy` 零警告，所有单元与集成测试全过，输出免外部依赖单二进制。

---

## 4. 执行节奏与分步编制策略 (Execution Rhythm)

为确保每份规范文档达到极高工业级水准，阶段三任务必须**按顺序分步逐篇输出**：

1. **第一步（当前首要任务）**：编制 [`docs/03-api/01-gateway-api.md`](file:///home/fugui/rusts/rs-gate/docs/03-api/01-gateway-api.md)（【代理协议契约】网关代理 API 契约规范）；
2. **第二步**：编制 [`docs/03-api/02-admin-api.md`](file:///home/fugui/rusts/rs-gate/docs/03-api/02-admin-api.md)（【控制台契约】管理后台 REST API 契约规范）；
3. **第三步**：编制 [`docs/04-tasks/01-tasks.md`](file:///home/fugui/rusts/rs-gate/docs/04-tasks/01-tasks.md)（【任务与验收】工程实施任务分解与自动化验收清单）。

---

## 5. 完成后自动化流程
每一步文档编写完成后，必须自检逻辑与格式完整性，确认无误后直接执行 git commit 与 git push 同步至 GitHub 仓库（`origin/main`），无需在汇报后等待口头指令。
