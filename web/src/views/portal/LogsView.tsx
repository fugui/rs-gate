import React, { useState, useEffect, useCallback } from 'react';
import { portalApi } from '../../services/portalApi';
import { useToast } from '../../context/ToastContext';
import { HttpStatusBadge } from '../../components/common/Badge';
import { Modal } from '../../components/common/Modal';
import { AuditLogItem } from '../../types/audit';

export const LogsView: React.FC = () => {
  const { showToast } = useToast();
  const [logs, setLogs] = useState<AuditLogItem[]>([]);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(1);
  const [loading, setLoading] = useState(false);
  const [activeLog, setActiveLog] = useState<AuditLogItem | null>(null);

  const loadLogs = useCallback(async () => {
    setLoading(true);
    try {
      const res = await portalApi.getUserAudits({ page, page_size: 20 });
      const data = res.data;
      setLogs(data?.items || []);
      setTotal(data?.total || 0);
    } catch (err: any) {
      showToast(err.message || '加载调用流水失败', 'error');
    } finally {
      setLoading(false);
    }
  }, [page, showToast]);

  useEffect(() => {
    loadLogs();
  }, [loadLogs]);

  return (
    <div>
      <div className="card-header" style={{ marginBottom: '1.5rem' }}>
        <div>
          <h2 className="card-title">个人调用流水与下钻分析</h2>
          <p className="kpi-subtext" style={{ marginTop: '0.25rem' }}>
            仅展示当前账号名下的所有请求审计详情，包含端到端耗时、Token 统计与 Trace ID。
          </p>
        </div>
        <button
          type="button"
          className="btn btn-secondary btn-sm"
          onClick={loadLogs}
          disabled={loading}
        >
          {loading ? '刷新中...' : '刷新日志'}
        </button>
      </div>

      <div className="card">
        <div className="table-responsive">
          <table className="data-table">
            <thead>
              <tr>
                <th>请求时间</th>
                <th>Trace ID</th>
                <th>模型</th>
                <th>状态码</th>
                <th>总 Tokens</th>
                <th>首字耗时 (TTFT)</th>
                <th>总耗时</th>
                <th>重试</th>
                <th>操作</th>
              </tr>
            </thead>
            <tbody>
              {loading ? (
                <tr>
                  <td colSpan={9} style={{ textAlign: 'center', padding: '32px' }}>
                    加载中...
                  </td>
                </tr>
              ) : logs.length === 0 ? (
                <tr>
                  <td colSpan={9} style={{ textAlign: 'center', padding: '32px', color: 'var(--text-muted)' }}>
                    暂无调用记录。
                  </td>
                </tr>
              ) : (
                logs.map((item) => (
                  <tr key={item.id}>
                    <td>
                      {item.created_at
                        ? new Date(item.created_at > 1e11 ? item.created_at : item.created_at * 1000).toLocaleTimeString()
                        : '-'}
                    </td>
                    <td>
                      <code style={{ fontSize: '12px', color: 'var(--color-primary)' }}>
                        {item.trace_id.substring(0, 16)}...
                      </code>
                    </td>
                    <td style={{ fontWeight: 600 }}>{item.model}</td>
                    <td>
                      <HttpStatusBadge code={item.status_code} />
                    </td>
                    <td>{item.total_tokens ? item.total_tokens.toLocaleString() : '-'}</td>
                    <td>{item.ttft_ms !== undefined && item.ttft_ms !== null ? `${item.ttft_ms} ms` : '-'}</td>
                    <td>{item.latency_ms} ms</td>
                    <td>{item.retry_count || 0}</td>
                    <td>
                      <button
                        type="button"
                        className="btn btn-secondary btn-sm"
                        onClick={() => setActiveLog(item)}
                      >
                        详情
                      </button>
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>

        {total > 20 && (
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
                disabled={page * 20 >= total}
                onClick={() => setPage((p) => p + 1)}
              >
                下一页
              </button>
            </div>
          </div>
        )}
      </div>

      {/* 审计日志详情弹窗 */}
      <Modal
        isOpen={!!activeLog}
        onClose={() => setActiveLog(null)}
        title="调用明细与 Trace 下钻"
        footer={
          <button
            type="button"
            className="btn btn-primary"
            onClick={() => setActiveLog(null)}
          >
            关闭
          </button>
        }
      >
        {activeLog && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '12px', fontSize: '13px' }}>
            <div>
              <span style={{ color: 'var(--text-muted)' }}>Trace ID: </span>
              <code style={{ color: 'var(--color-primary)' }}>{activeLog.trace_id}</code>
            </div>
            <div>
              <span style={{ color: 'var(--text-muted)' }}>请求模型: </span>
              <strong>{activeLog.model}</strong>
            </div>
            <div>
              <span style={{ color: 'var(--text-muted)' }}>状态码: </span>
              <HttpStatusBadge code={activeLog.status_code} />
            </div>
            <div>
              <span style={{ color: 'var(--text-muted)' }}>Token 统计: </span>
              <span>
                输入 {activeLog.prompt_tokens || 0} / 输出 {activeLog.completion_tokens || 0} / 总计{' '}
                {activeLog.total_tokens || 0}
              </span>
            </div>
            <div>
              <span style={{ color: 'var(--text-muted)' }}>耗时概览: </span>
              <span>
                首字 TTFT: {activeLog.ttft_ms ?? '-'} ms | 端到端总时延: {activeLog.latency_ms} ms
              </span>
            </div>
            <div>
              <span style={{ color: 'var(--text-muted)' }}>内部重试次数: </span>
              <span>{activeLog.retry_count || 0} 次</span>
            </div>
            {activeLog.error_message && (
              <div style={{ marginTop: '8px' }}>
                <span style={{ color: 'var(--color-danger)', fontWeight: 600 }}>错误详情: </span>
                <pre
                  style={{
                    margin: '6px 0 0 0',
                    padding: '10px',
                    background: 'var(--bg-app)',
                    borderRadius: '6px',
                    color: 'var(--color-danger)',
                    whiteSpace: 'pre-wrap',
                  }}
                >
                  {activeLog.error_message}
                </pre>
              </div>
            )}
          </div>
        )}
      </Modal>
    </div>
  );
};
