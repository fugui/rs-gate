import React, { useState } from 'react';
import { useToast } from '../../context/ToastContext';

interface CodeBlockProps {
  language: string;
  code: string;
}

export const CodeBlock: React.FC<CodeBlockProps> = ({ language, code }) => {
  const { showToast } = useToast();
  const [copied, setCopied] = useState(false);

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(code);
      setCopied(true);
      showToast('代码已复制到剪贴板', 'success');
      setTimeout(() => setCopied(false), 2000);
    } catch {
      showToast('复制失败，请手动选中复制', 'error');
    }
  };

  return (
    <div className="code-box">
      <div className="code-header">
        <span style={{ fontSize: '0.85rem', fontWeight: 600, color: 'var(--text-secondary)' }}>
          {language}
        </span>
        <button
          type="button"
          className="btn btn-secondary btn-sm"
          onClick={handleCopy}
          style={{ fontSize: '0.75rem', padding: '0.2rem 0.6rem' }}
        >
          {copied ? '已复制 ✓' : '复制'}
        </button>
      </div>
      <pre className="code-content" style={{ margin: 0, overflowX: 'auto', padding: '1rem' }}>
        <code>{code}</code>
      </pre>
    </div>
  );
};
