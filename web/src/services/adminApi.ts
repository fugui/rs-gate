import { httpClient } from './client';
import { ApiResponse, PaginatedData, User } from '../types/auth';
import { ApiKeyItem, BackendInstance, LogicalModel } from '../types/gateway';
import { AdminStatsOverview, AuditFilterParams, AuditLogItem } from '../types/audit';

export const adminApi = {
  // Stats
  getStatsOverview(): Promise<ApiResponse<AdminStatsOverview>> {
    return httpClient.request<AdminStatsOverview>('/api/admin/stats/overview', {
      method: 'GET',
    });
  },

  // Users
  getUsers(params: { page?: number; page_size?: number } = {}): Promise<ApiResponse<PaginatedData<User>>> {
    const query = new URLSearchParams();
    if (params.page) query.set('page', String(params.page));
    if (params.page_size) query.set('page_size', String(params.page_size));
    const qs = query.toString();
    return httpClient.request<PaginatedData<User>>(`/api/admin/users${qs ? '?' + qs : ''}`, {
      method: 'GET',
    });
  },

  createUser(data: {
    username: string;
    password: string;
    display_name: string;
    role: string;
    email?: string;
  }): Promise<ApiResponse<User>> {
    return httpClient.request<User>('/api/admin/users', {
      method: 'POST',
      body: JSON.stringify(data),
    });
  },

  updateUser(id: string, data: { display_name?: string; role?: string; password?: string }): Promise<ApiResponse<User>> {
    return httpClient.request<User>(`/api/admin/users/${id}`, {
      method: 'PUT',
      body: JSON.stringify(data),
    });
  },

  deleteUser(id: string): Promise<ApiResponse<null>> {
    return httpClient.request<null>(`/api/admin/users/${id}`, {
      method: 'DELETE',
    });
  },

  // Keys
  getKeys(params: { page?: number; page_size?: number; username?: string } = {}): Promise<ApiResponse<PaginatedData<ApiKeyItem>>> {
    const query = new URLSearchParams();
    if (params.page) query.set('page', String(params.page));
    if (params.page_size) query.set('page_size', String(params.page_size));
    if (params.username) query.set('username', params.username);
    const qs = query.toString();
    return httpClient.request<PaginatedData<ApiKeyItem>>(`/api/admin/keys${qs ? '?' + qs : ''}`, {
      method: 'GET',
    });
  },

  createKey(data: {
    user_id: string;
    name: string;
    rate_limit_rpm?: number;
    daily_quota_tokens?: number;
    weekly_quota_tokens?: number;
  }): Promise<ApiResponse<ApiKeyItem>> {
    return httpClient.request<ApiKeyItem>('/api/admin/keys', {
      method: 'POST',
      body: JSON.stringify(data),
    });
  },

  updateKey(id: string, data: { is_active?: number | boolean; rate_limit_rpm?: number; daily_quota_tokens?: number; weekly_quota_tokens?: number }): Promise<ApiResponse<ApiKeyItem>> {
    return httpClient.request<ApiKeyItem>(`/api/admin/keys/${id}`, {
      method: 'PUT',
      body: JSON.stringify(data),
    });
  },

  deleteKey(id: string): Promise<ApiResponse<null>> {
    return httpClient.request<null>(`/api/admin/keys/${id}`, {
      method: 'DELETE',
    });
  },

  resetKeyQuota(id: string, periodType: 'daily' | 'weekly'): Promise<ApiResponse<null>> {
    return httpClient.request<null>(`/api/admin/keys/${id}/reset-quota`, {
      method: 'POST',
      body: JSON.stringify({ period_type: periodType }),
    });
  },

  // Backends
  getBackends(): Promise<ApiResponse<BackendInstance[]>> {
    return httpClient.request<BackendInstance[]>('/api/admin/backends', {
      method: 'GET',
    });
  },

  createBackend(data: {
    name: string;
    endpoint_url: string;
    weight?: number;
    supported_protocols?: string[];
  }): Promise<ApiResponse<BackendInstance>> {
    return httpClient.request<BackendInstance>('/api/admin/backends', {
      method: 'POST',
      body: JSON.stringify(data),
    });
  },

  updateBackend(id: string, data: {
    name?: string;
    endpoint_url?: string;
    weight?: number;
    is_active?: number | boolean;
  }): Promise<ApiResponse<BackendInstance>> {
    return httpClient.request<BackendInstance>(`/api/admin/backends/${id}`, {
      method: 'PUT',
      body: JSON.stringify(data),
    });
  },

  deleteBackend(id: string): Promise<ApiResponse<null>> {
    return httpClient.request<null>(`/api/admin/backends/${id}`, {
      method: 'DELETE',
    });
  },

  probeBackend(id: string): Promise<ApiResponse<{ status: string; supported_protocols?: string[] }>> {
    return httpClient.request<{ status: string; supported_protocols?: string[] }>(`/api/admin/backends/${id}/probe`, {
      method: 'POST',
    });
  },

  // Models
  getModels(): Promise<ApiResponse<LogicalModel[]>> {
    return httpClient.request<LogicalModel[]>('/api/admin/models', {
      method: 'GET',
    });
  },

  createModel(data: {
    name: string;
    description?: string;
    max_retries?: number;
    retry_interval_ms?: number;
  }): Promise<ApiResponse<LogicalModel>> {
    return httpClient.request<LogicalModel>('/api/admin/models', {
      method: 'POST',
      body: JSON.stringify(data),
    });
  },

  updateModel(id: string, data: {
    name?: string;
    description?: string;
    max_retries?: number;
    retry_interval_ms?: number;
    is_active?: number | boolean;
  }): Promise<ApiResponse<LogicalModel>> {
    return httpClient.request<LogicalModel>(`/api/admin/models/${id}`, {
      method: 'PUT',
      body: JSON.stringify(data),
    });
  },

  bindModelBackend(modelId: string, data: { backend_id: string; weight_override?: number }): Promise<ApiResponse<null>> {
    return httpClient.request<null>(`/api/admin/models/${modelId}/backends`, {
      method: 'POST',
      body: JSON.stringify(data),
    });
  },

  unbindModelBackend(modelId: string, backendId: string): Promise<ApiResponse<null>> {
    return httpClient.request<null>(`/api/admin/models/${modelId}/backends/${backendId}`, {
      method: 'DELETE',
    });
  },

  // Audits
  getAudits(params: AuditFilterParams = {}): Promise<ApiResponse<PaginatedData<AuditLogItem>>> {
    const query = new URLSearchParams();
    if (params.page) query.set('page', String(params.page));
    if (params.page_size) query.set('page_size', String(params.page_size));
    if (params.username) query.set('username', params.username);
    if (params.model) query.set('model', params.model);
    if (params.status_code) query.set('status_code', String(params.status_code));
    if (params.trace_id) query.set('trace_id', params.trace_id);
    const qs = query.toString();
    return httpClient.request<PaginatedData<AuditLogItem>>(`/api/admin/audits${qs ? '?' + qs : ''}`, {
      method: 'GET',
    });
  },

  async exportAuditsCsv(params: AuditFilterParams = {}): Promise<void> {
    const query = new URLSearchParams();
    if (params.username) query.set('username', params.username);
    if (params.model) query.set('model', params.model);
    if (params.status_code) query.set('status_code', String(params.status_code));
    if (params.trace_id) query.set('trace_id', params.trace_id);
    const qs = query.toString();
    return httpClient.download(`/api/admin/audits/export${qs ? '?' + qs : ''}`, `rs-gate-audits-${Date.now()}.csv`);
  },
};
