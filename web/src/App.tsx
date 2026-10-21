import React, { useState, useEffect } from 'react';
import { useAuth } from './context/AuthContext';
import { useToast } from './context/ToastContext';
import { Sidebar } from './components/layout/Sidebar';
import { Navbar } from './components/layout/Navbar';
import { LoginView } from './views/auth/LoginView';

// Portal Views
import { KeysView } from './views/portal/KeysView';
import { UsageView } from './views/portal/UsageView';
import { ModelsView } from './views/portal/ModelsView';
import { LogsView } from './views/portal/LogsView';

// Admin Views
import { DashboardView } from './views/admin/DashboardView';
import { BackendsView } from './views/admin/BackendsView';
import { ModelsView as AdminModelsView } from './views/admin/ModelsView';
import { KeyAdminView } from './views/admin/KeyAdminView';
import { AuditLogsView } from './views/admin/AuditLogsView';
import { UsersView } from './views/admin/UsersView';

const pathToTitle: Record<string, string> = {
  '/portal/keys': '我的 API Keys',
  '/portal/usage': '用量与配额分析',
  '/portal/models': '模型广场与接入指南',
  '/portal/audits': '个人调用流水',
  '/admin/dashboard': '集群监控大盘',
  '/admin/backends': '物理后端实例与协议探针',
  '/admin/models': '逻辑模型集群拓扑',
  '/admin/keys': '全局虚拟 Key 监控',
  '/admin/audits': '全量调用审计',
  '/admin/users': '用户账户与权限管控',
};

export const App: React.FC = () => {
  const { isAuthenticated, isAdmin } = useAuth();
  const { showToast } = useToast();

  const getInitialPath = () => {
    const hash = window.location.hash.replace(/^#/, '');
    if (hash && hash !== '/login') return hash;
    const pathname = window.location.pathname;
    if (pathname && pathname !== '/' && pathname !== '/login') return pathname;
    return '/portal/keys';
  };

  const [currentPath, setCurrentPath] = useState<string>(getInitialPath);

  const navigate = (path: string) => {
    window.location.hash = '#' + path;
    setCurrentPath(path);
  };

  useEffect(() => {
    const handleHashChange = () => {
      const hash = window.location.hash.replace(/^#/, '');
      if (hash) {
        setCurrentPath(hash);
      }
    };

    window.addEventListener('hashchange', handleHashChange);
    return () => window.removeEventListener('hashchange', handleHashChange);
  }, []);

  // 未登录时渲染登录页
  if (!isAuthenticated) {
    return (
      <LoginView
        onSuccess={(role) => {
          if (role === 'admin' || role === 'superadmin') {
            navigate('/admin/dashboard');
          } else {
            navigate('/portal/keys');
          }
        }}
      />
    );
  }

  // RBAC 越权防护：普通用户访问 /admin 自动拦截
  if (currentPath.startsWith('/admin') && !isAdmin) {
    showToast('权限不足，无法访问管理员页面', 'error');
    navigate('/portal/keys');
    return null;
  }

  const renderCurrentView = () => {
    switch (currentPath) {
      case '/portal/keys':
        return <KeysView />;
      case '/portal/usage':
        return <UsageView />;
      case '/portal/models':
        return <ModelsView />;
      case '/portal/audits':
        return <LogsView />;
      case '/admin/dashboard':
        return <DashboardView />;
      case '/admin/backends':
        return <BackendsView />;
      case '/admin/models':
        return <AdminModelsView />;
      case '/admin/keys':
        return <KeyAdminView />;
      case '/admin/audits':
        return <AuditLogsView />;
      case '/admin/users':
        return <UsersView />;
      default:
        return isAdmin ? <DashboardView /> : <KeysView />;
    }
  };

  const pageTitle = pathToTitle[currentPath] || '控制台门户';

  return (
    <div id="app">
      <Sidebar currentPath={currentPath} onNavigate={navigate} />
      <main className="main-viewport">
        <Navbar title={pageTitle} />
        <div className="content-container">{renderCurrentView()}</div>
      </main>
    </div>
  );
};
