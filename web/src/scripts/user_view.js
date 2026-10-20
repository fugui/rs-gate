// User Self-Service Portal View
import { api } from './api.js';

export class UserView {
  static async renderKeys(container) {
    container.replaceChildren();

    const header = document.createElement('div');
    header.className = 'card-header';

    const titleBox = document.createElement('div');
    const h2 = document.createElement('h2');
    h2.className = 'card-title';
    h2.textContent = '我的虚拟 API Key';
    const sub = document.createElement('p');
    sub.className = 'kpi-subtext';
    sub.textContent = '管理名下的调用密钥。密钥创建时仅明文展示一次，请妥善保管。';
    titleBox.appendChild(h2);
    titleBox.appendChild(sub);

    const createBtn = document.createElement('button');
    createBtn.className = 'btn btn-primary';
    createBtn.textContent = '+ 申领新 Key';
    createBtn.addEventListener('click', () => UserView.showCreateKeyModal());

    header.appendChild(titleBox);
    header.appendChild(createBtn);
    container.appendChild(header);

    const card = document.createElement('div');
    card.className = 'card';

    const tableWrap = document.createElement('div');
    tableWrap.className = 'table-responsive';

    const table = document.createElement('table');
    table.className = 'data-table';

    const thead = document.createElement('thead');
    const trHead = document.createElement('tr');
    ['名称', 'Key 前缀', '日配额已用', '周配额已用', 'RPM 限流', '状态', '创建时间', '操作'].forEach(t => {
      const th = document.createElement('th');
      th.textContent = t;
      trHead.appendChild(th);
    });
    thead.appendChild(trHead);
    table.appendChild(thead);

    const tbody = document.createElement('tbody');
    table.appendChild(tbody);
    tableWrap.appendChild(table);
    card.appendChild(tableWrap);
    container.appendChild(card);

    try {
      const res = await api.getUserKeys();
      const keys = (res && res.data) ? res.data : [];

      if (keys.length === 0) {
        const emptyTr = document.createElement('tr');
        const emptyTd = document.createElement('td');
        emptyTd.colSpan = 8;
        emptyTd.style.textAlign = 'center';
        emptyTd.style.padding = '32px';
        emptyTd.textContent = '暂无 API Key，请点击右上角按钮申领。';
        emptyTr.appendChild(emptyTd);
        tbody.appendChild(emptyTr);
        return;
      }

      keys.forEach(k => {
        const tr = document.createElement('tr');

        // Name
        const tdName = document.createElement('td');
        tdName.style.fontWeight = '600';
        tdName.textContent = k.name;
        tr.appendChild(tdName);

        // Prefix
        const tdPrefix = document.createElement('td');
        const code = document.createElement('code');
        code.className = 'badge';
        code.style.background = 'var(--bg-app)';
        code.textContent = k.key_prefix || 'sk-gate-***';
        tdPrefix.appendChild(code);
        tr.appendChild(tdPrefix);

        // Daily Quota
        const tdDaily = document.createElement('td');
        const dUsed = k.quota_status ? k.quota_status.daily_used : 0;
        const dLimit = k.daily_quota;
        tdDaily.textContent = dLimit > 0 ? `${dUsed.toLocaleString()} / ${dLimit.toLocaleString()}` : `${dUsed.toLocaleString()} / 无限制`;
        tr.appendChild(tdDaily);

        // Weekly Quota
        const tdWeekly = document.createElement('td');
        const wUsed = k.quota_status ? k.quota_status.weekly_used : 0;
        const wLimit = k.weekly_quota;
        tdWeekly.textContent = wLimit > 0 ? `${wUsed.toLocaleString()} / ${wLimit.toLocaleString()}` : `${wUsed.toLocaleString()} / 无限制`;
        tr.appendChild(tdWeekly);

        // RPM
        const tdRpm = document.createElement('td');
        tdRpm.textContent = `${k.rpm_limit} req/min`;
        tr.appendChild(tdRpm);

        // Status
        const tdStatus = document.createElement('td');
        const stSpan = document.createElement('span');
        stSpan.className = k.is_active ? 'badge badge-status-healthy' : 'badge badge-status-broken';
        stSpan.textContent = k.is_active ? '● 启用中' : '● 已停用';
        tdStatus.appendChild(stSpan);
        tr.appendChild(tdStatus);

        // Created At
        const tdCreated = document.createElement('td');
        tdCreated.textContent = new Date(k.created_at).toLocaleString();
        tr.appendChild(tdCreated);

        // Actions
        const tdActions = document.createElement('td');
        const delBtn = document.createElement('button');
        delBtn.className = 'btn btn-secondary btn-sm';
        delBtn.style.color = 'var(--color-danger)';
        delBtn.textContent = '停用 / 删除';
        delBtn.addEventListener('click', async () => {
          if (confirm(`确定注销 Key "${k.name}" 吗？此操作无法恢复。`)) {
            try {
              await api.deleteUserKey(k.id);
              window.dispatchEvent(new CustomEvent('show-toast', { detail: { message: 'Key 已注销', type: 'success' } }));
              UserView.renderKeys(container);
            } catch (err) {
              window.dispatchEvent(new CustomEvent('show-toast', { detail: { message: err.message, type: 'error' } }));
            }
          }
        });
        tdActions.appendChild(delBtn);
        tr.appendChild(tdActions);

        tbody.appendChild(tr);
      });
    } catch (err) {
      const errTr = document.createElement('tr');
      const errTd = document.createElement('td');
      errTd.colSpan = 8;
      errTd.style.color = 'var(--color-danger)';
      errTd.textContent = `加载失败: ${err.message}`;
      errTr.appendChild(errTd);
      tbody.appendChild(errTr);
    }
  }

