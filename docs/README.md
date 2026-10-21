# rs-gate 文档知识库 (Docs Hub)

本项目严格采用 **SDD (Spec-Driven Development，规范驱动开发)** 模式，所有设计规范、接口契约与任务追踪均在此目录中进行统一管理与版本控制。

---

## 目录分层架构

```text
docs/
├── 00-prompt/       # 提示词库：记录各阶段 Prompt 资产、初始化指令与演进提示词
│   ├── 01-init.md   # 系统初始化与 SDD 开发总原则指令
│   ├── 02-design-prompt.md # 阶段二系统架构设计提示词指令
│   ├── 03-api-and-tasks-prompt.md # 阶段三接口契约与任务分解提示词指令
│   ├── 04-tasks-prompt.md # 阶段三第 4 步：工程实施与 TDD 任务分解提示词指令
│   └── 05-m1-implementation-prompt.md # 阶段四第 1 步：Milestone 1 基础骨架与存储引擎实施指令
├── 01-features/     # 特性与需求边界：特性清单矩阵 (Feature Matrix)、P0~P2 优先级与 Non-Goals
│   └── 01-feature-matrix.md # 阶段一需求范围与特性矩阵 (Feature Matrix)
├── 02-design/       # 系统技术架构 (SDD)：总分系列架构设计规范
│   ├── 01-system-architecture.md # 系统总体架构、端到端数据流与模块分层
│   ├── 02-storage-and-models.md  # SQLite 存储模型、DDL 与迁移机制
│   ├── 03-routing-and-proxy.md   # 多协议直通、加权调度、RAII 防护与 SSE 引擎
│   ├── 04-limiting-and-audit.md  # 虚拟 Key 鉴权、日周配额与异步审计流水线
│   ├── 05-web-console.md         # Web 控制台与自服务门户架构、交互体系与单二进制内嵌
│   ├── 06-threading-and-concurrency-model.md # 线程模型、M:N 协程调度与高并发容量规范
│   └── 07-runtime-observability-and-diagnostics.md # 运行时可观测性、监控指标与诊断架构规范
├── 03-api/          # 契约规范库：代理协议、控制台 REST API 与存储数据 DDL 契约
│   ├── 01-gateway-api.md # 网关代理端点契约 (OpenAI/vLLM/SGLang 原生直通)
│   ├── 02-admin-api.md   # 管理控制台后端 REST 契约
│   └── 03-database-schema.md # SQLite 物理数据契约、字段字典与 DDL 规范
├── 04-tasks/        # 实施规划与验收：Milestone 拆解、任务清单与自动化验收测试标准 (AC)
│   ├── 01-roadmap.md                   # 工程实施总览、Milestone 依赖 DAG 图与门禁准则
│   ├── 02-m1-foundation-and-storage.md # M1 作战卡: 基础骨架、配置管理与 SQLite WAL 引擎
│   ├── 03-m2-proxy-and-wlac.md         # M2 作战卡: 原生直通代理、WLAC 调度与零缓冲 SSE 管道
│   ├── 04-m3-security-and-audit.md     # M3 作战卡: 虚拟 Key 鉴权、日周配额流控与重试审计
│   ├── 05-m4-web-console-portal.md     # M4 作战卡: Web 控制台、自服务门户与单二进制嵌入
│   └── 06-m5-production-hardening.md   # M5 作战卡: 生产就绪、运维监控与综合发布加固
├── 05-adr/          # 架构决策记录 (ADR)：重大技术选型与权衡记录 (Architecture Decision Records)
│   └── 01-xxx.md    # 具体架构决策
└── 06-ops/          # 交付与运维：部署指南、配置模板、性能压测基准与监控指标
    └── 01-xxx.md    # 运维与发布规范
```

---

## 目录治理规则

1. **全链路双位编号递进**：
   - 一级子目录采用阶段双位数字前缀（`00-` ~ `06-`）；
   - 各阶段目录下的**所有 Markdown 文档必须带双位数字编号前缀**（如 `01-feature-matrix.md`、`01-design.md`、`01-tasks.md`），保证在文件树、IDE 及构建脚本中保持严格时序与逻辑有序性。
2. **规范先行**：任何代码实施前，必须确保 `01-features/`、`02-design/`、`03-api/` 与 `04-tasks/` 中有明确对应的设计与验收标准。
3. **决策可溯**：凡涉及关键技术选型（如存储方案、并发模型、前端内嵌方式、调度算法），须在 `05-adr/` 中以标准 ADR 模板（编号如 `01-storage-sqlite-wal.md`）记录背景与决策理由。
