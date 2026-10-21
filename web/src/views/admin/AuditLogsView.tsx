import React, { useState, useEffect, useCallback } from 'react';
import { adminApi } from '../../services/adminApi';
import { useToast } from '../../context/ToastContext';
import { HttpStatusBadge } from '../../components/common/Badge';
import { AuditLogItem } from '../../types/audit';

export const AuditLogsView: React.FC = () => {
  const { showToast } = useToast();
  const [logs, setLogs] = useState<AuditLogItem[]>([]);
  const [loading, setLoading] = useState(false);
  const [exporting, setExporting] = useState(false);
  const [page, setPage] = useState(1);
  const [total, setTotal] = useState(0);

  const loadAudits = useCallback(async () => {
    setLoading(true);
    try {
      const res = await adminApi.getAudits({ page, page_size: 50 });
      setLogs(res.data?.items || []);
      setTotal(res.data?.total || 0);
    } catch (err: any) {
      showToast(err.message || '加载全量审计日志失败', 'error');
    } finally {
      setLoading(false);
    }
  }, [page, showToast]);

  useEffect(() => {
    loadAudits();
  }, [loadAudits]);

  const handleExport = async () => {
    setExporting(true);
    try {
      await adminApi.exportAuditsCsv({ start_time: 0 });
      showToast('审计数据已成功导出为 CSV', 'success');
    } catch (err: any) {
      showToast(`导出失败: ${err.message}`, 'error');
    } finally {
      setExporting(false);
    }
  };

  return (
    <div>
      <div className="card-header" style={{ marginBottom: '1.5rem' }}>
        <div>
          <h2 className="card-title">全局调用审计与流式 CSV 导出</h2>
          <p className="kpi-subtext" style={{ marginTop: '0.25rem' }}>
            端到端透视集群全量大模型调用请求流水，支持重试故障转移链路下钻与对账导出。
          </p>
        </div>
        <div style={{ display: 'flex', gap: '10px' }}>
          <button
            type="button"
            className="btn btn-secondary btn-sm"
            onClick={loadAudits}
            disabled={loading}
          >
            {loading ? '刷新中...' : '刷新'}
          </button>
          <button
            type="button"
            className="btn btn-primary btn-sm"
            onClick={handleExport}
            disabled={exporting}
          >
            {exporting ? '导出中...' : '📥 导出为 CSV'}
          </button>
        </div>
      </div>

      <div className="card">
        <div className="table-responsive">
          <table className="data-table">
            <thead>
              <tr>
                <th>Trace ID</th>
                <th>模型</th>
                <th>后端节点</th>
                <th>重试次数</th>
                <th>状态</th>
                <th>耗时</th>
                <th>Token 消耗</th>
                <th>时间</th>
              </tr>
            </thead>
            <tbody>
              {loading ? (
                <tr>
                  <td colSpan={8} style={{ textAlign: 'center', padding: '32px' }}>
                    加载中...
                  </td>
                </tr>
              ) : logs.length === 0 ? (
                <tr>
                  <td colSpan={8} style={{ textAlign: 'center', padding: '32px', color: 'var(--text-muted)' }}>
                    暂无审计流水记录。
                  </td>
                </tr>
              ) : (
                logs.map((a: any) => {
                  const status = a.http_status || a.status_code || 200;
                  const dur = a.request_duration_ms || a.latency_ms || 0;
                  const tokens = a.total_tokens !== undefined ? a.total_tokens : '-';
                  const trace = (a.trace_id || '').slice(0, 8);

                  return (
                    <tr key={a.id}>
                      <td>
                        <code style={{ color: 'var(--color-primary)' }}>{trace}</code>
                      </td>
                      <td style={{ fontWeight: 600 }}>{a.model}</td>
                      <td>{a.backend_id || '-'}</td>
                      <td>{a.retry_count || 0} 次</td>
                      <td>
                        <HttpStatusBadge code={status} />
                      </td>
                      <td>{dur} ms</td>
                      <td>{tokens}</td>
                      <td>
                        {a.created_at
                          ? new Date(a.created_at > 1e11 ? a.created_at : a.created_at * 1000).toLocaleString()
                          : '-'}
                      </td>
                    </tr>
                  );
                })
              )}
            </tbody>
          </table>
        </div>

        {total > 50 && (
          <div
            style={{
              padding: '16px 20px',
              display: 'flex',
              justifyContent: 'space-between',
              alignItems: 'center',
              borderTop: '1px solid var(--border-subtle)',
            }}
          >
            <span style={{ fontSize: '13px', color: 'var(--text-muted)' }}>
              共 {total} 条记录，当前第 {page} 页
            </span>
            <div style={{ display: 'flex', gap: '8px' }}>
              <button
                type="button"
                className="btn btn-secondary btn-sm"
                disabled={page <= 1}
                onClick={() => setPage((p) => p - 1)}
              >
                上一页
              </button>
              <button
                type="button"
                className="btn btn-secondary btn-sm"
                disabled={page * 50 >= total}
                onClick={() => setPage((p) => p + 1)}
              >
                下一页
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
};
