// Lightweight Native History Router with RBAC Guards
import { api } from './api.js';

export const routes = [
  { path: '/login', view: 'login', title: '登录', public: true },
  
  // Developer Portal
  { path: '/portal/keys', view: 'portal-keys', title: '我的 API Keys', roles: ['user', 'admin', 'superadmin'] },
  { path: '/portal/usage', view: 'portal-usage', title: '用量与配额', roles: ['user', 'admin', 'superadmin'] },
  { path: '/portal/models', view: 'portal-models', title: '模型广场与接入', roles: ['user', 'admin', 'superadmin'] },
  { path: '/portal/audits', view: 'portal-audits', title: '个人调用流水', roles: ['user', 'admin', 'superadmin'] },

  // Admin Ops Console
  { path: '/admin/dashboard', view: 'admin-dashboard', title: '集群监控大盘', roles: ['admin', 'superadmin'] },
  { path: '/admin/backends', view: 'admin-backends', title: '物理后端实例', roles: ['admin', 'superadmin'] },
  { path: '/admin/models', view: 'admin-models', title: '逻辑模型集群', roles: ['admin', 'superadmin'] },
  { path: '/admin/keys', view: 'admin-keys', title: '全局虚拟 Key', roles: ['admin', 'superadmin'] },
  { path: '/admin/audits', view: 'admin-audits', title: '全量调用审计', roles: ['admin', 'superadmin'] },
  { path: '/admin/users', view: 'admin-users', title: '用户与权限', roles: ['superadmin'] },
];

class Router {
  constructor() {
    this.currentRoute = null;
    window.addEventListener('popstate', () => this.handleLocation());
    document.addEventListener('click', (e) => {
      const link = e.target.closest('[data-link]');
      if (link) {
        e.preventDefault();
        const href = link.getAttribute('href');
        this.navigate(href);
      }
    });
  }

  navigate(path) {
    if (window.location.pathname !== path) {
      window.history.pushState(null, '', path);
    }
    this.handleLocation();
  }

  handleLocation() {
    const path = window.location.pathname;
    let route = routes.find(r => r.path === path);
    
    // Fallback default
    if (!route) {
      if (path === '/' || path === '') {
        const user = api.user;
        if (!user) {
          route = routes.find(r => r.path === '/login');
        } else if (user.role === 'user') {
          route = routes.find(r => r.path === '/portal/keys');
        } else {
          route = routes.find(r => r.path === '/admin/dashboard');
        }
      } else {
        route = routes.find(r => r.path === '/portal/keys') || routes[0];
      }
    }

    // Auth Guard
    const user = api.user;
    if (!route.public && !user) {
      if (window.location.pathname !== '/login') {
        window.history.replaceState(null, '', '/login');
      }
      route = routes.find(r => r.path === '/login');
    } else if (route.roles && user && !route.roles.includes(user.role)) {
      // Permission denied
      window.dispatchEvent(new CustomEvent('show-toast', {
        detail: { message: '权限不足，无法访问此管理视图', type: 'error' }
      }));
      const fallbackPath = user.role === 'user' ? '/portal/keys' : '/admin/dashboard';
      window.history.replaceState(null, '', fallbackPath);
      route = routes.find(r => r.path === fallbackPath);
    }

    this.currentRoute = route;
    document.title = `${route.title} - rs-gate`;
    window.dispatchEvent(new CustomEvent('route-changed', { detail: { route } }));
  }
}

export const router = new Router();