  static showCreateKeyModal() {
    const backdrop = document.getElementById('modal-backdrop');
    const panel = document.getElementById('modal-container');
    panel.replaceChildren();

    const title = document.createElement('h3');
    title.className = 'card-title';
    title.textContent = '申领新虚拟 API Key';

    const form = document.createElement('form');
    form.style.marginTop = '16px';

    const groupName = document.createElement('div');
    groupName.className = 'form-group';
    const labelName = document.createElement('label');
    labelName.className = 'form-label';
    labelName.textContent = 'Key 名称 (用途描述)';
    const inputName = document.createElement('input');
    inputName.className = 'form-control';
    inputName.placeholder = '例如: 电商推荐微服务、知识库评测脚本';
    inputName.required = true;
    groupName.appendChild(labelName);
    groupName.appendChild(inputName);
    form.appendChild(groupName);

    const footer = document.createElement('div');
    footer.className = 'modal-footer';

    const cancelBtn = document.createElement('button');
    cancelBtn.type = 'button';
    cancelBtn.className = 'btn btn-secondary';
    cancelBtn.textContent = '取消';
    cancelBtn.addEventListener('click', () => backdrop.classList.remove('open'));

    const submitBtn = document.createElement('button');
    submitBtn.type = 'submit';
    submitBtn.className = 'btn btn-primary';
    submitBtn.textContent = '确认创建';

    footer.appendChild(cancelBtn);
    footer.appendChild(submitBtn);
    form.appendChild(footer);

    form.addEventListener('submit', async (e) => {
      e.preventDefault();
      const name = inputName.value.trim();
      if (!name) return;

      submitBtn.disabled = true;
      submitBtn.textContent = '生成中...';

      try {
        const res = await api.createUserKey({ name });
        backdrop.classList.remove('open');
        UserView.showPlaintextKeyModal(res.data);
        const main = document.getElementById('main-content');
        UserView.renderKeys(main);
      } catch (err) {
        window.dispatchEvent(new CustomEvent('show-toast', { detail: { message: err.message, type: 'error' } }));
        submitBtn.disabled = false;
        submitBtn.textContent = '确认创建';
      }
    });

    panel.appendChild(title);
    panel.appendChild(form);
    backdrop.classList.add('open');
  }

  static showPlaintextKeyModal(keyData) {
    const backdrop = document.getElementById('modal-backdrop');
    const panel = document.getElementById('modal-container');
    panel.replaceChildren();

    const title = document.createElement('h3');
    title.className = 'card-title';
    title.style.color = 'var(--color-success)';
    title.textContent = '🎉 API Key 创建成功！';

    const warning = document.createElement('p');
    warning.style.fontSize = '13px';
    warning.style.color = 'var(--color-warning)';
    warning.style.margin = '12px 0';
    warning.textContent = '⚠️ 注意：以下明文密钥仅在此展示一次！关闭弹窗后无法再次找回。请立即复制并妥善存储到安全环境。';

    const codeBox = document.createElement('div');
    codeBox.className = 'code-box';
    codeBox.style.padding = '14px';
    codeBox.style.display = 'flex';
    codeBox.style.justifyContent = 'space-between';
    codeBox.style.alignItems = 'center';

    const keyText = document.createElement('span');
    keyText.style.fontFamily = 'var(--font-mono)';
    keyText.style.color = 'white';
    keyText.style.wordBreak = 'break-all';
    keyText.textContent = keyData.plaintext_key;

    const copyBtn = document.createElement('button');
    copyBtn.className = 'btn btn-primary btn-sm';
    copyBtn.textContent = '复制密钥';
    copyBtn.addEventListener('click', async () => {
      await navigator.clipboard.writeText(keyData.plaintext_key);
      copyBtn.textContent = '已复制！';
      setTimeout(() => { copyBtn.textContent = '复制密钥'; }, 2000);
    });

    codeBox.appendChild(keyText);
    codeBox.appendChild(copyBtn);

    const footer = document.createElement('div');
    footer.className = 'modal-footer';

    const doneBtn = document.createElement('button');
    doneBtn.className = 'btn btn-secondary';
    doneBtn.textContent = '我已妥善保存，关闭';
    doneBtn.addEventListener('click', () => backdrop.classList.remove('open'));
    footer.appendChild(doneBtn);

    panel.appendChild(title);
    panel.appendChild(warning);
    panel.appendChild(codeBox);
    panel.appendChild(footer);
    backdrop.classList.add('open');
  }

