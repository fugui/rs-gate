# rs-gate: 阶段四 Milestone 1 基础骨架与存储引擎实施提示词指令

- **文档编号**：`05-m1-implementation-prompt.md`
- **编制日期**：2026-10-19
- **当前阶段**：阶段四：工程实施与交付 (第 1 步：Milestone 1 基础骨架与存储引擎实施篇)
- **基准依赖**：
  - 需求范围：[`docs/01-features/01-feature-matrix.md`](file:///home/fugui/rusts/rs-gate/docs/01-features/01-feature-matrix.md)
  - 存储设计：[`docs/02-design/01-system-architecture.md`](file:///home/fugui/rusts/rs-gate/docs/02-design/01-system-architecture.md), [`docs/02-design/02-storage-and-models.md`](file:///home/fugui/rusts/rs-gate/docs/02-design/02-storage-and-models.md)
  - 数据契约：[`docs/03-api/03-database-schema.md`](file:///home/fugui/rusts/rs-gate/docs/03-api/03-database-schema.md)
  - 实施作战卡：[`docs/04-tasks/01-roadmap.md`](file:///home/fugui/rusts/rs-gate/docs/04-tasks/01-roadmap.md), [`docs/04-tasks/02-m1-foundation-and-storage.md`](file:///home/fugui/rusts/rs-gate/docs/04-tasks/02-m1-foundation-and-storage.md)
- **目标产物**：
  - 构建配置：`Cargo.toml`
  - 声明式迁移：`migrations/20261019000000_init.sql`, `migrations/20261019000001_indexes.sql`
  - 测试夹具与红灯用例：`tests/common/mod.rs`, `tests/storage_migration_test.rs`, `tests/health_probe_test.rs`
  - 核心源码：`src/main.rs`, `src/server.rs`, `src/telemetry.rs`, `src/config/`, `src/storage/`

---

## 1. 角色定位与工程原则

你是一名精通高性能系统编程的资深 Rust 架构师与 TDD 倡导者。我们正在采用 **规范驱动开发 (Spec-Driven Development, SDD)** 模式与 **测试驱动开发 (Test-Driven Development, TDD)** 理念开发 **rs-gate（轻量级、单二进制交付的企业级内部大模型网关）**。

前期三阶段的所有规范、架构、API 契约以及模块化任务卡已全部定稿并推送到 Git 仓库。
现在，我们正式启动 **阶段四：工程实施与交付** 的第一个关键里程碑：
**「Milestone 1：基础骨架、配置管理与 SQLite WAL 存储引擎 (Foundation & Storage Engine)」**。

---

## 2. 严格遵循 TDD 闭环研发流程 (Red-Green-Refactor)

实施过程严禁跳过测试直接堆砌业务代码，必须严格执行三阶段 TDD 循环：

```
+-----------------------------------------------------------------------------------+
| 1. 红灯阶段 (Red Phase: Failing Tests First)                                      |
|    - 配置初始 Cargo.toml 依赖定义与测试工具库                                        |
|    - 先行实现 tests/common/mod.rs 临时 SQLite 数据库测试夹具 (TestDbHarness)           |
|    - 编写 tests/storage_migration_test.rs (迁移幂等性、7张表完整性、外键与索引、超管验证)|
|    - 编写 tests/health_probe_test.rs (存活探针 GET /health/live 测试)                 |
|    - 确认测试处于红灯 (Failing) 状态 (未编译或断言失败)                               |
+-----------------------------------------------------------------------------------+
                                          |
                                          v
+-----------------------------------------------------------------------------------+
| 2. 绿灯阶段 (Green Phase: Minimum Working Code)                                   |
|    - 落地 migrations/ 下两篇初始建表与索引 DDL                                       |
|    - 实现 src/config/ (YAML/TOML 读取 + RSGATE_* 环境变量覆盖 + 默认值)             |
|    - 实现 src/telemetry.rs (tracing-subscriber JSON/紧凑控制台日志)                |
|    - 实现 src/storage/db.rs (WAL 模式、Busy Timeout 5000ms、Foreign Keys 等 Pragmas)|
|    - 实现 src/storage/migration.rs (sqlx::migrate! 声明式嵌入式版本化自迁移)         |
|    - 实现 src/storage/bootstrap.rs (初次冷启动自动生成 admin 账号与 Argon2 哈希)     |
|    - 实现 src/server.rs 与 src/main.rs (Axum 基础路由、/health/live 挂载与平滑停机)  |
|    - 验证所有红灯测试用例转为绿灯 (All Tests Passed)                                  |
+-----------------------------------------------------------------------------------+
                                          |
                                          v
+-----------------------------------------------------------------------------------+
| 3. 重构与门禁阶段 (Refactor & Zero-Warning Gating)                                 |
|    - 消除代码坏味道与隐式克隆，确保生产代码零裸露 unwrap()/expect()                  |
|    - 执行 cargo fmt --check 确保格式化零差异                                         |
|    - 执行 cargo clippy --all-targets -- -D warnings 确保零警告通过                   |
|    - 运行 cargo test --all 确保全量测试通过                                          |
+-----------------------------------------------------------------------------------+
```

---

## 3. 详细实施任务与技术细则 (Tasks Checklist)

请严格参照 [`docs/04-tasks/02-m1-foundation-and-storage.md`](file:///home/fugui/rusts/rs-gate/docs/04-tasks/02-m1-foundation-and-storage.md) 的技术规范落地以下任务：

### 3.1 Task 1.1：`Cargo.toml` 生产级依赖拓扑
- 引入：`tokio` (full), `axum` (macros), `tower`, `tower-http` (cors, trace), `sqlx` (runtime-tokio, sqlite, macros, migrate), `serde`, `serde_json`, `serde_yaml`, `tracing`, `tracing-subscriber`, `argon2`, `sha2`, `rand`, `subtle`, `thiserror`, `anyhow`, `uuid`, `config`, `tempfile`, `reqwest` 等。

### 3.2 Task 1.2：全局多源配置管理系统 (`src/config/`)
- 支持 `AppConfig`、`ServerConfig`、`DatabaseConfig`、`SecurityConfig`；
- 遵循多源优先级：环境变量（`RSGATE_SERVER__PORT` 覆盖 `server.port`）> 配置文件（`config.yaml`）> 结构体代码默认值。

### 3.3 Task 1.3：结构化追踪日志 (`src/telemetry.rs`)
- 初始化 `tracing-subscriber`，支持 `RUST_LOG` 动态级别，支持 JSON 格式（生产）与紧凑彩色控制台格式（开发）。

### 3.4 Task 1.4：SQLite WAL 连接池与 Pragmas 调优 (`src/storage/db.rs`)
- 强制注入 WAL 模式、Normal 同步、Busy Timeout 5000ms、Foreign Keys 开启、内存缓存调优；
- 建立线程安全 `SqlitePool`。

### 3.5 Task 1.5：声明式迁移运行器与 DDL 固化 (`migrations/`, `src/storage/migration.rs`)
- 落地 `migrations/20261019000000_init.sql`（7 张物理表）与 `migrations/20261019000001_indexes.sql`（业务联合索引）；
- 利用 `sqlx::migrate!()` 实现启动时全自动、幂等自迁移。

### 3.6 Task 1.6：超级管理员自动初始化 (`src/storage/bootstrap.rs`)
- 冷启动检测 `users` 表，若为空且未设预设密码，生成 16 字符高熵强随机密码并在终端打印警告，Argon2 哈希持久化入库，角色设为 `superadmin`。

### 3.7 Task 1.7：Axum 存活探针与主服务编排 (`src/server.rs`, `src/main.rs`)
- 注册 `GET /health/live` 端点返回 200 OK 与 `{"status": "alive"}`；
- 编排启动流程，监听并优雅捕获 `SIGINT`/`SIGTERM` 停机信号。

---

## 4. 自动化验证指令与验收红线

在实施完成后，必须在终端执行以下命令并确保 100% 通过：

```bash
# 1. 运行存储迁移、索引与超管初始化测试
cargo test --test storage_migration_test -- --nocapture

# 2. 运行存活探针测试
cargo test --test health_probe_test -- --nocapture

# 3. 运行全量单元与集成测试
cargo test --all

# 4. 代码格式化检查
cargo fmt --check

# 5. 静态代码分析（零警告红线）
cargo clippy --all-targets -- -D warnings
```

---

## 5. 自动交付与 Push 准则

代码修改并检视完毕后：
1. 自检代码逻辑、错误处理强类型化与内存安全；
2. 确认 `cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 全部无报错、零警告；
3. 执行 git 提交并直接 push 到 GitHub 远端仓库（`origin/main`），无需等待口头指令。
