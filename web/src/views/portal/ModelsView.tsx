import React, { useState, useEffect, useCallback } from 'react';
import { portalApi } from '../../services/portalApi';
import { useToast } from '../../context/ToastContext';
import { CodeBlock } from '../../components/common/CodeBlock';
import { LogicalModel } from '../../types/gateway';

export const ModelsView: React.FC = () => {
  const { showToast } = useToast();
  const [models, setModels] = useState<LogicalModel[]>([]);
  const [loading, setLoading] = useState(false);
  const [selectedModel, setSelectedModel] = useState('deepseek-r1');
  const [langTab, setLangTab] = useState<'python' | 'curl' | 'typescript'>('python');

  const loadModels = useCallback(async () => {
    setLoading(true);
    try {
      const res = await portalApi.getUserModels();
      const list = res.data || [];
      setModels(list);
      if (list.length > 0 && !list.find((m) => m.id === selectedModel)) {
        setSelectedModel(list[0].id || list[0].name);
      }
    } catch (err: any) {
      showToast(err.message || '加载模型列表失败', 'error');
    } finally {
      setLoading(false);
    }
  }, [showToast, selectedModel]);

  useEffect(() => {
    loadModels();
  }, [loadModels]);

  const origin = window.location.origin;

  const pythonCode = `import openai

client = openai.OpenAI(
    base_url="${origin}/v1",
    api_key="<YOUR_GATEWAY_API_KEY>"
)

response = client.chat.completions.create(
    model="${selectedModel}",
    messages=[
        {"role": "system", "content": "You are a helpful AI assistant."},
        {"role": "user", "content": "Hello rs-gate!"}
    ],
    stream=True
)

for chunk in response:
    if chunk.choices and chunk.choices[0].delta.content:
        print(chunk.choices[0].delta.content, end="", flush=True)
print()`;

  const curlCode = `curl -X POST "${origin}/v1/chat/completions" \\
  -H "Content-Type: application/json" \\
  -H "Authorization: Bearer <YOUR_GATEWAY_API_KEY>" \\
  -d '{
    "model": "${selectedModel}",
    "messages": [
      {"role": "system", "content": "You are a helpful AI assistant."},
      {"role": "user", "content": "Hello rs-gate!"}
    ],
    "stream": true
  }'`;

  const tsCode = `import { OpenAI } from "openai";

const openai = new OpenAI({
  baseURL: "${origin}/v1",
  apiKey: process.env.GATEWAY_API_KEY || "<YOUR_GATEWAY_API_KEY>",
});

async function main() {
  const stream = await openai.chat.completions.create({
    model: "${selectedModel}",
    messages: [{ role: "user", content: "Hello rs-gate!" }],
    stream: true,
  });

  for await (const chunk of stream) {
    process.stdout.write(chunk.choices[0]?.delta?.content || "");
  }
}

main();`;

  const getCode = () => {
    if (langTab === 'python') return pythonCode;
    if (langTab === 'curl') return curlCode;
    return tsCode;
  };

  return (
    <div>
      <div className="card-header" style={{ marginBottom: '1.5rem' }}>
        <div>
          <h2 className="card-title">模型广场与快速接入指南</h2>
          <p className="kpi-subtext" style={{ marginTop: '0.25rem' }}>
            查看企业已开放的统一逻辑模型列表，并复制各语言 OpenAI 兼容 SDK 样例代码。
          </p>
        </div>
      </div>

      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1.4fr', gap: '24px', alignItems: 'start' }}>
        {/* 左侧：可用模型卡片列表 */}
        <div className="card" style={{ padding: '20px' }}>
          <h3 style={{ fontSize: '15px', fontWeight: 600, marginBottom: '16px' }}>
            已授权模型列表 ({models.length})
          </h3>

          {loading ? (
            <div style={{ textAlign: 'center', padding: '24px', color: 'var(--text-muted)' }}>
              加载中...
            </div>
          ) : models.length === 0 ? (
            <div style={{ textAlign: 'center', padding: '24px', color: 'var(--text-muted)' }}>
              暂无可访问模型，请联系管理员分配权限。
            </div>
          ) : (
            <div style={{ display: 'flex', flexDirection: 'column', gap: '10px' }}>
              {models.map((m) => {
                const isSelected = selectedModel === (m.id || m.name);
                return (
                  <div
                    key={m.id || m.name}
                    onClick={() => setSelectedModel(m.id || m.name)}
                    style={{
                      padding: '14px',
                      borderRadius: '8px',
                      border: isSelected
                        ? '1px solid var(--color-primary)'
                        : '1px solid var(--border-subtle)',
                      background: isSelected ? 'hsla(226, 75%, 58%, 0.1)' : 'var(--bg-app)',
                      cursor: 'pointer',
                      transition: 'all 0.15s ease',
                    }}
                  >
                    <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                      <span style={{ fontWeight: 600, fontSize: '14px' }}>{m.name || m.id}</span>
                      <span className="badge badge-status-healthy" style={{ fontSize: '11px' }}>
                        ● 可调用
                      </span>
                    </div>
                    {m.description && (
                      <div style={{ fontSize: '12px', color: 'var(--text-secondary)', marginTop: '4px' }}>
                        {m.description}
                      </div>
                    )}
                  </div>
                );
              })}
            </div>
          )}
        </div>

        {/* 右侧：代码接入指南与一键复制 */}
        <div className="card" style={{ padding: '20px' }}>
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '16px' }}>
            <h3 style={{ fontSize: '15px', fontWeight: 600, margin: 0 }}>
              多语言接入示例 ({selectedModel})
            </h3>
            <div style={{ display: 'flex', gap: '8px' }}>
              {(['python', 'curl', 'typescript'] as const).map((tab) => (
                <button
                  key={tab}
                  type="button"
                  className={`btn btn-sm ${langTab === tab ? 'btn-primary' : 'btn-secondary'}`}
                  onClick={() => setLangTab(tab)}
                  style={{ textTransform: 'capitalize' }}
                >
                  {tab === 'typescript' ? 'TS / Node' : tab}
                </button>
              ))}
            </div>
          </div>

          <p style={{ fontSize: '13px', color: 'var(--text-secondary)', marginBottom: '12px' }}>
            所有接口均为标准 OpenAI 兼容协议。将官方 SDK 中的 <code>base_url</code> 替换为网关地址即可。
          </p>

          <CodeBlock language={langTab.toUpperCase()} code={getCode()} />
        </div>
      </div>
    </div>
  );
};
