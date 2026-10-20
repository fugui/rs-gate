// Admin Operations Console View
import { api } from './api.js';

export class AdminView {
  static async renderDashboard(container) {
    container.replaceChildren();

    const header = document.createElement('div');
    header.className = 'card-header';
    const title = document.createElement('h2');
    title.className = 'card-title';
    title.textContent = '集群监控大盘 (Ops Dashboard)';
    header.appendChild(title);
    container.appendChild(header);

    const kpiGrid = document.createElement('div');
    kpiGrid.className = 'kpi-grid';
    container.appendChild(kpiGrid);

    const createKpi = (label, val, sub) => {
      const card = document.createElement('div');
      card.className = 'kpi-card';
      const l = document.createElement('div');
      l.className = 'kpi-label';
      l.textContent = label;
      const v = document.createElement('div');
      v.className = 'kpi-value';
      v.textContent = val;
      const s = document.createElement('div');
      s.className = 'kpi-subtext';
      s.textContent = sub;
      card.appendChild(l);
      card.appendChild(v);
      card.appendChild(s);
      return card;
    };

    try {
      const res = await api.getAdminStatsOverview();
      const stats = res.data || {};

      kpiGrid.appendChild(createKpi('实时 QPS', (stats.current_qps || 0).toFixed(1), '每秒处理请求数'));
      kpiGrid.appendChild(createKpi('在途长连接', stats.active_connections || 0, '当前正在转发的并发请求'));
      kpiGrid.appendChild(createKpi('今日 Token 吞吐', (stats.today_total_tokens || 0).toLocaleString(), '全集群累计消耗 Token'));
      kpiGrid.appendChild(createKpi('健康后端节点', `${stats.healthy_backends_count || 0} / ${stats.total_backends_count || 0}`, '活跃探活正常率'));
    } catch (err) {
      kpiGrid.appendChild(createKpi('系统状态', '在线', '部分统计接口加载中'));
    }
  }

  static async renderBackends(container) {
    container.replaceChildren();

    const header = document.createElement('div');
    header.className = 'card-header';
    const titleBox = document.createElement('div');
    const h2 = document.createElement('h2');
    h2.className = 'card-title';
    h2.textContent = '物理后端实例与协议探针';
    titleBox.appendChild(h2);
    header.appendChild(titleBox);
    container.appendChild(header);

    const card = document.createElement('div');
    card.className = 'card';
    const tableWrap = document.createElement('div');
    tableWrap.className = 'table-responsive';
    const table = document.createElement('table');
    table.className = 'data-table';

    const thead = document.createElement('thead');
    const tr = document.createElement('tr');
    ['ID / 节点名', '基础 URL', '权重', '支持协议徽章', '健康状态', '操作'].forEach(t => {
      const th = document.createElement('th');
      th.textContent = t;
      tr.appendChild(th);
    });
    thead.appendChild(tr);
    table.appendChild(thead);

    const tbody = document.createElement('tbody');
    table.appendChild(tbody);
    tableWrap.appendChild(table);
    card.appendChild(tableWrap);
    container.appendChild(card);

    try {
      const res = await api.getAdminBackends();
      const backends = res.data || [];

      backends.forEach(be => {
        const row = document.createElement('tr');

        const tdName = document.createElement('td');
        tdName.style.fontWeight = '600';
        tdName.textContent = `${be.name} (${be.id})`;
        row.appendChild(tdName);

        const tdUrl = document.createElement('td');
        tdUrl.textContent = be.endpoint_url || be.base_url;
        row.appendChild(tdUrl);

        const tdWeight = document.createElement('td');
        tdWeight.textContent = be.weight;
        row.appendChild(tdWeight);

        // Protocol capsules
        const tdCapsules = document.createElement('td');
        const caps = be.capabilities || ['chat', 'responses'];
        ['chat', 'responses', 'completions', 'embeddings'].forEach(p => {
          const badge = document.createElement('span');
          const active = caps.includes(p);
          badge.className = `badge ${active ? 'badge-protocol-active' : 'badge-protocol-inactive'}`;
          badge.textContent = `${active ? '🟢' : '⚪'} ${p}`;
          badge.style.marginRight = '4px';
          tdCapsules.appendChild(badge);
        });
        row.appendChild(tdCapsules);

        // Status
        const tdStatus = document.createElement('td');
        const st = document.createElement('span');
        const isHealthy = (be.status || be.health_status || '').toLowerCase().includes('health');
        st.className = isHealthy ? 'badge badge-status-healthy' : 'badge badge-status-broken';
        st.textContent = be.status || be.health_status || 'Healthy';
        tdStatus.appendChild(st);
        row.appendChild(tdStatus);

        // Probe button
        const tdAction = document.createElement('td');
        const probeBtn = document.createElement('button');
        probeBtn.className = 'btn btn-secondary btn-sm';
        probeBtn.textContent = '即时探活';
        probeBtn.addEventListener('click', async () => {
          probeBtn.disabled = true;
          probeBtn.textContent = '探测中...';
          try {
            await api.probeAdminBackend(be.id);
            window.dispatchEvent(new CustomEvent('show-toast', { detail: { message: `后端 ${be.id} 探活完成`, type: 'success' } }));
            AdminView.renderBackends(container);
          } catch (err) {
            window.dispatchEvent(new CustomEvent('show-toast', { detail: { message: `探活失败: ${err.message}`, type: 'error' } }));
            probeBtn.disabled = false;
            probeBtn.textContent = '即时探活';
          }
        });
        tdAction.appendChild(probeBtn);
        row.appendChild(tdAction);

        tbody.appendChild(row);
      });
    } catch (err) {
      const errTr = document.createElement('tr');
      const errTd = document.createElement('td');
      errTd.colSpan = 6;
      errTd.style.color = 'var(--color-danger)';
      errTd.textContent = `加载物理后端失败: ${err.message}`;
      errTr.appendChild(errTd);
      tbody.appendChild(errTr);
    }
  }

