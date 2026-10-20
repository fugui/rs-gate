// Unified REST API Client for rs-gate

class ApiClient {
  constructor() {
    this.token = localStorage.getItem('rsgate_token') || null;
    this.user = null;
    try {
      const storedUser = localStorage.getItem('rsgate_user');
      if (storedUser) {
        this.user = JSON.parse(storedUser);
      }
    } catch {
      this.user = null;
    }
  }

  setSession(token, user) {
    this.token = token;
    this.user = user;
    if (token) {
      localStorage.setItem('rsgate_token', token);
    } else {
      localStorage.removeItem('rsgate_token');
    }
    if (user) {
      localStorage.setItem('rsgate_user', JSON.stringify(user));
    } else {
      localStorage.removeItem('rsgate_user');
    }
  }

  clearSession() {
    this.token = null;
    this.user = null;
    localStorage.removeItem('rsgate_token');
    localStorage.removeItem('rsgate_user');
  }

  async request(endpoint, options = {}) {
    const url = endpoint.startsWith('/') ? endpoint : `/${endpoint}`;
    const headers = {
      'Content-Type': 'application/json',
      ...options.headers,
    };

    if (this.token) {
      headers['Authorization'] = `Bearer ${this.token}`;
    }

    try {
      const res = await fetch(url, {
        ...options,
        headers,
      });

      if (res.status === 401) {
        this.clearSession();
        window.dispatchEvent(new CustomEvent('unauthorized'));
        throw new Error('未登录或登录凭据已过期');
      }

      // Check if it is a streaming or blob response (e.g. CSV)
      const contentType = res.headers.get('content-type') || '';
      if (contentType.includes('text/csv')) {
        return res;
      }

      const json = await res.json();
      if (!res.ok || (json.code !== undefined && json.code !== 0)) {
        throw new Error(json.message || `Request failed with status ${res.status}`);
      }

      return json;
    } catch (err) {
      throw err;
    }
  }

  // Auth APIs
  login(username, password) {
    return this.request('/api/auth/login', {
      method: 'POST',
      body: JSON.stringify({ username, password }),
    });
  }

  logout() {
    return this.request('/api/auth/logout', { method: 'POST' });
  }

  getMe() {
    return this.request('/api/auth/me', { method: 'GET' });
  }

  // User Self-Service APIs
  getUserModels() {
    return this.request('/api/user/models', { method: 'GET' });
  }

  getUserKeys() {
    return this.request('/api/user/keys', { method: 'GET' });
  }

  createUserKey(data) {
    return this.request('/api/user/keys', {
      method: 'POST',
      body: JSON.stringify(data),
    });
  }

  updateUserKey(id, data) {
    return this.request(`/api/user/keys/${id}`, {
      method: 'PUT',
      body: JSON.stringify(data),
    });
  }

  deleteUserKey(id) {
    return this.request(`/api/user/keys/${id}`, { method: 'DELETE' });
  }

  getUserAudits(params = {}) {
    const query = new URLSearchParams(params).toString();
    return this.request(`/api/user/audits${query ? '?' + query : ''}`, { method: 'GET' });
  }

  // Admin APIs
  getAdminStatsOverview() {
    return this.request('/api/admin/stats/overview', { method: 'GET' });
  }

  getAdminStatsTrends(interval = '24h') {
    return this.request(`/api/admin/stats/trends?interval=${interval}`, { method: 'GET' });
  }

  getAdminUsers(params = {}) {
    const query = new URLSearchParams(params).toString();
    return this.request(`/api/admin/users${query ? '?' + query : ''}`, { method: 'GET' });
  }

  createAdminUser(data) {
    return this.request('/api/admin/users', {
      method: 'POST',
      body: JSON.stringify(data),
    });
  }

  updateAdminUser(id, data) {
    return this.request(`/api/admin/users/${id}`, {
      method: 'PUT',
      body: JSON.stringify(data),
    });
  }

  deleteAdminUser(id) {
    return this.request(`/api/admin/users/${id}`, { method: 'DELETE' });
  }

  getAdminKeys(params = {}) {
    const query = new URLSearchParams(params).toString();
    return this.request(`/api/admin/keys${query ? '?' + query : ''}`, { method: 'GET' });
  }

  createAdminKey(data) {
    return this.request('/api/admin/keys', {
      method: 'POST',
      body: JSON.stringify(data),
    });
  }

  updateAdminKey(id, data) {
    return this.request(`/api/admin/keys/${id}`, {
      method: 'PUT',
      body: JSON.stringify(data),
    });
  }

  deleteAdminKey(id) {
    return this.request(`/api/admin/keys/${id}`, { method: 'DELETE' });
  }

  resetAdminKeyQuota(id, periodType = 'daily') {
    return this.request(`/api/admin/keys/${id}/reset-quota`, {
      method: 'POST',
      body: JSON.stringify({ period_type: periodType }),
    });
  }

  getAdminBackends() {
    return this.request('/api/admin/backends', { method: 'GET' });
  }

  createAdminBackend(data) {
    return this.request('/api/admin/backends', {
      method: 'POST',
      body: JSON.stringify(data),
    });
  }

  updateAdminBackend(id, data) {
    return this.request(`/api/admin/backends/${id}`, {
      method: 'PUT',
      body: JSON.stringify(data),
    });
  }

  deleteAdminBackend(id) {
    return this.request(`/api/admin/backends/${id}`, { method: 'DELETE' });
  }

  probeAdminBackend(id) {
    return this.request(`/api/admin/backends/${id}/probe`, { method: 'POST' });
  }

  getAdminModels() {
    return this.request('/api/admin/models', { method: 'GET' });
  }

  createAdminModel(data) {
    return this.request('/api/admin/models', {
      method: 'POST',
      body: JSON.stringify(data),
    });
  }

  updateAdminModel(id, data) {
    return this.request(`/api/admin/models/${id}`, {
      method: 'PUT',
      body: JSON.stringify(data),
    });
  }

  bindAdminModelBackend(modelId, data) {
    return this.request(`/api/admin/models/${modelId}/backends`, {
      method: 'POST',
      body: JSON.stringify(data),
    });
  }

  unbindAdminModelBackend(modelId, backendId) {
    return this.request(`/api/admin/models/${modelId}/backends/${backendId}`, {
      method: 'DELETE',
    });
  }

  getAdminAudits(params = {}) {
    const query = new URLSearchParams(params).toString();
    return this.request(`/api/admin/audits${query ? '?' + query : ''}`, { method: 'GET' });
  }

  async exportAdminAuditsCsv(params = {}) {
    const query = new URLSearchParams(params).toString();
    const res = await this.request(`/api/admin/audits/export?${query}`, { method: 'GET' });
    const blob = await res.blob();
    const downloadUrl = window.URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = downloadUrl;
    a.download = `rs-gate-audits-${Date.now()}.csv`;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    window.URL.revokeObjectURL(downloadUrl);
  }
}

export const api = new ApiClient();
