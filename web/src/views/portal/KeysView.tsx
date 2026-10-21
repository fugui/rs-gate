import React, { useState, useEffect, useCallback } from 'react';
import { portalApi } from '../../services/portalApi';
import { useToast } from '../../context/ToastContext';
import { Modal } from '../../components/common/Modal';

export const KeysView: React.FC = () => {
  const { showToast } = useToast();
  const [keys, setKeys] = useState<any[]>([]);
  const [loading, setLoading] = useState(false);

  // Create Key Modal
  const [createModalOpen, setCreateModalOpen] = useState(false);
  const [keyName, setKeyName] = useState('');
  const [creating, setCreating] = useState(false);

  // Plaintext Key Modal
  const [plainKeyModalOpen, setPlainKeyModalOpen] = useState(false);
  const [plainKey, setPlainKey] = useState('');

  const loadKeys = useCallback(async () => {
    setLoading(true);
    try {
      const res = await portalApi.getUserKeys();
      setKeys(res.data || []);
    } catch (err: any) {
      showToast(err.message || '加载 Keys 失败', 'error');
    } finally {
      setLoading(false);
    }
  }, [showToast]);

  useEffect(() => {
    loadKeys();
  }, [loadKeys]);

  const handleCreate = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!keyName.trim()) return;

    setCreating(true);
    try {
      const res = await portalApi.createUserKey({ name: keyName.trim() });
      setCreateModalOpen(false);
      setKeyName('');
      const full = res.data.full_key || (res.data as any).plaintext_key || '';
      if (full) {
        setPlainKey(full);
        setPlainKeyModalOpen(true);
      } else {
        showToast('API Key 创建成功', 'success');
      }
      loadKeys();
    } catch (err: any) {
      showToast(err.message || '创建 Key 失败', 'error');
    } finally {
      setCreating(false);
    }
  };

  const handleDelete = async (id: string, name: string) => {
    if (!window.confirm(`确定停用或注销 Key "${name}" 吗？此操作无法撤销。`)) return;

    try {
      await portalApi.deleteUserKey(id);
      showToast('Key 已注销', 'success');
      loadKeys();
    } catch (err: any) {
      showToast(err.message || '注销失败', 'error');
    }
  };

  const handleCopyPlaintext = async () => {
    try {
      await navigator.clipboard.writeText(plainKey);
      showToast('密钥已复制到剪贴板', 'success');
    } catch {
      showToast('复制失败，请手动选中复制', 'error');
    }
  };

  return (
    <div>
      <div className="card-header" style={{ marginBottom: '1.5rem' }}>
        <div>
          <h2 className="card-title">我的虚拟 API Key</h2>
          <p className="kpi-subtext" style={{ marginTop: '0.25rem' }}>
            管理名下的调用密钥。密钥创建时仅明文展示一次，请妥善保管。
          </p>
        </div>
        <button
          type="button"
          className="btn btn-primary"
          onClick={() => setCreateModalOpen(true)}
        >
          + 申领新 Key
        </button>
      </div>

      <div className="card">
        <div className="table-responsive">
          <table className="data-table">
            <thead>
              <tr>
                <th>名称</th>
                <th>Key 前缀</th>
                <th>日配额已用</th>
                <th>周配额已用</th>
                <th>RPM 限流</th>
                <th>状态</th>
                <th>创建时间</th>
                <th>操作</th>
              </tr>
            </thead>
            <tbody>
              {loading ? (
                <tr>
                  <td colSpan={8} style={{ textAlign: 'center', padding: '32px' }}>
                    加载中...
                  </td>
                </tr>
              ) : keys.length === 0 ? (
                <tr>
                  <td colSpan={8} style={{ textAlign: 'center', padding: '32px', color: 'var(--text-muted)' }}>
                    暂无 API Key，请点击右上角按钮申领。
                  </td>
                </tr>
              ) : (
                keys.map((k) => {
                  const dUsed = k.quota_status ? k.quota_status.daily_used : k.daily_used_tokens || 0;
                  const dLimit = k.daily_quota || k.daily_quota_tokens || 0;
                  const wUsed = k.quota_status ? k.quota_status.weekly_used : k.weekly_used_tokens || 0;
                  const wLimit = k.weekly_quota || k.weekly_quota_tokens || 0;
                  const isActive = k.is_active === 1 || k.is_active === true;
                  const rpm = k.rpm_limit || k.rate_limit_rpm || 60;

                  return (
                    <tr key={k.id}>
                      <td style={{ fontWeight: 600 }}>{k.name}</td>
                      <td>
                        <code className="badge" style={{ background: 'var(--bg-app)' }}>
                          {k.key_prefix || 'sk-gate-***'}
                        </code>
                      </td>
                      <td>
                        {dLimit > 0
                          ? `${dUsed.toLocaleString()} / ${dLimit.toLocaleString()}`
                          : `${dUsed.toLocaleString()} / 无限制`}
                      </td>
                      <td>
                        {wLimit > 0
                          ? `${wUsed.toLocaleString()} / ${wLimit.toLocaleString()}`
                          : `${wUsed.toLocaleString()} / 无限制`}
                      </td>
                      <td>{rpm} req/min</td>
                      <td>
                        <span className={`badge ${isActive ? 'badge-status-healthy' : 'badge-status-broken'}`}>
                          {isActive ? '● 启用中' : '● 已停用'}
                        </span>
                      </td>
                      <td>
                        {k.created_at
                          ? new Date(k.created_at > 1e11 ? k.created_at : k.created_at * 1000).toLocaleString()
                          : '-'}
                      </td>
                      <td>
                        <button
                          type="button"
                          className="btn btn-secondary btn-sm"
                          style={{ color: 'var(--color-danger)' }}
                          onClick={() => handleDelete(k.id, k.name)}
                        >
                          停用 / 删除
                        </button>
                      </td>
                    </tr>
                  );
                })
              )}
            </tbody>
          </table>
        </div>
      </div>

      {/* 申领新 Key 弹窗 */}
      <Modal
        isOpen={createModalOpen}
        onClose={() => setCreateModalOpen(false)}
        title="申领新虚拟 API Key"
        footer={
          <>
            <button
              type="button"
              className="btn btn-secondary"
              onClick={() => setCreateModalOpen(false)}
            >
              取消
            </button>
            <button
              type="button"
              className="btn btn-primary"
              onClick={handleCreate}
              disabled={creating}
            >
              {creating ? '生成中...' : '确认创建'}
            </button>
          </>
        }
      >
        <form onSubmit={handleCreate}>
          <div className="form-group">
            <label className="form-label" style={{ display: 'block', marginBottom: '8px' }}>
              Key 名称 (用途描述)
            </label>
            <input
              type="text"
              className="form-control"
              value={keyName}
              onChange={(e) => setKeyName(e.target.value)}
              placeholder="例如: 电商推荐微服务、知识库评测脚本"
              required
              autoFocus
            />
          </div>
        </form>
      </Modal>

      {/* 明文密钥仅首次展示弹窗 */}
      <Modal
        isOpen={plainKeyModalOpen}
        onClose={() => setPlainKeyModalOpen(false)}
        title="🎉 API Key 创建成功！"
        footer={
          <button
            type="button"
            className="btn btn-primary"
            onClick={() => setPlainKeyModalOpen(false)}
          >
            我已妥善保存，关闭窗口
          </button>
        }
      >
        <div>
          <p style={{ fontSize: '13px', color: 'var(--color-warning)', margin: '0 0 16px 0', lineHeight: 1.6 }}>
            ⚠️ <strong>重要提示</strong>：以下明文密钥仅在此展示一次！关闭弹窗后系统将无法再次找回。请立即复制并妥善存储到安全环境。
          </p>
          <div
            className="code-box"
            style={{
              padding: '16px',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'space-between',
              gap: '12px',
            }}
          >
            <span
              style={{
                fontFamily: 'var(--font-mono)',
                color: 'white',
                wordBreak: 'break-all',
                fontSize: '14px',
              }}
            >
              {plainKey}
            </span>
            <button
              type="button"
              className="btn btn-secondary btn-sm"
              onClick={handleCopyPlaintext}
            >
              一键复制
            </button>
          </div>
        </div>
      </Modal>
    </div>
  );
};