  static async renderModels(container) {
    container.replaceChildren();

    const header = document.createElement('div');
    header.className = 'card-header';
    const title = document.createElement('h2');
    title.className = 'card-title';
    title.textContent = '逻辑模型集群与重试容错热配置';
    header.appendChild(title);
    container.appendChild(header);

    const card = document.createElement('div');
    card.className = 'card';
    const tableWrap = document.createElement('div');
    tableWrap.className = 'table-responsive';
    const table = document.createElement('table');
    table.className = 'data-table';

    const thead = document.createElement('thead');
    const tr = document.createElement('tr');
    ['模型 ID', '展示名称', '重试次数 (Retries)', '退避间隔 (ms)', '挂载后端实例'].forEach(t => {
      const th = document.createElement('th');
      th.textContent = t;
      tr.appendChild(th);
    });
    thead.appendChild(tr);
    table.appendChild(thead);

    const tbody = document.createElement('tbody');
    table.appendChild(tbody);
    tableWrap.appendChild(table);
    card.appendChild(tableWrap);
    container.appendChild(card);

    try {
      const res = await api.getAdminModels();
      const models = res.data || [];

      models.forEach(m => {
        const row = document.createElement('tr');

        const tdId = document.createElement('td');
        tdId.style.fontWeight = '600';
        tdId.textContent = m.id;
        row.appendChild(tdId);

        const tdName = document.createElement('td');
        tdName.textContent = m.display_name || m.id;
        row.appendChild(tdName);

        const tdRetries = document.createElement('td');
        tdRetries.textContent = `${m.max_retries} 次`;
        row.appendChild(tdRetries);

        const tdBackoff = document.createElement('td');
        tdBackoff.textContent = `${m.retry_interval_ms} ms`;
        row.appendChild(tdBackoff);

        const tdBackends = document.createElement('td');
        const bes = m.backends || [];
        if (bes.length === 0) {
          tdBackends.textContent = '未挂载后端';
        } else {
          tdBackends.textContent = bes.map(b => `${b.backend_id} (权重:${b.weight})`).join(', ');
        }
        row.appendChild(tdBackends);

        tbody.appendChild(row);
      });
    } catch (err) {
      const errTr = document.createElement('tr');
      const errTd = document.createElement('td');
      errTd.colSpan = 5;
      errTd.style.color = 'var(--color-danger)';
      errTd.textContent = `加载模型拓扑失败: ${err.message}`;
      errTr.appendChild(errTd);
      tbody.appendChild(errTr);
    }
  }

