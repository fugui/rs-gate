import React, { useState, useEffect, useCallback } from 'react';
import { adminApi } from '../../services/adminApi';
import { useToast } from '../../context/ToastContext';
import { LogicalModel } from '../../types/gateway';

export const ModelsView: React.FC = () => {
  const { showToast } = useToast();
  const [models, setModels] = useState<LogicalModel[]>([]);
  const [loading, setLoading] = useState(false);

  const loadModels = useCallback(async () => {
    setLoading(true);
    try {
      const res = await adminApi.getModels();
      setModels(res.data || []);
    } catch (err: any) {
      showToast(err.message || '加载模型拓扑失败', 'error');
    } finally {
      setLoading(false);
    }
  }, [showToast]);

  useEffect(() => {
    loadModels();
  }, [loadModels]);

  return (
    <div>
      <div className="card-header" style={{ marginBottom: '1.5rem' }}>
        <div>
          <h2 className="card-title">逻辑模型集群与重试容错热配置</h2>
          <p className="kpi-subtext" style={{ marginTop: '0.25rem' }}>
            配置对外暴露的逻辑模型标识符，动态绑定后端物理实例池，支持权重调度与退避重试参数。
          </p>
        </div>
        <button
          type="button"
          className="btn btn-secondary btn-sm"
          onClick={loadModels}
          disabled={loading}
        >
          {loading ? '刷新中...' : '刷新模型'}
        </button>
      </div>

      <div className="card">
        <div className="table-responsive">
          <table className="data-table">
            <thead>
              <tr>
                <th>模型 ID</th>
                <th>展示名称</th>
                <th>重试次数 (Retries)</th>
                <th>退避间隔 (ms)</th>
                <th>挂载物理后端实例</th>
              </tr>
            </thead>
            <tbody>
              {loading ? (
                <tr>
                  <td colSpan={5} style={{ textAlign: 'center', padding: '32px' }}>
                    加载中...
                  </td>
                </tr>
              ) : models.length === 0 ? (
                <tr>
                  <td colSpan={5} style={{ textAlign: 'center', padding: '32px', color: 'var(--text-muted)' }}>
                    暂无配置逻辑模型。
                  </td>
                </tr>
              ) : (
                models.map((m) => {
                  const bes = m.backends || [];
                  return (
                    <tr key={m.id}>
                      <td style={{ fontWeight: 600 }}>{m.id}</td>
                      <td>{(m as any).display_name || m.name || m.id}</td>
                      <td>{m.max_retries ?? 2} 次</td>
                      <td>{m.retry_interval_ms ?? 500} ms</td>
                      <td>
                        {bes.length === 0 ? (
                          <span style={{ color: 'var(--text-muted)' }}>未挂载后端</span>
                        ) : (
                          <div style={{ display: 'flex', gap: '6px', flexWrap: 'wrap' }}>
                            {bes.map((b) => (
                              <span
                                key={b.backend_id}
                                className="badge"
                                style={{ background: 'var(--bg-app)', fontSize: '12px' }}
                              >
                                {b.backend_name || b.backend_id}
                                {b.weight_override ? ` (权重: ${b.weight_override})` : ''}
                              </span>
                            ))}
                          </div>
                        )}
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
