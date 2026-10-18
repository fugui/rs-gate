# rs-gate 文档知识库 (Docs Hub)

本项目严格采用 **SDD (Spec-Driven Development，规范驱动开发)** 模式，所有设计规范、接口契约与任务追踪均在此目录中进行统一管理与版本控制。

---

## 目录分层架构

```text
docs/
├── 00-prompt/       # 提示词库：记录各阶段 Prompt 资产、初始化指令与演进提示词
│   └── 01-init.md   # 系统初始化与 SDD 开发总原则指令
├── 01-features/     # 特性与需求边界：特性清单矩阵 (Feature Matrix)、P0~P2 优先级与 Non-Goals
│   └── 01-feature-matrix.md # 阶段一需求范围与特性矩阵 (Feature Matrix)
├── 02-design/       # 系统技术架构 (SDD)：端到端架构设计、数据模型、流式转发与模块组织
│   └── 01-design.md # 阶段二系统技术架构设计
├── 03-api/          # 接口契约规范：OpenAI 兼容代理协议与管理控制台 REST API 定义
│   ├── 01-gateway-api.md # 网关代理端点契约 (OpenAI/vLLM/SGLang 直通)
│   └── 02-admin-api.md   # 管理控制台后端 REST 契约
├── 04-tasks/        # 实施规划与验收：Milestone 拆解、任务清单与自动化验收测试标准 (AC)
│   └── 01-tasks.md  # 阶段三任务拆解与验收标准清单
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