  static async renderUsage(container) {
    container.replaceChildren();

    const header = document.createElement('div');
    header.className = 'card-header';
    const title = document.createElement('h2');
    title.className = 'card-title';
    title.textContent = '个人用量概览与配额双进度条';
    header.appendChild(title);
    container.appendChild(header);

    try {
      const res = await api.getMe();
      const summary = res.data && res.data.quota_summary ? res.data.quota_summary : {};

      const dailyUsed = summary.daily_tokens_used || 0;
      const dailyLimit = summary.daily_tokens_limit || 1000000;
      const dailyPct = dailyLimit > 0 ? Math.min(100, (dailyUsed / dailyLimit) * 100) : 0;

      const weeklyUsed = summary.weekly_tokens_used || 0;
      const weeklyLimit = summary.weekly_tokens_limit || 5000000;
      const weeklyPct = weeklyLimit > 0 ? Math.min(100, (weeklyUsed / weeklyLimit) * 100) : 0;

      const getProgressColor = (pct) => {
        if (pct >= 100) return 'progress-red';
        if (pct >= 90) return 'progress-orange';
        if (pct >= 70) return 'progress-yellow';
        return 'progress-green';
      };

      // Daily Box
      const dailyBox = document.createElement('div');
      dailyBox.className = 'quota-box';
      const dMeta = document.createElement('div');
      dMeta.className = 'quota-meta';
      const dTitle = document.createElement('span');
      dTitle.className = 'quota-title';
      dTitle.textContent = '今日消耗 Token 配额';
      const dStats = document.createElement('span');
      dStats.className = 'quota-stats';
      dStats.textContent = `${dailyUsed.toLocaleString()} / ${dailyLimit.toLocaleString()} (${dailyPct.toFixed(1)}%)`;
      dMeta.appendChild(dTitle);
      dMeta.appendChild(dStats);

      const dTrack = document.createElement('div');
      dTrack.className = 'progress-track';
      const dBar = document.createElement('div');
      dBar.className = `progress-bar ${getProgressColor(dailyPct)}`;
      dBar.style.width = `${dailyPct}%`;
      dTrack.appendChild(dBar);
      dailyBox.appendChild(dMeta);
      dailyBox.appendChild(dTrack);

      // Weekly Box
      const weeklyBox = document.createElement('div');
      weeklyBox.className = 'quota-box';
      const wMeta = document.createElement('div');
      wMeta.className = 'quota-meta';
      const wTitle = document.createElement('span');
      wTitle.className = 'quota-title';
      wTitle.textContent = '本周消耗 Token 配额';
      const wStats = document.createElement('span');
      wStats.className = 'quota-stats';
      wStats.textContent = `${weeklyUsed.toLocaleString()} / ${weeklyLimit.toLocaleString()} (${weeklyPct.toFixed(1)}%)`;
      wMeta.appendChild(wTitle);
      wMeta.appendChild(wStats);

      const wTrack = document.createElement('div');
      wTrack.className = 'progress-track';
      const wBar = document.createElement('div');
      wBar.className = `progress-bar ${getProgressColor(weeklyPct)}`;
      wBar.style.width = `${weeklyPct}%`;
      wTrack.appendChild(wBar);
      weeklyBox.appendChild(wMeta);
      weeklyBox.appendChild(wTrack);

      container.appendChild(dailyBox);
      container.appendChild(weeklyBox);
    } catch (err) {
      const errBox = document.createElement('div');
      errBox.className = 'card';
      errBox.style.color = 'var(--color-danger)';
      errBox.textContent = `加载配额失败: ${err.message}`;
      container.appendChild(errBox);
    }
  }