  static async renderKeys(container) {
    container.replaceChildren();

    const header = document.createElement('div');
    header.className = 'card-header';
    const title = document.createElement('h2');
    title.className = 'card-title';
    title.textContent = '全局虚拟 Key 监控与一键配额重置';
    header.appendChild(title);
    container.appendChild(header);

    const card = document.createElement('div');
    card.className = 'card';
    const tableWrap = document.createElement('div');
    tableWrap.className = 'table-responsive';
    const table = document.createElement('table');
    table.className = 'data-table';

    const thead = document.createElement('thead');
    const tr = document.createElement('tr');
    ['Key ID', '所属用户', '名称', '前缀', '日配额已用', '周配额已用', '操作'].forEach(t => {
      const th = document.createElement('th');
      th.textContent = t;
      tr.appendChild(th);
    });
    thead.appendChild(tr);
    table.appendChild(thead);

    const tbody = document.createElement('tbody');
    table.appendChild(tbody);
    tableWrap.appendChild(table);
    card.appendChild(tableWrap);
    container.appendChild(card);

    try {
      const res = await api.getAdminKeys();
      const keys = (res.data && res.data.items) ? res.data.items : [];

      keys.forEach(k => {
        const row = document.createElement('tr');

        const tdId = document.createElement('td');
        tdId.textContent = k.id;
        row.appendChild(tdId);

        const tdUser = document.createElement('td');
        tdUser.textContent = k.user_username || k.user_id;
        row.appendChild(tdUser);

        const tdName = document.createElement('td');
        tdName.style.fontWeight = '600';
        tdName.textContent = k.name;
        row.appendChild(tdName);

        const tdPrefix = document.createElement('td');
        tdPrefix.textContent = k.key_prefix;
        row.appendChild(tdPrefix);

        const tdDaily = document.createElement('td');
        const dUsed = k.quota_status ? k.quota_status.daily_used : 0;
        tdDaily.textContent = `${dUsed.toLocaleString()} / ${k.daily_quota.toLocaleString()}`;
        row.appendChild(tdDaily);

        const tdWeekly = document.createElement('td');
        const wUsed = k.quota_status ? k.quota_status.weekly_used : 0;
        tdWeekly.textContent = `${wUsed.toLocaleString()} / ${k.weekly_quota.toLocaleString()}`;
        row.appendChild(tdWeekly);

        // Reset Quota Action
        const tdAction = document.createElement('td');
        const resetBtn = document.createElement('button');
        resetBtn.className = 'btn btn-secondary btn-sm';
        resetBtn.textContent = '重置今日配额';
        resetBtn.addEventListener('click', async () => {
          try {
            await api.resetAdminKeyQuota(k.id, 'daily');
            window.dispatchEvent(new CustomEvent('show-toast', { detail: { message: `Key ${k.id} 配额已清零`, type: 'success' } }));
            AdminView.renderKeys(container);
          } catch (err) {
            window.dispatchEvent(new CustomEvent('show-toast', { detail: { message: `重置失败: ${err.message}`, type: 'error' } }));
          }
        });
        tdAction.appendChild(resetBtn);
        row.appendChild(tdAction);

        tbody.appendChild(row);
      });
    } catch (err) {
      const errTr = document.createElement('tr');
      const errTd = document.createElement('td');
      errTd.colSpan = 7;
      errTd.style.color = 'var(--color-danger)';
      errTd.textContent = `加载 Key 列表失败: ${err.message}`;
      errTr.appendChild(errTd);
      tbody.appendChild(errTr);
    }
  }

