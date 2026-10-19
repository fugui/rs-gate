# rs-gate M1 作战卡：基础骨架、配置管理与 SQLite WAL 存储引擎 (Foundation & Storage Engine)

- **文档编号**：`02-m1-foundation-and-storage.md`
- **文档版本**：`v1.0.0`
- **编制日期**：2026-10-19
- **当前阶段**：阶段三：接口契约与任务分解 (第 4 步：M1 独立作战卡篇)
- **基准依赖**：
  - 实施总纲：[`docs/04-tasks/01-roadmap.md`](file:///home/fugui/rusts/rs-gate/docs/04-tasks/01-roadmap.md)
  - 架构设计：[`docs/02-design/01-system-architecture.md`](file:///home/fugui/rusts/rs-gate/docs/02-design/01-system-architecture.md), [`docs/02-design/02-storage-and-models.md`](file:///home/fugui/rusts/rs-gate/docs/02-design/02-storage-and-models.md)
  - 存储契约：[`docs/03-api/03-database-schema.md`](file:///home/fugui/rusts/rs-gate/docs/03-api/03-database-schema.md)
- **状态**：已完成 (Completed - Commit: 37bc1ef)

---

## 1. 本里程碑目标与文件变更树 (Target & File Tree)

### 1.1 核心目标与范围边界
本阶段作为 rs-gate 的基石工程，聚焦于建立系统运行时骨架与可靠的持久化底座：
1. **构建工程与依赖定义**：配置生产级 Rust 依赖拓扑（Tokio、Axum、SQLx、Argon2、Tracing 等）；
2. **多源动态配置系统**：实现分层配置加载（默认值 -> YAML/TOML 配置文件 -> `RSGATE_*` 环境变量覆盖）；
3. **结构化追踪日志**：初始化 `tracing-subscriber`，支持标准控制台与 JSON 格式输出及运行时日志级别控制；
4. **SQLite WAL 连接池与 Pragmas 调优**：建立基于 SQLx 的高性能 SQLite 连接池，启用 WAL 模式、Busy Timeout 5000ms 与内存缓存调优；
5. **声明式数据库自迁移**：启动时自动检测并幂等执行 `migrations/` 目录下的建表与索引 DDL；
6. **超级管理员自动注入**：数据库初次初始化时，自动探查并安全生成初始超级管理员（`admin`）账号；
7. **HTTP 服务骨架与存活探针**：启动 Axum 基础监听，对外暴露 `GET /health/live` 端点返回 200 OK。

### 1.2 涉及文件变更清单

```text
rs-gate/
├── Cargo.toml                              # [已落地交付] 核心依赖清单与二进制构建配置
├── migrations/
│   ├── 20261019000000_init.sql             # [已落地交付] 7 张物理表初始结构 DDL
│   └── 20261019000001_indexes.sql          # [已落地交付] 业务联合索引与覆盖索引 DDL
├── src/
│   ├── lib.rs                              # [已落地交付] 根模块导出
│   ├── main.rs                             # [已落地交付] 应用主入口编排与平滑停机
│   ├── server.rs                           # [已落地交付] Axum 服务生命周期与路由挂载
│   ├── telemetry.rs                        # [已落地交付] Tracing 结构化日志初始化
│   ├── config/
│   │   ├── mod.rs                          # [已落地交付] 配置加载与解析器
│   │   └── options.rs                      # [已落地交付] 配置结构体定义与默认值
│   └── storage/
│       ├── mod.rs                          # [已落地交付] 存储模块根导出
│       ├── db.rs                           # [已落地交付] SQLite 连接池与 Pragmas 配置
│       ├── migration.rs                    # [已落地交付] 声明式自迁移运行器
│       └── bootstrap.rs                    # [已落地交付] 默认超管账号初始化注入
└── tests/
    ├── common/
    │   └── mod.rs                          # [已落地交付] 临时 SQLite 数据库测试夹具 (TestDbHarness)
    ├── storage_migration_test.rs           # [已落地交付] 存储迁移、表结构、外键与索引集成测试
    └── health_probe_test.rs                # [已落地交付] 存活探针 HTTP 接口集成测试
```

---

## 2. 测试夹具与 TDD 先行用例 (TDD Test Harness & Failing Tests - Red Phase)

在编写任何业务实现代码前，必须先完成以下测试夹具与测试用例定义。测试用例在没有业务实现代码支撑时应能被测试框架正常发现并处于**红灯（Failing）**状态。

### 2.1 临时测试数据库夹具 (`tests/common/mod.rs`)

```rust
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use std::path::PathBuf;
use tempfile::TempDir;

/// 隔离的 SQLite 测试环境夹具，在 Drop 时自动清理磁盘文件
pub struct TestDbHarness {
    pub pool: SqlitePool,
    pub temp_dir: TempDir,
    pub db_path: PathBuf,
}

impl TestDbHarness {
    pub async fn new() -> anyhow::Result<Self> {
        let temp_dir = tempfile::tempdir()?;
        let db_path = temp_dir.path().join("test_gate.db");
        let conn_str = format!("sqlite://{}?mode=rwc", db_path.display());
        
        // 初始连接池
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&conn_str)
            .await?;
            
        Ok(Self { pool, temp_dir, db_path })
    }
}
```

### 2.2 存储自迁移与索引完整性测试 (`tests/storage_migration_test.rs`)

定义以下 4 个先行集成测试用例：

```rust
use crate::common::TestDbHarness;

#[tokio::test]
async fn test_migration_idempotency() {
    let harness = TestDbHarness::new().await.unwrap();
    // 第一次执行迁移
    rs_gate::storage::migration::run_migrations(&harness.pool).await.expect("First migration failed");
    // 第二次执行迁移（必须幂等成功）
    rs_gate::storage::migration::run_migrations(&harness.pool).await.expect("Repeated migration must succeed");
}

#[tokio::test]
async fn test_tables_and_foreign_keys_integrity() {
    let harness = TestDbHarness::new().await.unwrap();
    rs_gate::storage::migration::run_migrations(&harness.pool).await.unwrap();

    // 验证 7 张物理表存在性
    let tables: Vec<(String,)> = sqlx::query_as(
        "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name;"
    )
    .fetch_all(&harness.pool)
    .await
    .unwrap();
    
    let table_names: Vec<String> = tables.into_iter().map(|(t,)| t).collect();
    assert!(table_names.contains(&"users".to_string()));
    assert!(table_names.contains(&"virtual_keys".to_string()));
    assert!(table_names.contains(&"quota_usages".to_string()));
    assert!(table_names.contains(&"models".to_string()));
    assert!(table_names.contains(&"backend_servers".to_string()));
    assert!(table_names.contains(&"model_backends".to_string()));
    assert!(table_names.contains(&"audit_logs".to_string()));
}

#[tokio::test]
async fn test_indexes_exist() {
    let harness = TestDbHarness::new().await.unwrap();
    rs_gate::storage::migration::run_migrations(&harness.pool).await.unwrap();

    let indexes: Vec<(String,)> = sqlx::query_as(
        "SELECT name FROM sqlite_master WHERE type='index' AND name NOT LIKE 'sqlite_%';"
    )
    .fetch_all(&harness.pool)
    .await
    .unwrap();
    
    let index_names: Vec<String> = indexes.into_iter().map(|(i,)| i).collect();
    assert!(index_names.contains(&"idx_audit_logs_created_at".to_string()));
    assert!(index_names.contains(&"idx_audit_logs_key_id".to_string()));
    assert!(index_names.contains(&"idx_virtual_keys_key_prefix".to_string()));
}

#[tokio::test]
async fn test_bootstrap_superadmin_creation_and_auth() {
    let harness = TestDbHarness::new().await.unwrap();
    rs_gate::storage::migration::run_migrations(&harness.pool).await.unwrap();

    // 执行首次启动引导
    let admin_info = rs_gate::storage::bootstrap::ensure_superadmin(
        &harness.pool,
        Some("InitialAdminPass!123"),
    ).await.unwrap();

    assert_eq!(admin_info.username, "admin");
    assert_eq!(admin_info.role, "superadmin");

    // 验证密码哈希可通过 Argon2 校验
    let is_valid = rs_gate::storage::bootstrap::verify_password(
        "InitialAdminPass!123",
        &admin_info.password_hash,
    ).unwrap();
    assert!(is_valid);
}
```

### 2.3 存活探针接口测试 (`tests/health_probe_test.rs`)

```rust
use axum::{body::Body, http::{Request, StatusCode}};
use tower::ServiceExt;

#[tokio::test]
async fn test_live_health_probe_returns_200() {
    let app = rs_gate::server::build_app(None).await.unwrap();
    
    let response = app
        .oneshot(
            Request::builder()
                .uri("/health/live")
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(body_json["status"], "alive");
}
```

---

## 3. 分步工程实施任务 (Step-by-Step Implementation - Green Phase)

### 3.1 Task 1.1：`Cargo.toml` 生产级依赖配置
- **目标文件**：[`Cargo.toml`](file:///home/fugui/rusts/rs-gate/Cargo.toml)
- **实现要点**：
  - 引入异步运行时：`tokio = { version = "1.43", features = ["full"] }`
  - 引入 Web 框架：`axum = { version = "0.8", features = ["macros"] }`, `tower = "0.5"`, `tower-http = { version = "0.6", features = ["cors", "trace"] }`
  - 引入数据库驱动：`sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite", "macros", "migrate"] }`
  - 序列化与数据格式：`serde = { version = "1.0", features = ["derive"] }`, `serde_json = "1.0"`, `serde_yaml = "0.9"`
  - 日志与度量：`tracing = "0.1"`, `tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }`
  - 安全与密码学：`argon2 = "0.5"`, `sha2 = "0.10"`, `rand = "0.8"`, `subtle = "2.6"`
  - 辅助工具：`thiserror = "2.0"`, `anyhow = "1.0"`, `uuid = { version = "1.11", features = ["v4", "fast-rng"] }`, `config = "0.15"`
  - 开发测试依赖：`tempfile = "3.17"`, `reqwest = { version = "0.12", features = ["json", "stream"] }`

### 3.2 Task 1.2：全局多源配置管理系统
- **目标文件**：`src/config/options.rs`, `src/config/mod.rs`
- **核心结构定义**：
```rust
#[derive(Debug, Clone, serde::Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub security: SecurityConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ServerConfig {
    pub host: String, // 默认 "0.0.0.0"
    pub port: u16,    // 默认 8080
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct DatabaseConfig {
    pub url: String,               // 默认 "sqlite://data/rsgate.db?mode=rwc"
    pub max_connections: u32,      // 默认 32
    pub min_connections: u32,      // 默认 4
    pub busy_timeout_ms: u64,      // 默认 5000
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SecurityConfig {
    pub default_admin_password: Option<String>,
}
```
- **配置优先级**：
  1. 环境变量（`RSGATE_SERVER__PORT` 覆盖 `server.port`，双下划线表示嵌套层级）；
  2. 配置文件（`config.yaml` 或通过 `--config` 指定）；
  3. 结构体代码默认值（Default Trait）。

### 3.3 Task 1.3：结构化日志追踪系统
- **目标文件**：`src/telemetry.rs`
- **函数签名**：
```rust
pub fn init_telemetry(format: &str, default_level: &str) -> anyhow::Result<()>
```
- **实现要点**：
  - 基于 `EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_level))` 动态响应 `RUST_LOG`；
  - 若 `format == "json"` 输出格式化 JSON 行日志，便于生产采集；否则输出便于本地阅读的紧凑彩色控制台格式。

### 3.4 Task 1.4：SQLite WAL 存储引擎与 Pragmas 调优
- **目标文件**：`src/storage/db.rs`
- **函数签名**：
```rust
pub async fn init_db_pool(config: &DatabaseConfig) -> Result<SqlitePool, sqlx::Error>
```
- **关键实现**：
  - 在 `SqliteConnectOptions` 中注入必须执行的 Pragmas 指令：
    - `PRAGMA journal_mode = WAL;`
    - `PRAGMA synchronous = NORMAL;`
    - `PRAGMA busy_timeout = 5000;`
    - `PRAGMA foreign_keys = ON;`
    - `PRAGMA temp_store = MEMORY;`
    - `PRAGMA cache_size = -64000;`
  - 使用 `SqlitePoolOptions::new().max_connections(config.max_connections).connect_with(opts).await` 建立线程安全连接池。

### 3.5 Task 1.5：声明式建表与索引 DDL 迁移
- **目标文件**：`migrations/20261019000000_init.sql`, `migrations/20261019000001_indexes.sql`, `src/storage/migration.rs`
- **实现要点**：
  - 将阶段三契约中的建表 DDL 与索引 DDL 严格固化在 `migrations/` 目录下；
  - `run_migrations(pool: &SqlitePool) -> Result<(), sqlx::migrate::MigrateError>` 利用 `sqlx::migrate!()` 宏执行嵌入式声明式版本化迁移；
  - 保证系统重复启动或冷热重启时自动跳过已应用版本，零手工干预。

### 3.6 Task 1.6：超管账号自初始化注入 (Bootstrap)
- **目标文件**：`src/storage/bootstrap.rs`
- **函数签名**：
```rust
pub async fn ensure_superadmin(
    pool: &SqlitePool,
    preset_password: Option<&str>,
) -> anyhow::Result<UserInfo>
```
- **实现要点**：
  - 查询 `SELECT COUNT(*) FROM users;`；
  - 若为 0，则判定为初次冷启动：
    - 若配置了 `preset_password` 则采用预设密码；
    - 否则通过 `rand::thread_rng()` 随机生成 16 字符高熵强密码并在终端以高亮 Warning 形式打印；
    - 使用 `argon2::Argon2::default()` 算出密码哈希值并插入 `users` 表，角色标记为 `superadmin`。

### 3.7 Task 1.7：HTTP 服务骨架与存活探针挂载
- **目标文件**：`src/server.rs`, `src/main.rs`
- **实现要点**：
  - 实现 `build_app(pool: Option<SqlitePool>) -> Result<axum::Router, anyhow::Error>`；
  - 注册 `GET /health/live` 端点，返回 JSON `{"status": "alive", "timestamp": ...}`；
  - 在 `main.rs` 中完整串联配置加载、日志初始化、数据库连接池建立、迁移执行、超管账号保障，最后绑定 TCP 端口启动 Tokio 监听，挂载平滑停机信号（`SIGINT`/`SIGTERM`）。

---

## 4. 自动化验收标准与命令行验证手册 (Automated AC & CLI Playbook)

### 4.1 单元与集成测试自动化验证
开发者或 CI 在工程根目录下执行以下命令，必须全部通过：

```bash
# 1. 运行存储迁移、索引与超管初始化集成测试
cargo test --test storage_migration_test -- --nocapture

# 2. 运行存活探针集成测试
cargo test --test health_probe_test -- --nocapture

# 3. 运行全量测试
cargo test --all
```

**验收门禁通过准则**：
- [x] `test_migration_idempotency` 成功通过，重复迁移耗时 $\le 50\text{ms}$；
- [x] `test_tables_and_foreign_keys_integrity` 成功校验 7 张表与外键完整性；
- [x] `test_indexes_exist` 校验联合索引已成功创建；
- [x] `test_bootstrap_superadmin_creation_and_auth` 成功生成超管账号且 Argon2 校验匹配；
- [x] `test_live_health_probe_returns_200` 响应 HTTP 200 与 `{"status":"alive"}`。

### 4.2 命令行黑盒验证手册 (CLI Blackbox Playbook)

```bash
# 启动本地开发服务 (支持端口与数据库动态覆盖)
RSGATE_SERVER__PORT=8080 RSGATE_DATABASE__URL="sqlite://data/rsgate_dev.db?mode=rwc" cargo run

# 若 8080 端口已被本机其他进程占用（如遭遇 os error 98），可通过环境变量灵活变更端口：
export RSGATE_SERVER__PORT=8030
./target/debug/rs-gate

# 验证存活探针响应 (对齐监听端口)
curl -i http://localhost:8030/health/live

# 预期返回结果
# HTTP/1.1 200 OK
# content-type: application/json
# {"status":"alive","timestamp":1792413046761}
```
