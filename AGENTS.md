# AGENTS.md - rs-gate 开发与协作准则

本项目为 **rs-gate（轻量级、单二进制交付的企业级内部大模型网关）**。
所有进入本项目的 AI 代理（Agents）必须严格遵循本准则。

---

## 1. 核心工程哲学：规范驱动开发 (SDD)

本项目严禁直接跳过设计堆砌业务代码。所有开发工作必须遵循严格的流水线：
> **「文档规范先行 -> 架构设计定稿 -> 接口契约明晰 -> 任务拆解与验收清单 -> 分阶段实施与验证」**

### 研发阶段递进原则
1. **阶段一：特性清单与边界明确 (`docs/01-features/01-features.md`)**：明确网关、流控安全、管理控制台、审计系统的需求矩阵与 Non-Goals。
2. **阶段二：系统架构设计文档 (`docs/02-design/01-design.md`)**：端到端数据流水线、低延迟 SSE 流式中继、SQLite 存储与索引设计、代码子模块分层定义。
3. **阶段三：接口契约与任务分解 (`docs/03-api/01-gateway-api.md`, `docs/04-tasks/01-tasks.md`)**：OpenAI 兼容与直通协议、控制台 REST API 契约、Milestone 任务划分与自动化验收标准 (AC)。
4. **阶段四：工程实施与交付 (`src/`, `web/`, `tests/`)**：在上述设计定稿并确认后，方可开展业务代码与测试编写。

---

## 2. 行为准则与自动化交付流水线 (Mandatory Workflow)

每次代码修改与任务实施必须遵循以下交付闭环：
1. **代码检视 (Self-Review)**：每次代码修改完成后，必须对修改内容进行自检（包括逻辑完备性、内存安全、错误处理与文档一致性）。
2. **本地校验 (Verification)**：必须确保本地构建无报错、静态分析零警告（`cargo clippy`）、所有单元与集成测试全部通过。
3. **自动交付 (Auto Push)**：**确认无误且本地构建与测试全部通过后，直接提交并 push 到 GitHub 仓库（`origin/main`），无需在汇报后等待用户口头指示。**

---

## 3. 目录边界与治理规范 (Layout & Policies)

项目一级目录具有强边界隔离，严禁随意跨目录散落代码或新建无约定顶层目录：

```text
rs-gate/
├── docs/            # 规范文档库：00-prompt ~ 06-ops，版本化管理所有设计与契约
├── src/             # 后端核心源码：网关路由、鉴权、流控、SSE 代理与异步审计
├── web/             # 前端控制台源码：管理界面代码，产物供 Rust 单二进制内嵌
├── migrations/      # 数据库版本迁移：声明式 SQLite DDL 迁移脚本
├── tests/           # 集成与回归测试：端到端代理测试、流式测试与压测脚本
├── AGENTS.md        # Agent 行为准则与架构宪法
└── README.md        # 项目主页与开发路线图
```

- **单二进制交付**：`web/` 前端构建产物最终须嵌入 Rust 二进制中（如使用 `rust-embed`），运行时无外部静态资源依赖。
- **配置与数据隔离**：SQLite 数据文件、日志与本地临时文件严禁提交至 Git（由 `.gitignore` 保障）。

---

## 4. Rust 编码规范与技术红线 (Guardrails)

1. **异步与流式非阻塞**：
   - 核心网关代理必须保证高吞吐、低延迟，严禁在异步运行时（Tokio）中执行阻塞式 I/O 或耗时计算。
   - SSE（Server-Sent Events）流式转发必须做到逐 chunk 实时透传，严禁全量缓存后再输出。
2. **严苛代码质量**：
   - 必须通过 `cargo clippy --all-targets -- -D warnings`，保持零警告。
   - 生产业务代码中严禁随意使用 `unwrap()` 或 `expect()`，错误处理必须采用强类型枚举错误或规范传播机制。
3. **并发安全与锁策略**：
   - 流控计数（如令牌桶、滑动窗口）必须优先考虑高性能原子操作（`Atomic*`）或细粒度锁，杜绝全局粗粒度锁竞争。
4. **数据库与持久化**：
   - 仅使用 SQLite WAL 模式，数据库变更必须以版本化 DDL 存放在 `migrations/`，系统启动时具备自初始化与自升级能力。

---

## 5. 常用开发与验证指令速查

```bash
# 代码格式化检查
cargo fmt --check

# 静态代码分析（零警告红线）
cargo clippy --all-targets -- -D warnings

# 运行全量单元与集成测试
cargo test --all

# 查看当前分支与远端状态
git status && git branch -vv

# 标准提交与推送
git add .
git commit -m "feat/docs/fix: <description>"
git push origin main
```