  static async renderAudits(container) {
    container.replaceChildren();

    const header = document.createElement('div');
    header.className = 'card-header';
    const title = document.createElement('h2');
    title.className = 'card-title';
    title.textContent = '全局调用审计与流式 CSV 导出';

    const exportBtn = document.createElement('button');
    exportBtn.className = 'btn btn-primary btn-sm';
    exportBtn.textContent = '📥 导出为 CSV';
    exportBtn.addEventListener('click', async () => {
      exportBtn.disabled = true;
      exportBtn.textContent = '导出中...';
      try {
        await api.exportAdminAuditsCsv({ start_time: 0 });
        window.dispatchEvent(new CustomEvent('show-toast', { detail: { message: 'CSV 导出完成', type: 'success' } }));
      } catch (err) {
        window.dispatchEvent(new CustomEvent('show-toast', { detail: { message: `导出失败: ${err.message}`, type: 'error' } }));
      } finally {
        exportBtn.disabled = false;
        exportBtn.textContent = '📥 导出为 CSV';
      }
    });

    header.appendChild(title);
    header.appendChild(exportBtn);
    container.appendChild(header);

    const card = document.createElement('div');
    card.className = 'card';
    const tableWrap = document.createElement('div');
    tableWrap.className = 'table-responsive';
    const table = document.createElement('table');
    table.className = 'data-table';

    const thead = document.createElement('thead');
    const tr = document.createElement('tr');
    ['Trace ID', '模型', '后端节点', '重试次数', '状态', '耗时', 'Token 消耗', '时间'].forEach(t => {
      const th = document.createElement('th');
      th.textContent = t;
      tr.appendChild(th);
    });
    thead.appendChild(tr);
    table.appendChild(thead);

    const tbody = document.createElement('tbody');
    table.appendChild(tbody);
    tableWrap.appendChild(table);
    card.appendChild(tableWrap);
    container.appendChild(card);

    try {
      const res = await api.getAdminAudits({ page_size: 50 });
      const items = (res.data && res.data.items) ? res.data.items : [];

      items.forEach(a => {
        const row = document.createElement('tr');

        const tdTrace = document.createElement('td');
        tdTrace.textContent = (a.trace_id || '').slice(0, 8);
        row.appendChild(tdTrace);

        const tdModel = document.createElement('td');
        tdModel.textContent = a.model;
        row.appendChild(tdModel);

        const tdBe = document.createElement('td');
        tdBe.textContent = a.backend_id;
        row.appendChild(tdBe);

        const tdRetries = document.createElement('td');
        tdRetries.textContent = `${a.retry_count || 0} 次`;
        row.appendChild(tdRetries);

        const tdStatus = document.createElement('td');
        const stSpan = document.createElement('span');
        stSpan.className = a.http_status === 200 ? 'badge badge-http-200' : 'badge badge-http-5xx';
        stSpan.textContent = a.http_status;
        tdStatus.appendChild(stSpan);
        row.appendChild(tdStatus);

        const tdDur = document.createElement('td');
        tdDur.textContent = `${a.request_duration_ms}ms`;
        row.appendChild(tdDur);

        const tdTokens = document.createElement('td');
        tdTokens.textContent = a.total_tokens;
        row.appendChild(tdTokens);

        const tdTime = document.createElement('td');
        tdTime.textContent = new Date(a.created_at).toLocaleString();
        row.appendChild(tdTime);

        tbody.appendChild(row);
      });
    } catch (err) {
      const errTr = document.createElement('tr');
      const errTd = document.createElement('td');
      errTd.colSpan = 8;
      errTd.style.color = 'var(--color-danger)';
      errTd.textContent = `加载审计日志失败: ${err.message}`;
      errTr.appendChild(errTd);
      tbody.appendChild(errTr);
    }
  }

  static async renderUsers(container) {
    container.replaceChildren();

    const header = document.createElement('div');
    header.className = 'card-header';
    const title = document.createElement('h2');
    title.className = 'card-title';
    title.textContent = '用户账户与权限管控';
    header.appendChild(title);
    container.appendChild(header);

    const card = document.createElement('div');
    card.className = 'card';
    const tableWrap = document.createElement('div');
    tableWrap.className = 'table-responsive';
    const table = document.createElement('table');
    table.className = 'data-table';

    const thead = document.createElement('thead');
    const tr = document.createElement('tr');
    ['用户名', '显示姓名', '角色', '邮箱', '状态', '创建时间'].forEach(t => {
      const th = document.createElement('th');
      th.textContent = t;
      tr.appendChild(th);
    });
    thead.appendChild(tr);
    table.appendChild(thead);

    const tbody = document.createElement('tbody');
    table.appendChild(tbody);
    tableWrap.appendChild(table);
    card.appendChild(tableWrap);
    container.appendChild(card);

    try {
      const res = await api.getAdminUsers();
      const items = (res.data && res.data.items) ? res.data.items : [];

      items.forEach(u => {
        const row = document.createElement('tr');

        const tdUname = document.createElement('td');
        tdUname.style.fontWeight = '600';
        tdUname.textContent = u.username;
        row.appendChild(tdUname);

        const tdDisp = document.createElement('td');
        tdDisp.textContent = u.display_name || '-';
        row.appendChild(tdDisp);

        const tdRole = document.createElement('td');
        const roleSpan = document.createElement('span');
        roleSpan.className = 'badge';
        roleSpan.style.background = u.role === 'superadmin' ? 'hsla(260, 60%, 55%, 0.2)' : 'var(--bg-app)';
        roleSpan.textContent = u.role;
        tdRole.appendChild(roleSpan);
        row.appendChild(tdRole);

        const tdEmail = document.createElement('td');
        tdEmail.textContent = u.email || '-';
        row.appendChild(tdEmail);

        const tdSt = document.createElement('td');
        tdSt.textContent = u.is_active ? '启用' : '禁用';
        row.appendChild(tdSt);

        const tdCreated = document.createElement('td');
        tdCreated.textContent = new Date(u.created_at).toLocaleString();
        row.appendChild(tdCreated);

        tbody.appendChild(row);
      });
    } catch (err) {
      const errTr = document.createElement('tr');
      const errTd = document.createElement('td');
      errTd.colSpan = 6;
      errTd.style.color = 'var(--color-danger)';
      errTd.textContent = `加载用户列表失败: ${err.message}`;
      errTr.appendChild(errTd);
      tbody.appendChild(errTr);
    }
  }
}
