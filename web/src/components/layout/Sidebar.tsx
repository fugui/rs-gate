import React from 'react';
import { useAuth } from '../../context/AuthContext';

interface SidebarProps {
  currentPath: string;
  onNavigate: (path: string) => void;
}

export const Sidebar: React.FC<SidebarProps> = ({ currentPath, onNavigate }) => {
  const { user, isAdmin, logout } = useAuth();

  const handleNav = (e: React.MouseEvent, path: string) => {
    e.preventDefault();
    onNavigate(path);
  };

  const displayName = user?.display_name || user?.username || '开发者';
  const avatarLetter = (displayName[0] || 'U').toUpperCase();

  return (
    <aside className="sidebar">
      <div className="sidebar-header">
        <div className="sidebar-logo">RS</div>
        <span className="sidebar-title">rs-gate</span>
        <span className="sidebar-badge">Gateway</span>
      </div>

      <nav className="sidebar-nav">
        {/* Developer Portal Section */}
        <div className="nav-section-title">开发者自服务</div>
        <a
          href="#/portal/keys"
          className={`nav-item ${currentPath === '/portal/keys' ? 'active' : ''}`}
          onClick={(e) => handleNav(e, '/portal/keys')}
        >
          <span>🔑</span>
          <span>我的 API Keys</span>
        </a>
        <a
          href="#/portal/usage"
          className={`nav-item ${currentPath === '/portal/usage' ? 'active' : ''}`}
          onClick={(e) => handleNav(e, '/portal/usage')}
        >
          <span>📊</span>
          <span>用量与配额</span>
        </a>
        <a
          href="#/portal/models"
          className={`nav-item ${currentPath === '/portal/models' ? 'active' : ''}`}
          onClick={(e) => handleNav(e, '/portal/models')}
        >
          <span>🤖</span>
          <span>模型广场与接入</span>
        </a>
        <a
          href="#/portal/audits"
          className={`nav-item ${currentPath === '/portal/audits' ? 'active' : ''}`}
          onClick={(e) => handleNav(e, '/portal/audits')}
        >
          <span>📝</span>
          <span>个人调用流水</span>
        </a>

        {/* Admin Ops Section */}
        {isAdmin && (
          <div>
            <div className="nav-section-title">运维与集群治理</div>
            <a
              href="#/admin/dashboard"
              className={`nav-item ${currentPath === '/admin/dashboard' ? 'active' : ''}`}
              onClick={(e) => handleNav(e, '/admin/dashboard')}
            >
              <span>📈</span>
              <span>集群监控大盘</span>
            </a>
            <a
              href="#/admin/backends"
              className={`nav-item ${currentPath === '/admin/backends' ? 'active' : ''}`}
              onClick={(e) => handleNav(e, '/admin/backends')}
            >
              <span>🖥️</span>
              <span>物理后端实例</span>
            </a>
            <a
              href="#/admin/models"
              className={`nav-item ${currentPath === '/admin/models' ? 'active' : ''}`}
              onClick={(e) => handleNav(e, '/admin/models')}
            >
              <span>🔀</span>
              <span>逻辑模型集群</span>
            </a>
            <a
              href="#/admin/keys"
              className={`nav-item ${currentPath === '/admin/keys' ? 'active' : ''}`}
              onClick={(e) => handleNav(e, '/admin/keys')}
            >
              <span>🛡️</span>
              <span>全局虚拟 Key</span>
            </a>
            <a
              href="#/admin/audits"
              className={`nav-item ${currentPath === '/admin/audits' ? 'active' : ''}`}
              onClick={(e) => handleNav(e, '/admin/audits')}
            >
              <span>📜</span>
              <span>全量调用审计</span>
            </a>
            <a
              href="#/admin/users"
              className={`nav-item ${currentPath === '/admin/users' ? 'active' : ''}`}
              onClick={(e) => handleNav(e, '/admin/users')}
            >
              <span>👥</span>
              <span>用户与权限</span>
            </a>
          </div>
        )}
      </nav>

      <div className="sidebar-footer">
        <div className="user-profile">
          <div className="user-avatar">{avatarLetter}</div>
          <div className="user-meta">
            <span className="user-name">{displayName}</span>
            <span className="user-role-badge">{user?.role || 'user'}</span>
          </div>
        </div>
        <button
          type="button"
          className="btn btn-secondary btn-sm"
          onClick={() => logout()}
          title="退出登录"
        >
          退出
        </button>
      </div>
    </aside>
  );
};
