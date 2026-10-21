import React, { useState, useEffect, useCallback } from 'react';
import { adminApi } from '../../services/adminApi';
import { useToast } from '../../context/ToastContext';
import { AdminStatsOverview } from '../../types/audit';

export const DashboardView: React.FC = () => {
  const { showToast } = useToast();
  const [stats, setStats] = useState<AdminStatsOverview | null>(null);
  const [loading, setLoading] = useState(false);

  const fetchStats = useCallback(async (isSilent = false) => {
    if (!isSilent) setLoading(true);
    try {
      const res = await adminApi.getStatsOverview();
      setStats(res.data);
    } catch (err: any) {
      if (!isSilent) {
        showToast(err.message || '加载大盘统计失败', 'error');
      }
    } finally {
      if (!isSilent) setLoading(false);
    }
  }, [showToast]);

  useEffect(() => {
    fetchStats();

    // 5秒静默轮询，页面隐藏时自动挂起
    const timer = setInterval(() => {
      if (!document.hidden) {
        fetchStats(true);
      }
    }, 5000);

    return () => clearInterval(timer);
  }, [fetchStats]);

  return (
    <div>
      <div className="card-header" style={{ marginBottom: '1.5rem' }}>
        <div>
          <h2 className="card-title">集群监控大盘 (Ops Dashboard)</h2>
          <p className="kpi-subtext" style={{ marginTop: '0.25rem' }}>
            全局网关吞吐量、在途活跃长连接与物理节点健康度实时全景。
          </p>
        </div>
        <button
          type="button"
          className="btn btn-secondary btn-sm"
          onClick={() => fetchStats()}
          disabled={loading}
        >
          {loading ? '刷新中...' : '即刻刷新'}
        </button>
      </div>

      <div className="kpi-grid" style={{ marginBottom: '24px' }}>
        <div className="kpi-card">
          <div className="kpi-label">实时 QPS</div>
          <div className="kpi-value">
            {stats ? (stats.current_qps || 0).toFixed(1) : '-'}
          </div>
          <div className="kpi-subtext">每秒处理请求数 (1s 滑动窗口)</div>
        </div>

        <div className="kpi-card">
          <div className="kpi-label">在途长连接</div>
          <div className="kpi-value">{stats ? stats.active_connections : '-'}</div>
          <div className="kpi-subtext">当前正在转发的并发请求数</div>
        </div>

        <div className="kpi-card">
          <div className="kpi-label">今日 Token 吞吐</div>
          <div className="kpi-value">
            {stats ? (stats.today_total_tokens || 0).toLocaleString() : '-'}
          </div>
          <div className="kpi-subtext">全集群今日累计消耗 Token</div>
        </div>

        <div className="kpi-card">
          <div className="kpi-label">健康后端节点</div>
          <div className="kpi-value">
            {stats
              ? `${stats.healthy_backends_count || 0} / ${stats.total_backends_count || 0}`
              : '-'}
          </div>
          <div className="kpi-subtext">活跃健康探测正常率</div>
        </div>
      </div>

      <div className="card" style={{ padding: '24px' }}>
        <h3 style={{ fontSize: '15px', fontWeight: 600, marginBottom: '12px' }}>
          网关运行机制与调度说明
        </h3>
        <p style={{ fontSize: '13px', color: 'var(--text-secondary)', lineHeight: 1.7, margin: 0 }}>
          rs-gate 采用加权最小活跃连接（Weighted Least-Connections）与连续失败熔断机制。
          当物理节点连续多次请求超时或返回 5xx 时，网关会自动将该节点降级或熔断隔离，
          并将后续流量平滑自动故障转移（Failover）至同集群可用备用节点，保证业务零感知。
        </p>
      </div>
    </div>
  );
};
