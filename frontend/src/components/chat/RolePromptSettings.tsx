import { useState, useEffect } from 'react';
import { apiFetch } from '../../lib/api';

interface RolePromptResponse {
  role_prompt: string;
}

interface RolePromptSettingsProps {
  collapsed?: boolean;
}

export function RolePromptSettings({ collapsed: initialCollapsed = true }: RolePromptSettingsProps) {
  const [collapsed, setCollapsed] = useState(initialCollapsed);
  const [prompt, setPrompt] = useState<string>('');
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);

  // Load prompt on mount
  useEffect(() => {
    setLoading(true);
    apiFetch('/api/v1/settings/role-prompt')
      .then((r) => (r.ok ? r.json() : Promise.reject(`HTTP ${r.status}`)))
      .then((data: RolePromptResponse) => {
        setPrompt(data.role_prompt);
        setError(null);
      })
      .catch((e) => setError(`加载失败: ${e}`))
      .finally(() => setLoading(false));
  }, []);

  const save = async () => {
    setSaving(true);
    setError(null);
    setSaved(false);
    try {
      const resp = await apiFetch('/api/v1/settings/role-prompt', {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ role_prompt: prompt }),
      });
      if (!resp.ok) {
        const err = await resp.json().catch(() => null);
        throw new Error(err?.error?.message ?? `HTTP ${resp.status}`);
      }
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    } catch (e) {
      setError(e instanceof Error ? e.message : '保存失败');
    } finally {
      setSaving(false);
    }
  };

  const headerStyle: React.CSSProperties = {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'space-between',
    cursor: 'pointer',
    userSelect: 'none',
  };

  const textareaStyle: React.CSSProperties = {
    width: '100%',
    minHeight: '80px',
    background: 'var(--bg-tertiary)',
    border: '1px solid var(--border)',
    borderRadius: '6px',
    padding: '10px 12px',
    color: 'var(--text-primary)',
    fontSize: '13px',
    outline: 'none',
    fontFamily: 'inherit',
    resize: 'vertical',
    lineHeight: '1.5',
  };

  return (
    <div style={{ borderBottom: '1px solid var(--border)' }}>
      {/* Header toggle */}
      <div
        style={{ ...headerStyle, padding: '12px 20px' }}
        onClick={() => setCollapsed(!collapsed)}
      >
        <span style={{ fontSize: '13px', fontWeight: 500, color: 'var(--text-secondary)' }}>
          🎭 角色扮演提示词
        </span>
        <span style={{ fontSize: '12px', color: 'var(--text-secondary)' }}>
          {collapsed ? '▶' : '▼'}
        </span>
      </div>

      {!collapsed && (
        <div style={{ padding: '0 20px 16px', display: 'flex', flexDirection: 'column', gap: '12px' }}>
          {loading && <span style={{ fontSize: '13px', color: 'var(--text-secondary)' }}>加载中...</span>}
          {error && (
            <span style={{ fontSize: '12px', color: 'var(--error)', whiteSpace: 'pre-wrap' }}>
              {error}
            </span>
          )}

          <label>
            <span style={{ fontSize: '12px', color: 'var(--text-secondary)', marginBottom: '4px', display: 'block' }}>
              自定义 AI 角色设定（例如：猫娘、顾问、演员…）留空则使用默认人格。
            </span>
            <textarea
              style={textareaStyle}
              value={prompt}
              placeholder={'例如：\n你是一只名叫「小灵」的猫娘助手，说话时偶尔带喵~的口癖，性格活泼可爱但专业。用简体中文交流。'}
              disabled={saving}
              onChange={(e) => setPrompt(e.target.value)}
              rows={4}
            />
          </label>

          <button
            onClick={save}
            disabled={saving}
            style={{
              alignSelf: 'flex-end',
              background: saved ? 'var(--success, #4caf50)' : 'var(--accent)',
              border: 'none',
              borderRadius: '6px',
              padding: '6px 16px',
              color: '#fff',
              fontSize: '13px',
              cursor: saving ? 'not-allowed' : 'pointer',
              opacity: saving ? 0.7 : 1,
              transition: 'background 0.2s',
            }}
          >
            {saving ? '保存中...' : saved ? '✓ 已保存' : '保存'}
          </button>
        </div>
      )}
    </div>
  );
}
