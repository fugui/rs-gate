// Main Application Bootstrap for rs-gate Web Console
import { api } from './api.js';
import { router } from './router.js';
import { UserView } from './user_view.js';
import { AdminView } from './admin_view.js';

class App {
  constructor() {
    this.initToastListener();
    this.initAuthListeners();
    this.initRouteListener();
    router.handleLocation();
  }

  initToastListener() {
    const container = document.getElementById('toast-container');
    window.addEventListener('show-toast', (e) => {
      const { message, type = 'info' } = e.detail;
      const toast = document.createElement('div');
      toast.className = `toast toast-${type}`;
      toast.textContent = message;
      container.appendChild(toast);
      setTimeout(() => {
        toast.style.opacity = '0';
        toast.style.transition = 'opacity 300ms ease';
        setTimeout(() => toast.remove(), 300);
      }, 3500);
    });
  }

  initAuthListeners() {
    window.addEventListener('unauthorized', () => {
      router.navigate('/login');
    });

    const logoutBtn = document.getElementById('logout-btn');
    if (logoutBtn) {
      logoutBtn.addEventListener('click', async () => {
        try {
          await api.logout();
        } catch {}
        api.clearSession();
        router.navigate('/login');
      });
    }
  }

  initRouteListener() {
    window.addEventListener('route-changed', (e) => {
      const { route } = e.detail;
      this.updateSidebar(route);
      this.renderView(route);
    });
  }

  updateSidebar(currentRoute) {
    const sidebar = document.getElementById('sidebar');
    const user = api.user;

    if (!user || currentRoute.view === 'login') {
      sidebar.style.display = 'none';
      document.querySelector('.main-viewport').style.marginLeft = '0';
      return;
    }

    sidebar.style.display = 'flex';
    document.querySelector('.main-viewport').style.marginLeft = '';

    // Update User Profile
    const userNameEl = document.getElementById('user-display-name');
    const userRoleEl = document.getElementById('user-role-badge');
    const userAvatarEl = document.getElementById('user-avatar');
    if (userNameEl) userNameEl.textContent = user.display_name || user.username;
    if (userRoleEl) userRoleEl.textContent = user.role;
    if (userAvatarEl) userAvatarEl.textContent = (user.username || 'U')[0].toUpperCase();

    // Toggle menu items based on role
    const adminNav = document.getElementById('admin-nav-section');
    if (adminNav) {
      adminNav.style.display = (user.role === 'admin' || user.role === 'superadmin') ? 'block' : 'none';
    }

    // Update active class on nav links
    document.querySelectorAll('.nav-item').forEach(el => {
      if (el.getAttribute('href') === currentRoute.path) {
        el.classList.add('active');
      } else {
        el.classList.remove('active');
      }
    });

    // Update topbar title
    const topbarTitle = document.getElementById('topbar-title');
    if (topbarTitle) {
      topbarTitle.textContent = currentRoute.title;
    }
  }

  renderView(route) {
    const main = document.getElementById('main-content');
    main.replaceChildren();

    switch (route.view) {
      case 'login':
        this.renderLoginView(main);
        break;
      case 'portal-keys':
        UserView.renderKeys(main);
        break;
      case 'portal-usage':
        UserView.renderUsage(main);
        break;
      case 'portal-models':
        UserView.renderModels(main);
        break;
      case 'portal-audits':
        UserView.renderAudits(main);
        break;
      case 'admin-dashboard':
        AdminView.renderDashboard(main);
        break;
      case 'admin-backends':
        AdminView.renderBackends(main);
        break;
      case 'admin-models':
        AdminView.renderModels(main);
        break;
      case 'admin-keys':
        AdminView.renderKeys(main);
        break;
      case 'admin-audits':
        AdminView.renderAudits(main);
        break;
      case 'admin-users':
        AdminView.renderUsers(main);
        break;
      default:
        UserView.renderKeys(main);
    }
  }

  renderLoginView(container) {
    const wrap = document.createElement('div');
    wrap.style.maxWidth = '400px';
    wrap.style.margin = '100px auto';
    wrap.className = 'card';

    const title = document.createElement('h2');
    title.className = 'card-title';
    title.style.textAlign = 'center';
    title.style.marginBottom = '24px';
    title.textContent = '登录 rs-gate 控制台';

    const form = document.createElement('form');

    const groupUser = document.createElement('div');
    groupUser.className = 'form-group';
    const labelUser = document.createElement('label');
    labelUser.className = 'form-label';
    labelUser.textContent = '用户名';
    const inputUser = document.createElement('input');
    inputUser.className = 'form-control';
    inputUser.required = true;
    inputUser.autofocus = true;
    groupUser.appendChild(labelUser);
    groupUser.appendChild(inputUser);

    const groupPass = document.createElement('div');
    groupPass.className = 'form-group';
    const labelPass = document.createElement('label');
    labelPass.className = 'form-label';
    labelPass.textContent = '密码';
    const inputPass = document.createElement('input');
    inputPass.type = 'password';
    inputPass.className = 'form-control';
    inputPass.required = true;
    groupPass.appendChild(labelPass);
    groupPass.appendChild(inputPass);

    const submitBtn = document.createElement('button');
    submitBtn.type = 'submit';
    submitBtn.className = 'btn btn-primary';
    submitBtn.style.width = '100%';
    submitBtn.style.marginTop = '12px';
    submitBtn.textContent = '登 录';

    form.appendChild(groupUser);
    form.appendChild(groupPass);
    form.appendChild(submitBtn);

    form.addEventListener('submit', async (e) => {
      e.preventDefault();
      submitBtn.disabled = true;
      submitBtn.textContent = '验证中...';

      try {
        const res = await api.login(inputUser.value.trim(), inputPass.value);
        api.setSession(res.data.token, res.data.user);
        window.dispatchEvent(new CustomEvent('show-toast', { detail: { message: `欢迎回来，${res.data.user.display_name || res.data.user.username}`, type: 'success' } }));
        if (res.data.user.role === 'user') {
          router.navigate('/portal/keys');
        } else {
          router.navigate('/admin/dashboard');
        }
      } catch (err) {
        window.dispatchEvent(new CustomEvent('show-toast', { detail: { message: err.message, type: 'error' } }));
        submitBtn.disabled = false;
        submitBtn.textContent = '登 录';
      }
    });

    wrap.appendChild(title);
    wrap.appendChild(form);
    container.appendChild(wrap);
  }
}

// Bootstrap app on DOMContentLoaded
window.addEventListener('DOMContentLoaded', () => {
  new App();
});
