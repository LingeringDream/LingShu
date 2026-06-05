import { useState, useEffect } from 'react';
import { apiFetch } from '../../lib/api';

interface LlmSettings {
  model: string;
  temperature: number;
  max_tokens: number;
}

interface ChatSettingsProps {
  collapsed?: boolean;
}

export function ChatSettings({ collapsed: initialCollapsed = true }: ChatSettingsProps) {
  const [collapsed, setCollapsed] = useState(initialCollapsed);
  const [settings, setSettings] = useState<LlmSettings | null>(null);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Load settings on mount
  useEffect(() => {
    setLoading(true);
    apiFetch('/api/v1/settings/llm')
      .then((r) => (r.ok ? r.json() : Promise.reject(`HTTP ${r.status}`)))
      .then((data: LlmSettings) => {
        setSettings(data);
        setError(null);
      })
      .catch((e) => setError(`加载配置失败: ${e}`))
      .finally(() => setLoading(false));
  }, []);

  const saveField = async (field: keyof LlmSettings, value: string | number) => {
    if (!settings) return;
    setSaving(true);
    setError(null);
    try {
      const patch: Partial<LlmSettings> = { [field]: value };
      const resp = await apiFetch('/api/v1/settings/llm', {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(patch),
      });
      if (!resp.ok) {
        const err = await resp.json();
        throw new Error(err?.error?.message ?? `HTTP ${resp.status}`);
      }
      const updated: LlmSettings = await resp.json();
      setSettings(updated);
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

  const labelStyle: React.CSSProperties = {
    fontSize: '12px',
    color: 'var(--text-secondary)',
    marginBottom: '4px',
    display: 'block',
  };

  const inputStyle: React.CSSProperties = {
    width: '100%',
    background: 'var(--bg-tertiary)',
    border: '1px solid var(--border)',
    borderRadius: '6px',
    padding: '8px 10px',
    color: 'var(--text-primary)',
    fontSize: '13px',
    outline: 'none',
    fontFamily: 'inherit',
  };

  return (
    <div style={{ borderBottom: '1px solid var(--border)' }}>
      {/* Header toggle */}
      <div
        style={{ ...headerStyle, padding: '12px 20px' }}
        onClick={() => setCollapsed(!collapsed)}
      >
        <span style={{ fontSize: '13px', fontWeight: 500, color: 'var(--text-secondary)' }}>
         模型配置
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
          {settings && (
            <>
              {/* Model name */}
              <label>
                <span style={labelStyle}>模型名称</span>
                <input
                  style={inputStyle}
                  value={settings.model}
                  placeholder="例如 gemma4:e4b"
                  disabled={saving}
                  onBlur={(e) => {
                    if (e.target.value !== settings.model) saveField('model', e.target.value);
                  }}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') (e.target as HTMLInputElement).blur();
                  }}
                  onChange={(e) =>
                    setSettings((s) => (s ? { ...s, model: e.target.value } : s))
                  }
                />
              </label>

              {/* Temperature */}
              <label>
                <span style={labelStyle}>
                  温度 ({(settings.temperature ?? 0.7).toFixed(1)})
                </span>
                <input
                  type="range"
                  min="0"
                  max="2"
                  step="0.1"
                  style={{ width: '100%', accentColor: 'var(--accent)' }}
                  value={settings.temperature ?? 0.7}
                  disabled={saving}
                  onMouseUp={(e) => {
                    const v = parseFloat((e.target as HTMLInputElement).value);
                    if (v !== settings.temperature) saveField('temperature', v);
                  }}
                  onChange={(e) =>
                    setSettings((s) => (s ? { ...s, temperature: parseFloat(e.target.value) } : s))
                  }
                />
              </label>

              {/* Max tokens */}
              <label>
                <span style={labelStyle}>最大 Tokens</span>
                <input
                  style={inputStyle}
                  type="number"
                  min={1}
                  max={32768}
                  value={settings.max_tokens ?? 2048}
                  disabled={saving}
                  onBlur={(e) => {
                    const v = parseInt(e.target.value, 10);
                    if (!isNaN(v) && v !== settings.max_tokens) saveField('max_tokens', v);
                  }}
                  onChange={(e) =>
                    setSettings((s) =>
                      s ? { ...s, max_tokens: parseInt(e.target.value, 10) || 0 } : s,
                    )
                  }
                />
              </label>

              {saving && (
                <span style={{ fontSize: '12px', color: 'var(--text-secondary)' }}>保存中...</span>
              )}
            </>
          )}
        </div>
      )}
    </div>
  );
}
