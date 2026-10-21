import React, { useState } from 'react';
import { useAuth } from '../../context/AuthContext';
import { useToast } from '../../context/ToastContext';
import { authApi } from '../../services/authApi';

interface LoginViewProps {
  onSuccess: (role: string) => void;
}

export const LoginView: React.FC<LoginViewProps> = ({ onSuccess }) => {
  const { login } = useAuth();
  const { showToast } = useToast();
  const [username, setUsername] = useState('superadmin');
  const [password, setPassword] = useState('AdminPass123!');
  const [loading, setLoading] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!username.trim() || !password.trim()) {
      showToast('请输入用户名和密码', 'warning');
      return;
    }

    setLoading(true);
    try {
      const res = await authApi.login(username, password);
      const data = res.data;
      login(data.token, data.user);
      showToast('登录成功，欢迎使用 rs-gate', 'success');
      onSuccess(data.user.role);
    } catch (err: any) {
      showToast(err.message || '登录失败，请检查用户名或密码', 'error');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div
      style={{
        minHeight: '100vh',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        background: 'radial-gradient(ellipse at top, hsl(226, 35%, 15%), var(--bg-app))',
        padding: '20px',
      }}
    >
      <div
        className="card"
        style={{
          width: '100%',
          maxWidth: '420px',
          padding: '36px',
          borderRadius: '16px',
          boxShadow: 'var(--shadow-lg)',
        }}
      >
        <div style={{ textAlign: 'center', marginBottom: '28px' }}>
          <div
            style={{
              width: '48px',
              height: '48px',
              margin: '0 auto 12px auto',
              background: 'linear-gradient(135deg, var(--color-primary), var(--color-accent))',
              borderRadius: '12px',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              fontSize: '22px',
              fontWeight: 800,
              color: 'white',
              boxShadow: '0 0 16px hsla(226, 75%, 58%, 0.4)',
            }}
          >
            RS
          </div>
          <h2 style={{ fontSize: '20px', fontWeight: 700, margin: '0 0 6px 0' }}>rs-gate 网关控制台</h2>
          <p style={{ fontSize: '13px', color: 'var(--text-secondary)', margin: 0 }}>
            企业级大模型统一分发与自服务门户
          </p>
        </div>

        <form onSubmit={handleSubmit}>
          <div className="form-group" style={{ marginBottom: '16px' }}>
            <label className="form-label" style={{ display: 'block', marginBottom: '6px', fontSize: '13px' }}>
              用户名
            </label>
            <input
              type="text"
              className="form-control"
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              placeholder="请输入用户名"
              required
            />
          </div>

          <div className="form-group" style={{ marginBottom: '24px' }}>
            <label className="form-label" style={{ display: 'block', marginBottom: '6px', fontSize: '13px' }}>
              密码
            </label>
            <input
              type="password"
              className="form-control"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              placeholder="请输入密码"
              required
            />
          </div>

          <button
            type="submit"
            className="btn btn-primary"
            style={{ width: '100%', padding: '12px', fontSize: '14px', fontWeight: 600 }}
            disabled={loading}
          >
            {loading ? '登录中...' : '登 录'}
          </button>
        </form>

        <div style={{ marginTop: '20px', textAlign: 'center', fontSize: '12px', color: 'var(--text-muted)' }}>
          单二进制交付 • SQLite WAL 存储 • 毫秒级流式中继
        </div>
      </div>
    </div>
  );
};
