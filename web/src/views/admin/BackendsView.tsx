import React, { useState, useEffect, useCallback } from 'react';
import { adminApi } from '../../services/adminApi';
import { useToast } from '../../context/ToastContext';
import { BackendInstance } from '../../types/gateway';

export const BackendsView: React.FC = () => {
  const { showToast } = useToast();
  const [backends, setBackends] = useState<BackendInstance[]>([]);
  const [loading, setLoading] = useState(false);
  const [probingId, setProbingId] = useState<string | null>(null);

  const loadBackends = useCallback(async () => {
    setLoading(true);
    try {
      const res = await adminApi.getBackends();
      setBackends(res.data || []);
    } catch (err: any) {
      showToast(err.message || '加载物理后端失败', 'error');
    } finally {
      setLoading(false);
    }
  }, [showToast]);

  useEffect(() => {
    loadBackends();
  }, [loadBackends]);

  const handleProbe = async (id: string) => {
    setProbingId(id);
    try {
      await adminApi.probeBackend(id);
      showToast(`后端 ${id} 探活完成，状态已同步`, 'success');
      loadBackends();
    } catch (err: any) {
      showToast(`探活失败: ${err.message}`, 'error');
    } finally {
      setProbingId(null);
    }
  };

  const protocols = ['chat', 'responses', 'completions', 'embeddings'];

  return (
    <div>
      <div className="card-header" style={{ marginBottom: '1.5rem' }}>
        <div>
          <h2 className="card-title">物理后端实例与协议探针</h2>
          <p className="kpi-subtext" style={{ marginTop: '0.25rem' }}>
            管理 vLLM / SGLang / TEI 及第三方 API 物理服务实例，实时探测各端点协议能力矩阵。
          </p>
        </div>
        <button
          type="button"
          className="btn btn-secondary btn-sm"
          onClick={loadBackends}
          disabled={loading}
        >
          {loading ? '刷新中...' : '刷新节点'}
        </button>
      </div>

      <div className="card">
        <div className="table-responsive">
          <table className="data-table">
            <thead>
              <tr>
                <th>ID / 节点名</th>
                <th>基础 URL</th>
                <th>权重</th>
                <th>支持协议徽章</th>
                <th>健康状态</th>
                <th>操作</th>
              </tr>
            </thead>
            <tbody>
              {loading ? (
                <tr>
                  <td colSpan={6} style={{ textAlign: 'center', padding: '32px' }}>
                    加载中...
                  </td>
                </tr>
              ) : backends.length === 0 ? (
                <tr>
                  <td colSpan={6} style={{ textAlign: 'center', padding: '32px', color: 'var(--text-muted)' }}>
                    暂无配置物理后端节点。
                  </td>
                </tr>
              ) : (
                backends.map((be) => {
                  const caps = (be as any).capabilities || be.supported_protocols || ['chat', 'responses'];
                  const statusStr = be.status || (be as any).health_status || 'Healthy';
                  const isHealthy = statusStr.toLowerCase().includes('health');

                  return (
                    <tr key={be.id}>
                      <td style={{ fontWeight: 600 }}>
                        {be.name} <span style={{ color: 'var(--text-muted)', fontSize: '12px' }}>({be.id})</span>
                      </td>
                      <td>
                        <code>{be.endpoint_url || (be as any).base_url}</code>
                      </td>
                      <td>{be.weight ?? 10}</td>
                      <td>
                        <div style={{ display: 'flex', gap: '4px', flexWrap: 'wrap' }}>
                          {protocols.map((p) => {
                            const active = caps.includes(p);
                            return (
                              <span
                                key={p}
                                className={`badge ${active ? 'badge-protocol-active' : 'badge-protocol-inactive'}`}
                              >
                                {active ? '🟢' : '⚪'} {p}
                              </span>
                            );
                          })}
                        </div>
                      </td>
                      <td>
                        <span className={`badge ${isHealthy ? 'badge-status-healthy' : 'badge-status-broken'}`}>
                          {statusStr}
                        </span>
                      </td>
                      <td>
                        <button
                          type="button"
                          className="btn btn-secondary btn-sm"
                          disabled={probingId === be.id}
                          onClick={() => handleProbe(be.id)}
                        >
                          {probingId === be.id ? '探测中...' : '即时探活'}
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
