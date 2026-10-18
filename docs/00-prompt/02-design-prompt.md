# rs-gate: 阶段二系统架构设计 (SDD) 提示词指令

## 角色定位与工程原则
你是一名资深分布式系统架构师与 Rust 全栈技术专家。我们正在采用 **规范驱动开发 (Spec-Driven Development, SDD)** 模式开发 **rs-gate（轻量级、单二进制交付的企业级内部大模型网关）**。

在阶段一特性清单与边界界定规范（[`docs/01-features/01-feature-matrix.md`](file:///home/fugui/rusts/rs-gate/docs/01-features/01-feature-matrix.md)）已定稿并检视通过的基础上，现在正式进入：
**「阶段二：系统架构设计文档编制」**。

---

## 任务目标
请在 [`docs/02-design/01-design.md`](file:///home/fugui/rusts/rs-gate/docs/02-design/01-design.md) 中输出完整、详尽、具备高落地性的**系统技术架构设计规范 (System Architecture Design)**。

本阶段**严禁编写任何业务源码与测试代码**，只产出架构设计文档 [`docs/02-design/01-design.md`](file:///home/fugui/rusts/rs-gate/docs/02-design/01-design.md)。

---

## 核心设计要求与技术约束 (Technical Guardrails)

文档必须全面覆盖以下 8 大核心架构维度，并深入解决阶段一检视发现的关键工程细节：

### 1. 端到端系统架构与数据流流水线 (End-to-End Pipeline)
- 绘制整体系统分层架构图（Mermaid 格式），涵盖：网络接入层、鉴权与流控层、路由调度层、SSE 透传中继层、异步审计落盘管道与嵌入式 Web 管理控制台。
- 详细绘制客户端 HTTP/SSE 请求的端到端生命周期序列图 (Sequence Diagram)：
  `Client -> 鉴权校验 -> 令牌桶与日/周配额预检 -> 路由集群选路 -> 活跃连接计数绑定 -> Upstream 中继转发 -> 流式 Chunk 实时透传与 Usage 提取 -> 释放连接计数 -> 异步审计事件投递 -> 批量落盘`。

### 2. 调度算法与并发控制设计 (Weighted Least-Active-Connections)
- **调度选路算法落地**：落实基于加权最小活跃长连接算法，**明确采用无除法交叉乘法比较**（$\text{Compare}(A, B) \iff C_A \times W_B \text{ vs } C_B \times W_A$），杜绝整除截断丢失精度；定义多节点负载相等时的平滑轮询机制与冷启动选路策略。
- **RAII 连接泄漏终极防护**：设计 `ActiveConnectionGuard` 模式，利用 Rust 的 `Drop` trait 机制确保在客户端断连、请求超时、网络重置或 Tokio 任务取消时，`C_i.fetch_sub(1)` 必然触发，绝不发生长连接泄漏。
- **客户端 Abort 中断传导**：设计客户端断开时向后端 Upstream 发送取消信号并释放连接的完整异步传导机制。

### 3. 多协议原生直通与低延迟 SSE 流式引擎 (Passthrough & SSE Engine)
- **直通端点设计**：定义 `/v1/chat/completions`、`/v1/responses`、`/v1/completions`、`/v1/embeddings`、`/v1/rerank` 与 `/v1/models` 的透明转发契约；坚持零跨协议 AST 转换损耗。
- **SSE 流式解析与 Token 计量**：设计非阻塞异步管道的逐 chunk 透传机制；定义 `usage` 提取算法（兼容 `stream_options.include_usage`）；明确缺少 usage 时的兜底策略。
- **流式透支防范策略**：明确日/周配额在流式场景下的准入门槛（$Quota - Used \ge MinReserve$）与超额透支判定策略。

### 4. 后端服务器集群与协议能力预先探测 (Capability Probing & Health)
- **探测策略与凭证注入**：设计后端添加与心跳探测时的能力识别机制，必须注入后端托管凭据；优先采用 `OPTIONS` / `HEAD` 或轻量探测包，精准区分 200/400（支持）与 404/405（不支持）。
- **能力位掩码与内存状态**：定义后端节点内存结构中的协议能力位图（`ProtocolType` bitflags / HashSet）与健康状态机（Healthy / Degraded / Unhealthy / CircuitBroken）。

### 5. 存储架构与声明式演进方案 (SQLite WAL & Migrations)
- **数据库模式设计**：基于 SQLite WAL 模式，输出完整的 DDL 物理数据模型，包括：
  - `admin_users`（管理员用户与密码 Hash）；
  - `virtual_keys`（虚拟 Key 摘要、归属、日/周配额上限与重置周期）；
  - `backend_servers`（物理后端实例、地址、权重、托管凭据、探测能力集、健康状态）；
  - `models` 与 `model_backends`（逻辑模型定义及其后端的关联与权重映射）；
  - `audit_logs`（审计调用流水、Token 消耗、请求耗时、状态码、Trace ID）；
  - `quota_usages`（租户日/周周期 Token 累计表）。
- **索引与查询优化**：为审计检索设计前缀/组合索引，保障 10 万级日志下响应 $\le 50\text{ms}$。
- **声明式迁移机制**：定义 `migrations/` 目录的 DDL 文件加载、校验与自升迁逻辑。

### 6. 异步非阻塞审计存储管道 (Async Audit Pipeline)
- **解耦设计**：采用 Tokio 有界通道（`tokio::sync::mpsc::channel`）解耦代理热路径与 SQLite 磁盘 I/O。
- **批量刷盘 Worker**：定义审计后台 Worker 的批量写入策略（基于时间窗口如 500ms 或批次大小如 100 条触发 `BEGIN TRANSACTION ... COMMIT`）。
- **背压与溢出策略**：定义通道满时的安全降级策略与指标告警。

### 7. 嵌入式 Web 控制台与单二进制打包方案 (Single Binary Embedded Web)
- **静态资源内嵌方案**：设计通过 `rust-embed` 将前端产物打包至二进制文件的集成流程与 HTTP 静态文件路由分发（含 SPA Fallback 机制）。
- **API 隔离与统一端口**：定义网关代理请求路由与管理后台 REST API 路由的分发策略（支持统一端口或独立端口监听）。

### 8. 代码工程结构与子模块划分 (Project Structure & Module Organization)
- 推导并确定 `src/` 与 `web/` 的模块与目录层级规范（高内聚、低耦合、清晰分层），明确各个模块的职责边界：
  - 路由与代理模块 (`gateway/`, `proxy/`)；
  - 调度与后端池模块 (`routing/`, `backend/`)；
  - 鉴权与流控模块 (`auth/`, `limiting/`)；
  - 异步审计与存储模块 (`audit/`, `storage/`)；
  - 管理 API 与内嵌 Web 模块 (`admin/`, `embedded/`)；
  - 基础公共与配置模块 (`config/`, `error/`, `telemetry/`)。

---

## 交付产物与格式要求
1. **目标文件**：[`docs/02-design/01-design.md`](file:///home/fugui/rusts/rs-gate/docs/02-design/01-design.md)
2. **图表规范**：包含高清晰度的 Mermaid 架构图、数据流图与序列图。
3. **技术红线**：严禁伪代码敷衍，关键数据结构必须使用清晰规范的 Rust 类型定义（Struct / Enum / Trait）；关键 SQL DDL 必须完整可执行。