  static async renderModels(container) {
    container.replaceChildren();

    const header = document.createElement('div');
    header.className = 'card-header';
    const title = document.createElement('h2');
    title.className = 'card-title';
    title.textContent = '模型广场与多语言接入指南';
    header.appendChild(title);
    container.appendChild(header);

    try {
      const res = await api.getUserModels();
      const models = (res && res.data) ? res.data : [];

      const grid = document.createElement('div');
      grid.style.display = 'grid';
      grid.style.gap = '24px';

      models.forEach(m => {
        const card = document.createElement('div');
        card.className = 'card';

        const mHead = document.createElement('div');
        mHead.className = 'card-header';
        const mTitle = document.createElement('h3');
        mTitle.className = 'card-title';
        mTitle.textContent = m.display_name || m.id;
        const mId = document.createElement('code');
        mId.className = 'badge';
        mId.textContent = m.id;
        mHead.appendChild(mTitle);
        mHead.appendChild(mId);

        const desc = document.createElement('p');
        desc.style.color = 'var(--text-secondary)';
        desc.style.fontSize = '14px';
        desc.textContent = m.description || '暂无描述';

        // Code snippets
        const codeTabs = document.createElement('div');
        codeTabs.style.marginTop = '16px';

        const codeBox = document.createElement('div');
        codeBox.className = 'code-box';

        const codeHead = document.createElement('div');
        codeHead.className = 'code-header';
        codeHead.textContent = 'Python (OpenAI SDK)';

        const copyBtn = document.createElement('button');
        copyBtn.className = 'btn btn-secondary btn-sm';
        copyBtn.textContent = '复制代码';

        const pre = document.createElement('pre');
        pre.className = 'code-content';
        const snippet = m.code_snippets && m.code_snippets.python 
          ? m.code_snippets.python 
          : `from openai import OpenAI\n\nclient = OpenAI(base_url="http://localhost:8080/v1", api_key="YOUR_KEY")\nresp = client.chat.completions.create(model="${m.id}", messages=[{"role": "user", "content": "Hello"}])\nprint(resp.choices[0].message.content)`;
        pre.textContent = snippet;

        copyBtn.addEventListener('click', async () => {
          await navigator.clipboard.writeText(snippet);
          copyBtn.textContent = '已复制！';
          setTimeout(() => { copyBtn.textContent = '复制代码'; }, 2000);
        });

        codeHead.appendChild(copyBtn);
        codeBox.appendChild(codeHead);
        codeBox.appendChild(pre);

        card.appendChild(mHead);
        card.appendChild(desc);
        card.appendChild(codeBox);
        grid.appendChild(card);
      });

      container.appendChild(grid);
    } catch (err) {
      const errBox = document.createElement('div');
      errBox.className = 'card';
      errBox.style.color = 'var(--color-danger)';
      errBox.textContent = `加载模型列表失败: ${err.message}`;
      container.appendChild(errBox);
    }
  }

  static async renderAudits(container) {
    container.replaceChildren();

    const header = document.createElement('div');
    header.className = 'card-header';
    const title = document.createElement('h2');
    title.className = 'card-title';
    title.textContent = '个人调用审计流水';
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
    ['Trace ID', '模型', '协议', '状态', '耗时', 'Token 消耗', '调用时间'].forEach(t => {
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
      const res = await api.getUserAudits();
      const items = (res.data && res.data.items) ? res.data.items : [];

      if (items.length === 0) {
        const emptyTr = document.createElement('tr');
        const emptyTd = document.createElement('td');
        emptyTd.colSpan = 7;
        emptyTd.style.textAlign = 'center';
        emptyTd.style.padding = '32px';
        emptyTd.textContent = '暂无调用审计记录。';
        emptyTr.appendChild(emptyTd);
        tbody.appendChild(emptyTr);
        return;
      }

      items.forEach(a => {
        const row = document.createElement('tr');

        const tdTrace = document.createElement('td');
        const code = document.createElement('code');
        code.className = 'badge';
        code.textContent = (a.trace_id || '').slice(0, 8);
        tdTrace.appendChild(code);
        row.appendChild(tdTrace);

        const tdModel = document.createElement('td');
        tdModel.textContent = a.model;
        row.appendChild(tdModel);

        const tdProto = document.createElement('td');
        tdProto.textContent = a.protocol;
        row.appendChild(tdProto);

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
        tdTokens.textContent = `${a.total_tokens} (P:${a.prompt_tokens} / C:${a.completion_tokens})`;
        row.appendChild(tdTokens);

        const tdTime = document.createElement('td');
        tdTime.textContent = new Date(a.created_at).toLocaleString();
        row.appendChild(tdTime);

        tbody.appendChild(row);
      });
    } catch (err) {
      const errTr = document.createElement('tr');
      const errTd = document.createElement('td');
      errTd.colSpan = 7;
      errTd.style.color = 'var(--color-danger)';
      errTd.textContent = `加载调用流水失败: ${err.message}`;
      errTr.appendChild(errTd);
      tbody.appendChild(errTr);
    }
  }
}
