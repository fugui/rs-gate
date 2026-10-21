export interface BackendInstance {
  id: string;
  name: string;
  endpoint_url: string;
  is_active: number | boolean;
  consecutive_failures: number;
  status: 'Healthy' | 'Degraded' | 'Unhealthy' | 'CircuitBroken' | string;
  active_concurrency?: number;
  weight?: number;
  supported_protocols?: string[];
  created_at?: number;
}

export interface ModelBackendMapping {
  backend_id: string;
  backend_name?: string;
  endpoint_url?: string;
  weight_override?: number;
  is_active?: number | boolean;
}

export interface LogicalModel {
  id: string;
  name: string;
  description?: string;
  is_active: number | boolean;
  max_retries: number;
  retry_interval_ms: number;
  created_at?: number;
  backends?: ModelBackendMapping[];
}

export interface ApiKeyItem {
  id: string;
  name: string;
  key_prefix: string;
  full_key?: string; // 仅在创建成功时首次返回
  user_id?: string;
  username?: string;
  is_active: number | boolean;
  rate_limit_rpm: number;
  daily_quota_tokens: number;
  weekly_quota_tokens: number;
  daily_used_tokens: number;
  weekly_used_tokens: number;
  created_at: number;
}

export interface QuotaOverview {
  daily_quota_tokens: number;
  weekly_quota_tokens: number;
  daily_used_tokens: number;
  weekly_used_tokens: number;
  daily_reset_countdown_sec?: number;
  weekly_reset_countdown_sec?: number;
}
