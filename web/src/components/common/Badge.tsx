import React from 'react';

interface ProtocolBadgeProps {
  name: string;
  active: boolean;
}

export const ProtocolBadge: React.FC<ProtocolBadgeProps> = ({ name, active }) => {
  return (
    <span className={`badge ${active ? 'badge-protocol-active' : 'badge-protocol-inactive'}`}>
      {active ? '🟢' : '⚪'} {name}
    </span>
  );
};

interface StatusBadgeProps {
  status: string;
}

export const StatusBadge: React.FC<StatusBadgeProps> = ({ status }) => {
  const s = status.toLowerCase();
  let cls = 'badge-status-healthy';
  let dot = '●';

  if (s.includes('circuit') || s.includes('broken') || s.includes('unhealthy')) {
    cls = 'badge-status-broken';
  } else if (s.includes('degraded')) {
    cls = 'badge-status-degraded';
  }

  return (
    <span className={`badge ${cls}`}>
      {dot} {status}
    </span>
  );
};

interface HttpStatusBadgeProps {
  code: number;
}

export const HttpStatusBadge: React.FC<HttpStatusBadgeProps> = ({ code }) => {
  let cls = 'badge-http-200';
  if (code === 429) {
    cls = 'badge-http-429';
  } else if (code >= 400) {
    cls = 'badge-http-5xx';
  }

  return <span className={`badge ${cls}`}>{code}</span>;
};
