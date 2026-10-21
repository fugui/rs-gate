import React from 'react';

interface QuotaBarProps {
  label: string;
  used: number;
  total: number;
  unit?: string;
  subtext?: string;
}

export const QuotaBar: React.FC<QuotaBarProps> = ({
  label,
  used,
  total,
  unit = 'Tokens',
  subtext,
}) => {
  const pct = total > 0 ? Math.min(100, (used / total) * 100) : 0;

  let colorCls = 'progress-green';
  if (pct >= 100) {
    colorCls = 'progress-red';
  } else if (pct >= 90) {
    colorCls = 'progress-orange';
  } else if (pct >= 70) {
    colorCls = 'progress-yellow';
  }

  return (
    <div className="quota-card" style={{ marginBottom: '1.25rem' }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: '0.5rem' }}>
        <span style={{ fontWeight: 600 }}>{label}</span>
        <span style={{ color: 'var(--text-secondary)', fontSize: '0.875rem' }}>
          {used.toLocaleString()} / {total.toLocaleString()} {unit} ({pct.toFixed(1)}%)
        </span>
      </div>
      <div className="progress-track">
        <div
          className={`progress-bar ${colorCls}`}
          style={{ width: `${pct}%`, transition: 'width 0.3s ease' }}
        />
      </div>
      {subtext && (
        <div style={{ fontSize: '0.75rem', color: 'var(--text-muted)', marginTop: '0.35rem' }}>
          {subtext}
        </div>
      )}
    </div>
  );
};
