-- ============================================================================
-- 物理索引清单: migrations/20261019000001_indexes.sql
-- ============================================================================

-- 1. 鉴权与流控热路径索引 (内存驻留，点查延迟 <= 0.05ms)
CREATE UNIQUE INDEX IF NOT EXISTS idx_virtual_keys_key_hash 
    ON virtual_keys(key_hash);

CREATE INDEX IF NOT EXISTS idx_virtual_keys_key_prefix
    ON virtual_keys(key_prefix);

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
CREATE INDEX IF NOT EXISTS idx_audit_logs_created_at 
    ON audit_logs(created_at DESC);

CREATE INDEX IF NOT EXISTS idx_audit_logs_created_at_desc 
    ON audit_logs(created_at DESC);

CREATE INDEX IF NOT EXISTS idx_audit_logs_user_created 
    ON audit_logs(user_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_audit_logs_key_id 
    ON audit_logs(key_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_audit_logs_key_created 
    ON audit_logs(key_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_audit_logs_model_created 
    ON audit_logs(model, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_audit_logs_trace_id 
    ON audit_logs(trace_id);
