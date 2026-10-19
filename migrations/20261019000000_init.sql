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
    is_active INTEGER NOT NULL DEFAULT 1,
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
