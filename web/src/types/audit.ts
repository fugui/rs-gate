export interface AuditLogItem {
  id: string;
  trace_id: string;
  user_id?: string;
  username?: string;
  key_id?: string;
  key_name?: string;
  model: string;
  protocol?: string;
  status_code: number;
  prompt_tokens?: number;
  completion_tokens?: number;
  total_tokens?: number;
  ttft_ms?: number;
  latency_ms: number;
  retry_count?: number;
  error_message?: string;
  created_at: number;
}

export interface AdminStatsOverview {
  current_qps: number;
  active_connections: number;
  today_total_tokens: number;
  total_requests?: number;
  healthy_backends_count: number;
  total_backends_count: number;
}

export interface AuditFilterParams {
  page?: number;
  page_size?: number;
  username?: string;
  model?: string;
  status_code?: number | string;
  trace_id?: string;
  start_time?: number;
  end_time?: number;
}
