import { ApiResponse } from '../types/auth';

class HttpClient {
  private token: string | null = null;

  constructor() {
    this.token = localStorage.getItem('rsgate_token');
  }

  setToken(token: string | null) {
    this.token = token;
    if (token) {
      localStorage.setItem('rsgate_token', token);
    } else {
      localStorage.removeItem('rsgate_token');
    }
  }

  getToken(): string | null {
    return this.token;
  }

  async request<T = unknown>(endpoint: string, options: RequestInit = {}): Promise<ApiResponse<T>> {
    const url = endpoint.startsWith('/') ? endpoint : `/${endpoint}`;
    const headers: Record<string, string> = {
      'Content-Type': 'application/json',
      ...(options.headers as Record<string, string>),
    };

    if (this.token) {
      headers['Authorization'] = `Bearer ${this.token}`;
    }

    const res = await fetch(url, {
      ...options,
      headers,
    });

    if (res.status === 401) {
      this.setToken(null);
      window.dispatchEvent(new CustomEvent('unauthorized'));
      throw new Error('登录凭据已失效，请重新登录');
    }

    const json = await res.json();
    if (!res.ok || (json.code !== undefined && json.code !== 0)) {
      throw new Error(json.message || `请求失败 (${res.status})`);
    }

    return json as ApiResponse<T>;
  }

  async download(endpoint: string, filename: string): Promise<void> {
    const url = endpoint.startsWith('/') ? endpoint : `/${endpoint}`;
    const headers: Record<string, string> = {};
    if (this.token) {
      headers['Authorization'] = `Bearer ${this.token}`;
    }

    const res = await fetch(url, { headers });
    if (!res.ok) {
      throw new Error(`下载失败 (${res.status})`);
    }
    const blob = await res.blob();
    const downloadUrl = window.URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = downloadUrl;
    a.download = filename;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    window.URL.revokeObjectURL(downloadUrl);
  }
}

export const httpClient = new HttpClient();
