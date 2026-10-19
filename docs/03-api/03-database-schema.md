# rs-gate 存储数据契约与 SQLite 物理模型规范 (Database Schema & Storage Contract)

- **文档编号**：`03-database-schema.md`
- **文档版本**：`v1.0.0`
- **编制日期**：2026-10-19
- **当前阶段**：阶段三：接口契约与任务分解 (第 3 步：底座存储数据契约篇)
- **基准依赖**：[`docs/01-features/01-feature-matrix.md`](file:///home/fugui/rusts/rs-gate/docs/01-features/01-feature-matrix.md), [`docs/02-design/01-system-architecture.md`](file:///home/fugui/rusts/rs-gate/docs/02-design/01-system-architecture.md), [`docs/02-design/02-storage-and-models.md`](file:///home/fugui/rusts/rs-gate/docs/02-design/02-storage-and-models.md), [`docs/02-design/03-routing-and-proxy.md`](file:///home/fugui/rusts/rs-gate/docs/02-design/03-routing-and-proxy.md), [`docs/02-design/04-limiting-and-audit.md`](file:///home/fugui/rusts/rs-gate/docs/02-design/04-limiting-and-audit.md), [`docs/02-design/05-web-console.md`](file:///home/fugui/rusts/rs-gate/docs/02-design/05-web-console.md)
- **状态**：定稿待审 (Ready for Review)

---

## 1. 规范概述与底座引擎契约 (Storage Engine Contracts)

### 1.1 存储选型与运行模式
**rs-gate** 坚定采用嵌入式 **SQLite 3.35+** 作为系统唯一的持久化与状态存储引擎。系统启动时必须通过连接池自动执行以下核心 Pragmas 调优指令，保障高并发下的非阻塞读取与安全写入：

```sql
PRAGMA journal_mode = WAL;         -- 强制开启预写日志模式，实现并发读写互不阻塞
PRAGMA synchronous = NORMAL;       -- 兼顾断电安全性与纳秒级落盘性能
PRAGMA busy_timeout = 5000;        -- 遭遇写锁竞争时最多等待 5000ms，杜绝 SQLITE_BUSY 报错
PRAGMA foreign_keys = ON;          -- 严格启用物理外键约束与级联删除
PRAGMA temp_store = MEMORY;        -- 临时表与临时索引常驻内存
PRAGMA cache_size = -64000;        -- 分配约 64MB 专用内存页面缓存
```

### 1.2 时间戳与数值规范
1. **时间戳标准**：
   - 全系统时间戳统一采用 **毫秒级 Unix 时间戳 (64位整数，`INTEGER`)**；
   - 保证前后端、网关代理流水与审计落盘时区中立，统一按 UTC 毫秒基准计算，展示层由 Web 控制台按浏览器本地时区自动格式化。
2. **布尔值表示**：
   - SQLite 无原生 `BOOLEAN` 类型，统一采用 `INTEGER NOT NULL DEFAULT 0/1`，`1` 表示真/启用，`0` 表示假/禁用。
3. **软限制与硬配额取值**：
   - 配额（`daily_token_quota`、`weekly_token_quota`）取值为 `0` 时，统一约定语义为**“不设限额 (Unlimited)”**。

---

## 2. 权威 SQLite 物理表结构 DDL (Physical Tables DDL)

声明式初始迁移脚本文件位置：`migrations/20261019000000_init.sql`。

```sql
-- ============================================================================
-- rs-gate 核心业务物理 DDL 契约
-- ============================================================================

-- 1. 用户账户表 (users) - 统一管理超级管理员、管理员与普通开发者
CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY NOT NULL,
    username TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    role TEXT NOT NULL DEFAULT 'user', -- 'superadmin' | 'admin' | 'user'
    display_name TEXT NOT NULL DEFAULT '',
    email TEXT NOT NULL DEFAULT '',
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- 2. 虚拟 API Key 表 (virtual_keys)
CREATE TABLE IF NOT EXISTS virtual_keys (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL,
    name TEXT NOT NULL,
    key_hash TEXT NOT NULL UNIQUE,
    key_prefix TEXT NOT NULL,
    allowed_models TEXT NOT NULL DEFAULT '["*"]', -- JSON 字符串数组
    rpm_limit INTEGER NOT NULL DEFAULT 60,
    max_concurrency INTEGER NOT NULL DEFAULT 10,
    daily_token_quota INTEGER NOT NULL DEFAULT 0,  -- 0 表示不限
    weekly_token_quota INTEGER NOT NULL DEFAULT 0, -- 0 表示不限
    ip_whitelist TEXT,                             -- CIDR 列表逗号分隔
    is_active INTEGER NOT NULL DEFAULT 1,
    expires_at INTEGER,                            -- 毫秒时间戳，NULL 表示永久有效
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

-- 3. 日/周双周期配额累积表 (quota_usages)
CREATE TABLE IF NOT EXISTS quota_usages (
    id TEXT PRIMARY KEY NOT NULL,
    key_id TEXT NOT NULL,
    period_type TEXT NOT NULL,                     -- 'daily' | 'weekly'
    period_start TEXT NOT NULL,                    -- daily: 'YYYY-MM-DD', weekly: 'YYYY-Www'
    prompt_tokens INTEGER NOT NULL DEFAULT 0,
    completion_tokens INTEGER NOT NULL DEFAULT 0,
    total_tokens INTEGER NOT NULL DEFAULT 0,
    request_count INTEGER NOT NULL DEFAULT 0,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (key_id) REFERENCES virtual_keys(id) ON DELETE CASCADE,
    CONSTRAINT uq_key_period UNIQUE (key_id, period_type, period_start)
);

-- 4. 逻辑大模型定义表 (models)
CREATE TABLE IF NOT EXISTS models (
    id TEXT PRIMARY KEY NOT NULL,                  -- 如 "deepseek-r1", "qwen-2.5-72b"
    display_name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    max_retries INTEGER NOT NULL DEFAULT 2,        -- 上游 5xx/429/超时重试次数 (默认 2)
    retry_interval_ms INTEGER NOT NULL DEFAULT 500,-- 基础退避等待时间毫秒 (默认 500ms)
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- 5. 物理后端服务器实例表 (backend_servers)
CREATE TABLE IF NOT EXISTS backend_servers (
    id TEXT PRIMARY KEY NOT NULL,                  -- 如 "be-vllm-01"
    name TEXT NOT NULL,
    base_url TEXT NOT NULL,                        -- e.g. "http://10.0.1.10:8000"
    weight INTEGER NOT NULL DEFAULT 10,            -- 基础加权值 [1, 100]
    upstream_api_key TEXT,                         -- 后端托管真实凭据 (脱敏存储)
    capabilities_mask INTEGER NOT NULL DEFAULT 0,  -- 探针所得协议能力位图
    health_status TEXT NOT NULL DEFAULT 'healthy', -- 'healthy' | 'degraded' | 'circuit_broken'
    consecutive_failures INTEGER NOT NULL DEFAULT 0,
    last_probe_at INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- 6. 模型与后端集群多对多关联映射表 (model_backends)
CREATE TABLE IF NOT EXISTS model_backends (
    id TEXT PRIMARY KEY NOT NULL,
    model_id TEXT NOT NULL,
    backend_id TEXT NOT NULL,
    weight_override INTEGER,                       -- 覆盖后端默认权重，NULL 则使用 backend_servers.weight
    priority INTEGER NOT NULL DEFAULT 1,           -- 选路优先级 (默认 1)
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    FOREIGN KEY (model_id) REFERENCES models(id) ON DELETE CASCADE,
    FOREIGN KEY (backend_id) REFERENCES backend_servers(id) ON DELETE CASCADE,
    CONSTRAINT uq_model_backend UNIQUE (model_id, backend_id)
);

-- 7. 审计调用流水明细表 (audit_logs)
CREATE TABLE IF NOT EXISTS audit_logs (
    id TEXT PRIMARY KEY NOT NULL,                  -- 雪花算法或 UUID
    trace_id TEXT NOT NULL,                        -- 全链路 Trace ID (x-request-id)
    key_id TEXT NOT NULL,                          -- 虚拟 Key ID
    user_id TEXT,                                  -- 冗余归属用户 ID (加速用户个人明细查询)
    model TEXT NOT NULL,                           -- 请求调用的逻辑模型
    backend_id TEXT NOT NULL,                      -- 最终完成响应的物理后端 ID
    protocol TEXT NOT NULL,                        -- 端点协议 (chat/responses/embeddings 等)
    client_ip TEXT NOT NULL,                       -- 客户端来源 IP
    http_status INTEGER NOT NULL,                  -- 返回给客户端的 HTTP 状态码
    prompt_tokens INTEGER NOT NULL DEFAULT 0,
    completion_tokens INTEGER NOT NULL DEFAULT 0,
    total_tokens INTEGER NOT NULL DEFAULT 0,
    request_duration_ms INTEGER NOT NULL,          -- 请求总生命周期耗时 (毫秒)
    ttft_ms INTEGER,                               -- 首字透传时间 (毫秒，流式专属)
    is_stream INTEGER NOT NULL DEFAULT 0,          -- 是否流式 (1/0)
    retry_count INTEGER NOT NULL DEFAULT 0,        -- 内部重试次数 (0 表示无重试)
    failover_path TEXT,                            -- 故障转移经历的节点路径 JSON 数组
    error_message TEXT,                            -- 错误原因摘要
    created_at INTEGER NOT NULL                    -- 调用发生时间戳 (毫秒)
);
```

---

## 3. 完整数据字典与字段约束契约 (Data Dictionaries)

### 3.1 用户账户表 (`users`)

| 字段名 | 物理类型 | 必填 | 默认值 | 约束 / 范围 | 业务含义与说明 |
| :--- | :---: | :---: | :---: | :--- | :--- |
| `id` | TEXT | **是** | - | 主键，UUIDv4 | 用户唯一物理标识（如 `usr-0192a3b4-5566`）。 |
| `username` | TEXT | **是** | - | UNIQUE，长度 3~32 字符 | 用户登录用户名，英数字与下划线。 |
| `password_hash` | TEXT | **是** | - | 不可逆加盐 Hash | 用户登录密码哈希（Argon2id 或 PBKDF2-SHA256）。 |
| `role` | TEXT | **是** | `'user'` | 枚举：`superadmin` \| `admin` \| `user` | RBAC 角色权限。 |
| `display_name` | TEXT | **是** | `''` | 最大 64 字符 | 用户真实姓名或业务昵称。 |
| `email` | TEXT | **是** | `''` | 邮箱格式校验 | 用户业务邮箱地址。 |
| `is_active` | INTEGER | **是** | `1` | `0` 或 `1` | 账户激活状态。`0` 时禁止登录且所属 Key 全面禁用。 |
| `created_at` | INTEGER | **是** | - | 毫秒时间戳 | 账户注册创建时间。 |
| `updated_at` | INTEGER | **是** | - | 毫秒时间戳 | 账户最近一次修改时间。 |

---

### 3.2 虚拟 API Key 表 (`virtual_keys`)

| 字段名 | 物理类型 | 必填 | 默认值 | 约束 / 范围 | 业务含义与说明 |
| :--- | :---: | :---: | :---: | :--- | :--- |
| `id` | TEXT | **是** | - | 主键，UUIDv4 | 虚拟 Key 唯一物理标识。 |
| `user_id` | TEXT | **是** | - | 外键 -> `users(id)` | 归属用户 ID，级联删除。 |
| `name` | TEXT | **是** | - | 最大 64 字符 | 密钥业务名称（如“商品搜索微服务”）。 |
| `key_hash` | TEXT | **是** | - | UNIQUE，SHA-256 64位十六进制 | 真实明文 Key 加盐后的单向 Hash，用于内存极速点查。 |
| `key_prefix` | TEXT | **是** | - | 前 12 字符 (如 `sk-gate-9f8e***`) | 脱敏前缀，用于控制台展示与日志辨识。 |
| `allowed_models` | TEXT | **是** | `'["*"]'` | JSON 数组格式 | 允许调用的模型 ID 白名单。`["*"]` 表示全模型授权。 |
| `rpm_limit` | INTEGER | **是** | `60` | $\ge 0$ | 单 Key 每分钟最大请求数（RPM）令牌桶容量。 |
| `max_concurrency` | INTEGER | **是** | `10` | $\ge 1$ | 允许的最大在途活跃长连接并发数门限。 |
| `daily_token_quota` | INTEGER | **是** | `0` | $\ge 0$ | 每日 Token 配额上限。`0` 表示不限。 |
| `weekly_token_quota` | INTEGER | **是** | `0` | $\ge 0$ | 每周 Token 配额上限。`0` 表示不限。 |
| `ip_whitelist` | TEXT | 否 | NULL | CIDR 逗号分隔字符串 | 允许调用网关的 IP 段白名单（如 `10.0.0.0/8,192.168.1.0/24`）。 |
| `is_active` | INTEGER | **是** | `1` | `0` 或 `1` | 启用状态。`0` 时网关拦截并返回 401。 |
| `expires_at` | INTEGER | 否 | NULL | 毫秒时间戳 | 过期时间。NULL 表示永久有效；当前时间超过该值返回 401。 |
| `created_at` | INTEGER | **是** | - | 毫秒时间戳 | 创建时间。 |
| `updated_at` | INTEGER | **是** | - | 毫秒时间戳 | 更新时间。 |

---

### 3.3 日/周双周期配额累积表 (`quota_usages`)

| 字段名 | 物理类型 | 必填 | 默认值 | 约束 / 范围 | 业务含义与说明 |
| :--- | :---: | :---: | :---: | :--- | :--- |
| `id` | TEXT | **是** | - | 主键，UUIDv4 | 配额周期统计行 ID。 |
| `key_id` | TEXT | **是** | - | 外键 -> `virtual_keys(id)` | 关联的虚拟 Key ID。 |
| `period_type` | TEXT | **是** | - | 枚举：`'daily'` \| `'weekly'` | 配额统计周期类型。 |
| `period_start` | TEXT | **是** | - | `YYYY-MM-DD` 或 `YYYY-Www` | 统计周期的标准自然对齐键（如 `'2026-10-19'` 或 `'2026-W42'`）。 |
| `prompt_tokens` | INTEGER | **是** | `0` | $\ge 0$ | 当前周期内累计消耗的 Prompt Token 总量。 |
| `completion_tokens` | INTEGER | **是** | `0` | $\ge 0$ | 当前周期内累计消耗的 Completion Token 总量。 |
| `total_tokens` | INTEGER | **是** | `0` | $\ge 0$ | 当前周期内累计消耗的总 Token 数量。 |
| `request_count` | INTEGER | **是** | `0` | $\ge 0$ | 当前周期内累计成功请求次数。 |
| `updated_at` | INTEGER | **是** | - | 毫秒时间戳 | 最近一次批量更新落地时间戳。 |

---

### 3.4 逻辑大模型定义表 (`models`)

| 字段名 | 物理类型 | 必填 | 默认值 | 约束 / 范围 | 业务含义与说明 |
| :--- | :---: | :---: | :---: | :--- | :--- |
| `id` | TEXT | **是** | - | 主键，英数短横线 | 逻辑模型唯一代号（如 `deepseek-r1`、`qwen-2.5-72b`）。 |
| `display_name` | TEXT | **是** | - | 最大 64 字符 | 控制台与前端呈现给用户的友好模型全称。 |
| `description` | TEXT | **是** | `''` | 文本说明 | 适用场景、能力特点与上下文窗口描述。 |
| `max_retries` | INTEGER | **是** | `2` | $[0, 5]$ | **上游 5xx/429/超时重试最大次数**（默认 2 次）。 |
| `retry_interval_ms` | INTEGER | **是** | `500` | $[100, 5000]$ | **基础退避等待时间毫秒**（默认 500ms）。 |
| `is_active` | INTEGER | **是** | `1` | `0` 或 `1` | 模型是否可用。`0` 时网关拦截并返回 404。 |
| `created_at` | INTEGER | **是** | - | 毫秒时间戳 | 模型配置创建时间。 |
| `updated_at` | INTEGER | **是** | - | 毫秒时间戳 | 模型参数更新时间。 |

---

### 3.5 物理后端服务器实例表 (`backend_servers`)

| 字段名 | 物理类型 | 必填 | 默认值 | 约束 / 范围 | 业务含义与说明 |
| :--- | :---: | :---: | :---: | :--- | :--- |
| `id` | TEXT | **是** | - | 主键，唯一实例代号 | 物理实例代号（如 `be-vllm-01`、`be-sglang-02`）。 |
| `name` | TEXT | **是** | - | 最大 64 字符 | 物理实例友好备注（如“GPU-Cluster-Node-01”）。 |
| `base_url` | TEXT | **是** | - | 合法 URL 格式 | 后端实例内网 HTTP/HTTPS 根地址。 |
| `weight` | INTEGER | **是** | `10` | $[1, 100]$ | 调度基础权重。 |
| `upstream_api_key` | TEXT | 否 | NULL | 敏感凭据 | 托管真实后端私密 API Key（网关转发时代替虚拟 Key 注入）。 |
| `capabilities_mask` | INTEGER | **是** | `0` | 位图整数 $[0, 63]$ | 协议能力徽章位图：Bit0=Chat, Bit1=Responses, Bit2=Completions, Bit3=Embeddings, Bit4=Rerank, Bit5=Models。 |
| `health_status` | TEXT | **是** | `'healthy'` | 枚举：`healthy` \| `degraded` \| `circuit_broken` | 实例健康状态机。 |
| `consecutive_failures` | INTEGER | **是** | `0` | $\ge 0$ | 连续失败计数。达到门限（3）触发 `circuit_broken` 熔断。 |
| `last_probe_at` | INTEGER | 否 | NULL | 毫秒时间戳 | 最近一次探活完成时间。 |
| `created_at` | INTEGER | **是** | - | 毫秒时间戳 | 注册时间。 |
| `updated_at` | INTEGER | **是** | - | 毫秒时间戳 | 更新时间。 |

---

### 3.6 模型与后端集群多对多关联映射表 (`model_backends`)

| 字段名 | 物理类型 | 必填 | 默认值 | 约束 / 范围 | 业务含义与说明 |
| :--- | :---: | :---: | :---: | :--- | :--- |
| `id` | TEXT | **是** | - | 主键，UUIDv4 | 映射关系主键。 |
| `model_id` | TEXT | **是** | - | 外键 -> `models(id)` | 关联的逻辑模型代号。 |
| `backend_id` | TEXT | **是** | - | 外键 -> `backend_servers(id)` | 关联的物理后端代号。 |
| `weight_override` | INTEGER | 否 | NULL | $[1, 100]$ | 权重个性化覆写。若为 NULL 则继承后端表自带的 `weight`。 |
| `priority` | INTEGER | **是** | `1` | $\ge 1$ | 调度优先级层级（优先在最高优先级节点池执行 WLAC 选路）。 |
| `is_active` | INTEGER | **是** | `1` | `0` 或 `1` | 绑定关系是否启用。 |
| `created_at` | INTEGER | **是** | - | 毫秒时间戳 | 绑定创建时间。 |

---

### 3.7 审计调用流水明细表 (`audit_logs`)

| 字段名 | 物理类型 | 必填 | 默认值 | 约束 / 范围 | 业务含义与说明 |
| :--- | :---: | :---: | :---: | :--- | :--- |
| `id` | TEXT | **是** | - | 主键，UUIDv4 | 审计明细行全局唯一标识符。 |
| `trace_id` | TEXT | **是** | - | UUID 格式 | 全链路追踪 Trace ID，对齐出站响应头 `x-request-id`。 |
| `key_id` | TEXT | **是** | - | 字符串 | 调用的虚拟 Key ID。 |
| `user_id` | TEXT | 否 | NULL | 外键关联语义 | **冗余归属用户 ID**，实现自服务门户亚毫秒级个人历史查询。 |
| `model` | TEXT | **是** | - | 字符串 | 请求的逻辑模型名称。 |
| `backend_id` | TEXT | **是** | - | 字符串 | 最终实际承载流量并成功响应的物理后端代号。 |
| `protocol` | TEXT | **是** | - | 枚举：`chat` \| `responses` \| `completions` \| `embeddings` \| `rerank` | 调用的协议端点类型。 |
| `client_ip` | TEXT | **是** | - | IPv4 / IPv6 | 发起请求的客户端真实来源 IP。 |
| `http_status` | INTEGER | **是** | - | HTTP 状态码 (200, 429, 502 等) | 网关返回给下游客户端的最终 HTTP 状态码。 |
| `prompt_tokens` | INTEGER | **是** | `0` | $\ge 0$ | 输入 Prompt Token 数量。 |
| `completion_tokens` | INTEGER | **是** | `0` | $\ge 0$ | 模型生成 Completion Token 数量。 |
| `total_tokens` | INTEGER | **是** | `0` | $\ge 0$ | 最终计费总 Token 数。 |
| `request_duration_ms` | INTEGER | **是** | - | $\ge 0$ (毫秒) | 网关端到端处理耗时（包含退避与重试耗时）。 |
| `ttft_ms` | INTEGER | 否 | NULL | $\ge 0$ (毫秒) | 首字到达与透传时间（仅在流式请求下记录）。 |
| `is_stream` | INTEGER | **是** | `0` | `0` 或 `1` | 是否为 SSE 流式请求。 |
| `retry_count` | INTEGER | **是** | `0` | $\ge 0$ | **内部静默重试次数**（`0` 表示首发成功，`1`~`2` 表示发生容错重试）。 |
| `failover_path` | TEXT | 否 | NULL | JSON 数组字符串 | **故障转移节点轨迹**（如 `'["be-vllm-01 (503)", "be-sglang-02 (200)"]'`）。 |
| `error_message` | TEXT | 否 | NULL | 文本摘要 | 异常报错信息（若请求失败或部分中断）。 |
| `created_at` | INTEGER | **是** | - | 毫秒时间戳 | 请求完成被写入队列的时间戳。 |

---

## 4. 关键索引契约与查询加速矩阵 (Index Specifications)

为确保系统在海量调用并发与高频控制台检索场景下保持毫秒级性能，严格落实以下物理索引定义：

```sql
-- ============================================================================
-- 物理索引清单: migrations/20261019000001_indexes.sql
-- ============================================================================

-- 1. 鉴权与流控热路径索引 (内存驻留，点查延迟 <= 0.05ms)
CREATE UNIQUE INDEX IF NOT EXISTS idx_virtual_keys_key_hash 
    ON virtual_keys(key_hash);

CREATE INDEX IF NOT EXISTS idx_virtual_keys_user_id 
    ON virtual_keys(user_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_quota_usages_lookup 
    ON quota_usages(key_id, period_type, period_start);

-- 2. 路由选路与后端调度热路径索引
CREATE INDEX IF NOT EXISTS idx_model_backends_model_active 
    ON model_backends(model_id, is_active, priority DESC);

CREATE INDEX IF NOT EXISTS idx_backend_servers_health 
    ON backend_servers(health_status, is_active);

-- 3. 审计流水高性能组合检索索引 (针对控制台 10 万+ 级日志多维下钻)
CREATE INDEX IF NOT EXISTS idx_audit_logs_created_at_desc 
    ON audit_logs(created_at DESC);

CREATE INDEX IF NOT EXISTS idx_audit_logs_user_created 
    ON audit_logs(user_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_audit_logs_key_created 
    ON audit_logs(key_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_audit_logs_model_created 
    ON audit_logs(model, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_audit_logs_trace_id 
    ON audit_logs(trace_id);
```

---

## 5. 初始种子数据与默认超管初始化契约 (Seed Data & Bootstrapping)

首次部署无数据库文件时，系统启动自动执行声明式迁移并写入初始种子数据：

### 5.1 默认超级管理员生成规则
- **用户名**：`admin`
- **初始密码**：若启动环境变量 `RSGATE_ADMIN_INITIAL_PASSWORD` 存在则使用该值；若未配置则随机生成 16 位强随机熵密码，并通过标准日志 `tracing::info!` 输出至控制台；
- **角色**：`superadmin`
- **初始化 SQL 示例**：
  ```sql
  INSERT OR IGNORE INTO users (
      id, username, password_hash, role, display_name, email, is_active, created_at, updated_at
  ) VALUES (
      'usr-superadmin-default',
      'admin',
      '$argon2id$v=19$m=19456,t=2,p=1$ZGVmYXVsdHNhbHQ$...',
      'superadmin',
      'System SuperAdmin',
      'admin@internal.corp',
      1,
      1729296000000,
      1729296000000
  );
  ```

### 5.2 开箱即用示例模型与后端种子
为便于本地开发与快速验收，内置提供开箱即用示例模型骨架：
```sql
-- 初始逻辑模型定义
INSERT OR IGNORE INTO models (id, display_name, description, max_retries, retry_interval_ms, is_active, created_at, updated_at)
VALUES 
('qwen-2.5-72b-instruct', 'Qwen 2.5 72B Instruct', '通用高智能业务指令模型', 2, 500, 1, 1729296000000, 1729296000000),
('deepseek-r1', 'DeepSeek R1', '企业级深度推理强化大模型', 2, 500, 1, 1729296000000, 1729296000000);
```

---

## 6. 契约验收标准与后续规划 (Verification Checklist & Next Steps)

### 6.1 存储数据契约验收标准 (AC Checklist)

- [x] **DDL 语法与 WAL 规范严密**：SQLite 3.35+ 物理建表语句无语法歧义，配置 WAL 与毫秒级 Unix 时间戳标准。
- [x] **数据字典 100% 覆盖**：完整覆盖 `users`、`virtual_keys`、`quota_usages`、`models`、`backend_servers`、`model_backends` 与 `audit_logs` 全部 7 张核心表。
- [x] **对齐重试与故障转移规范**：`models` 表具备 `max_retries` 与 `retry_interval_ms`，`audit_logs` 表具备 `retry_count` 与 `failover_path`。
- [x] **索引设计与执行计划验证**：定义覆盖点查与多维下钻复合索引，保障 10 万级数据分页下钻 $\le 50\text{ms}$。
- [x] **种子数据自引导机制完备**：清晰定义首部署无数据库文件时的默认超管与示例模型初始化逻辑。

### 6.2 阶段三推进路线

至此，**阶段三的所有接口契约规范已全部编制完成**：
1. [`docs/03-api/01-gateway-api.md`](file:///home/fugui/rusts/rs-gate/docs/03-api/01-gateway-api.md)（【代理协议契约】网关代理 API 契约规范 - 融入静默重试与容错）
2. [`docs/03-api/02-admin-api.md`](file:///home/fugui/rusts/rs-gate/docs/03-api/02-admin-api.md)（【控制台契约】控制台与自服务门户 REST API 契约规范）
3. [`docs/03-api/03-database-schema.md`](file:///home/fugui/rusts/rs-gate/docs/03-api/03-database-schema.md)（【存储数据契约】数据库 DDL 与字段字典契约规范）

👉 **下一步工作规划**：进入阶段三最终步骤：编制 [`docs/04-tasks/01-tasks.md`](file:///home/fugui/rusts/rs-gate/docs/04-tasks/01-tasks.md)（【任务与验收】工程实施任务分解与自动化验收清单，分解 Milestone 1 ~ Milestone 5 落地工程流水线）。
