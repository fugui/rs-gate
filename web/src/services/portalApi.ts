import { httpClient } from './client';
import { ApiResponse, PaginatedData } from '../types/auth';
import { ApiKeyItem, LogicalModel } from '../types/gateway';
import { AuditFilterParams, AuditLogItem } from '../types/audit';

export const portalApi = {
  getUserModels(): Promise<ApiResponse<LogicalModel[]>> {
    return httpClient.request<LogicalModel[]>('/api/user/models', {
      method: 'GET',
    });
  },

  getUserKeys(): Promise<ApiResponse<ApiKeyItem[]>> {
    return httpClient.request<ApiKeyItem[]>('/api/user/keys', {
      method: 'GET',
    });
  },

  createUserKey(data: {
    name: string;
    rate_limit_rpm?: number;
    daily_quota_tokens?: number;
    weekly_quota_tokens?: number;
  }): Promise<ApiResponse<ApiKeyItem>> {
    return httpClient.request<ApiKeyItem>('/api/user/keys', {
      method: 'POST',
      body: JSON.stringify(data),
    });
  },

  updateUserKey(id: string, data: { is_active?: number | boolean; name?: string }): Promise<ApiResponse<ApiKeyItem>> {
    return httpClient.request<ApiKeyItem>(`/api/user/keys/${id}`, {
      method: 'PUT',
      body: JSON.stringify(data),
    });
  },

  deleteUserKey(id: string): Promise<ApiResponse<null>> {
    return httpClient.request<null>(`/api/user/keys/${id}`, {
      method: 'DELETE',
    });
  },

  getUserAudits(params: AuditFilterParams = {}): Promise<ApiResponse<PaginatedData<AuditLogItem>>> {
    const query = new URLSearchParams();
    if (params.page) query.set('page', String(params.page));
    if (params.page_size) query.set('page_size', String(params.page_size));
    if (params.model) query.set('model', params.model);
    if (params.status_code) query.set('status_code', String(params.status_code));
    if (params.trace_id) query.set('trace_id', params.trace_id);
    const qs = query.toString();
    return httpClient.request<PaginatedData<AuditLogItem>>(`/api/user/audits${qs ? '?' + qs : ''}`, {
      method: 'GET',
    });
  },
};
