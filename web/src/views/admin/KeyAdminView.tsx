import React, { useState, useEffect, useCallback } from 'react';
import { adminApi } from '../../services/adminApi';
import { useToast } from '../../context/ToastContext';

export const KeyAdminView: React.FC = () => {
  const { showToast } = useToast();
  const [keys, setKeys] = useState<any[]>([]);
  const [loading, setLoading] = useState(false);
  const [resettingId, setResettingId] = useState<string | null>(null);

  const loadKeys = useCallback(async () => {
    setLoading(true);
    try {
      const res = await adminApi.getKeys({ page_size: 50 });
      setKeys(res.data?.items || []);
    } catch (err: any) {
      showToast(err.message || '加载 Key 列表失败', 'error');
    } finally {
      setLoading(false);
    }
  }, [showToast]);

  useEffect(() => {
    loadKeys();
  }, [loadKeys]);

  const handleResetQuota = async (id: string) => {
    if (!window.confirm(`确定一键重置 Key ${id} 的今日已用配额吗？重置后将立即解除拦截。`)) return;

    setResettingId(id);
    try {
      await adminApi.resetKeyQuota(id, 'daily');
      showToast(`Key ${id} 今日已用配额已清零`, 'success');
      loadKeys();
    } catch (err: any) {
      showToast(`重置失败: ${err.message}`, 'error');
    } finally {
      setResettingId(null);
    }
  };

  return (
    <div>
      <div className="card-header" style={{ marginBottom: '1.5rem' }}>
        <div>
          <h2 className="card-title">全局虚拟 Key 监控与一键配额重置</h2>
          <p className="kpi-subtext" style={{ marginTop: '0.25rem' }}>
            全局跨用户监控企业内所有 API Key，支持突发压测或部门用量耗尽时的一键配额即时纠偏。
          </p>
        </div>
        <button
          type="button"
          className="btn btn-secondary btn-sm"
          onClick={loadKeys}
          disabled={loading}
        >
          {loading ? '刷新中...' : '刷新列表'}
        </button>
      </div>

      <div className="card">
        <div className="table-responsive">
          <table className="data-table">
            <thead>
              <tr>
                <th>Key ID</th>
                <th>所属用户</th>
                <th>名称</th>
                <th>前缀</th>
                <th>日配额已用</th>
                <th>周配额已用</th>
                <th>操作</th>
              </tr>
            </thead>
            <tbody>
              {loading ? (
                <tr>
                  <td colSpan={7} style={{ textAlign: 'center', padding: '32px' }}>
                    加载中...
                  </td>
                </tr>
              ) : keys.length === 0 ? (
                <tr>
                  <td colSpan={7} style={{ textAlign: 'center', padding: '32px', color: 'var(--text-muted)' }}>
                    暂无虚拟 Key 记录。
                  </td>
                </tr>
              ) : (
                keys.map((k) => {
                  const dUsed = k.quota_status ? k.quota_status.daily_used : k.daily_used_tokens || 0;
                  const dLimit = k.daily_quota || k.daily_quota_tokens || 0;
                  const wUsed = k.quota_status ? k.quota_status.weekly_used : k.weekly_used_tokens || 0;
                  const wLimit = k.weekly_quota || k.weekly_quota_tokens || 0;

                  return (
                    <tr key={k.id}>
                      <td>
                        <code>{k.id}</code>
                      </td>
                      <td style={{ fontWeight: 600 }}>{k.user_username || k.username || k.user_id}</td>
                      <td>{k.name}</td>
                      <td>
                        <code className="badge" style={{ background: 'var(--bg-app)' }}>
                          {k.key_prefix}
                        </code>
                      </td>
                      <td>
                        {dLimit > 0
                          ? `${dUsed.toLocaleString()} / ${dLimit.toLocaleString()}`
                          : `${dUsed.toLocaleString()} / 无限制`}
                      </td>
                      <td>
                        {wLimit > 0
                          ? `${wUsed.toLocaleString()} / ${wLimit.toLocaleString()}`
                          : `${wUsed.toLocaleString()} / 无限制`}
                      </td>
                      <td>
                        <button
                          type="button"
                          className="btn btn-secondary btn-sm"
                          disabled={resettingId === k.id}
                          onClick={() => handleResetQuota(k.id)}
                        >
                          {resettingId === k.id ? '重置中...' : '重置今日配额'}
                        </button>
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
