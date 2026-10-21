import React, { useState, useEffect, useCallback } from 'react';
import { adminApi } from '../../services/adminApi';
import { useToast } from '../../context/ToastContext';
import { User } from '../../types/auth';

export const UsersView: React.FC = () => {
  const { showToast } = useToast();
  const [users, setUsers] = useState<User[]>([]);
  const [loading, setLoading] = useState(false);

  const loadUsers = useCallback(async () => {
    setLoading(true);
    try {
      const res = await adminApi.getUsers();
      setUsers(res.data?.items || []);
    } catch (err: any) {
      showToast(err.message || '加载用户列表失败', 'error');
    } finally {
      setLoading(false);
    }
  }, [showToast]);

  useEffect(() => {
    loadUsers();
  }, [loadUsers]);

  return (
    <div>
      <div className="card-header" style={{ marginBottom: '1.5rem' }}>
        <div>
          <h2 className="card-title">用户账户与权限管控</h2>
          <p className="kpi-subtext" style={{ marginTop: '0.25rem' }}>
            管理内部开发者账号、RBAC 角色权限分配（superadmin / admin / user）与生命周期。
          </p>
        </div>
        <button
          type="button"
          className="btn btn-secondary btn-sm"
          onClick={loadUsers}
          disabled={loading}
        >
          {loading ? '刷新中...' : '刷新用户'}
        </button>
      </div>

      <div className="card">
        <div className="table-responsive">
          <table className="data-table">
            <thead>
              <tr>
                <th>用户名</th>
                <th>显示姓名</th>
                <th>角色</th>
                <th>邮箱</th>
                <th>状态</th>
                <th>创建时间</th>
              </tr>
            </thead>
            <tbody>
              {loading ? (
                <tr>
                  <td colSpan={6} style={{ textAlign: 'center', padding: '32px' }}>
                    加载中...
                  </td>
                </tr>
              ) : users.length === 0 ? (
                <tr>
                  <td colSpan={6} style={{ textAlign: 'center', padding: '32px', color: 'var(--text-muted)' }}>
                    暂无用户记录。
                  </td>
                </tr>
              ) : (
                users.map((u: any) => {
                  const isActive = u.is_active === 1 || u.is_active === true || u.is_active === undefined;
                  const isSuper = u.role === 'superadmin';

                  return (
                    <tr key={u.id}>
                      <td style={{ fontWeight: 600 }}>{u.username}</td>
                      <td>{u.display_name || '-'}</td>
                      <td>
                        <span
                          className="badge"
                          style={{
                            background: isSuper ? 'hsla(260, 60%, 55%, 0.2)' : 'var(--bg-app)',
                            color: isSuper ? 'hsl(260, 70%, 75%)' : 'inherit',
                          }}
                        >
                          {u.role}
                        </span>
                      </td>
                      <td>{u.email || '-'}</td>
                      <td>
                        <span className={`badge ${isActive ? 'badge-status-healthy' : 'badge-status-broken'}`}>
                          {isActive ? '● 正常' : '● 禁用'}
                        </span>
                      </td>
                      <td>
                        {u.created_at
                          ? new Date(u.created_at > 1e11 ? u.created_at : u.created_at * 1000).toLocaleString()
                          : '-'}
                      </td>
                    </tr>
                  );
                })
              )}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
};
