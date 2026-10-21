import React, { useState, useEffect, useCallback } from 'react';
import { portalApi } from '../../services/portalApi';
import { useToast } from '../../context/ToastContext';
import { QuotaBar } from '../../components/common/QuotaBar';

export const UsageView: React.FC = () => {
  const { showToast } = useToast();
  const [loading, setLoading] = useState(false);
  const [dailyUsed, setDailyUsed] = useState(0);
  const [dailyLimit, setDailyLimit] = useState(0);
  const [weeklyUsed, setWeeklyUsed] = useState(0);
  const [weeklyLimit, setWeeklyLimit] = useState(0);

  const loadUsage = useCallback(async () => {
    setLoading(true);
    try {
      const res = await portalApi.getUserKeys();
      const keys = res.data || [];

      let dUsed = 0;
      let dLimit = 0;
      let wUsed = 0;
      let wLimit = 0;

      keys.forEach((k: any) => {
        dUsed += k.quota_status ? k.quota_status.daily_used : k.daily_used_tokens || 0;
        dLimit += k.daily_quota || k.daily_quota_tokens || 0;
        wUsed += k.quota_status ? k.quota_status.weekly_used : k.weekly_used_tokens || 0;
        wLimit += k.weekly_quota || k.weekly_quota_tokens || 0;
      });

      setDailyUsed(dUsed);
      setDailyLimit(dLimit);
      setWeeklyUsed(wUsed);
      setWeeklyLimit(wLimit);
    } catch (err: any) {
      showToast(err.message || '加载配额用量失败', 'error');
    } finally {
      setLoading(false);
    }
  }, [showToast]);

  useEffect(() => {
    loadUsage();
  }, [loadUsage]);

  return (
    <div>
      <div className="card-header" style={{ marginBottom: '1.5rem' }}>
        <div>
          <h2 className="card-title">用量消耗与配额大盘</h2>
          <p className="kpi-subtext" style={{ marginTop: '0.25rem' }}>
            实时监控名下所有 API Key 的日级与周级 Token 配额水位，支持自适应色阶预警。
          </p>
        </div>
        <button
          type="button"
          className="btn btn-secondary btn-sm"
          onClick={loadUsage}
          disabled={loading}
        >
          {loading ? '刷新中...' : '刷新用量'}
        </button>
      </div>

      <div className="card" style={{ padding: '28px', marginBottom: '24px' }}>
        <h3 style={{ fontSize: '16px', fontWeight: 600, marginBottom: '20px' }}>
          Token 额度双水位进度条
        </h3>

        <QuotaBar
          label="今日已用额度 (Daily Quota)"
          used={dailyUsed}
          total={dailyLimit > 0 ? dailyLimit : 1000000}
          subtext="每日 UTC 00:00 自动重置。低于 70% 绿色正常，超过 90% 橙色告警，达到 100% 自动熔断。"
        />

        <QuotaBar
          label="本周累计已用额度 (Weekly Quota)"
          used={weeklyUsed}
          total={weeklyLimit > 0 ? weeklyLimit : 5000000}
          subtext="每周一 UTC 00:00 自动重置。周配额耗尽后无法发起新的模型调用。"
        />
      </div>

      <div className="kpi-grid">
        <div className="kpi-card">
          <div className="kpi-label">今日总消耗</div>
          <div className="kpi-value">{dailyUsed.toLocaleString()}</div>
          <div className="kpi-subtext">全 Key 今日累计消耗 Token</div>
        </div>
        <div className="kpi-card">
          <div className="kpi-label">本周总消耗</div>
          <div className="kpi-value">{weeklyUsed.toLocaleString()}</div>
          <div className="kpi-subtext">全 Key 本周累计消耗 Token</div>
        </div>
        <div className="kpi-card">
          <div className="kpi-label">配额重置周期</div>
          <div className="kpi-value">每日午夜</div>
          <div className="kpi-subtext">UTC 00:00 自动化时钟重置已用量</div>
        </div>
      </div>
    </div>
  );
};
