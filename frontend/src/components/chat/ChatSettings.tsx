import { useState, useEffect } from 'react';
import { apiFetch } from '../../lib/api';

interface LlmSettings {
  provider: string;
  api_key: string;
  api_base_url: string;
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
  const [saved, setSaved] = useState(false);

  // Ollama model list
  const [ollamaModels, setOllamaModels] = useState<string[]>([]);
  const [loadingModels, setLoadingModels] = useState(false);

  const fetchOllamaModels = async () => {
    setLoadingModels(true);
    try {
      const resp = await fetch('http://localhost:11434/api/tags');
      if (resp.ok) {
        const data: { models?: { name: string }[] } = await resp.json();
        setOllamaModels((data.models || []).map((m) => m.name));
      }
    } catch { /* Ollama not reachable */ }
    finally { setLoadingModels(false); }
  };

  // Load settings on mount
  useEffect(() => {
    setLoading(true);
    apiFetch('/api/v1/settings/llm')
      .then((r) => (r.ok ? r.json() : Promise.reject(`HTTP ${r.status}`)))
      .then((data: LlmSettings) => {
        setSettings(data);
        setError(null);
        // Fetch models if using Ollama
        if (!data.provider || data.provider === 'ollama') {
          fetchOllamaModels();
        }
      })
      .catch((e) => setError(`加载配置失败: ${e}`))
      .finally(() => setLoading(false));
  }, []);

  // When provider changes, fetch models
  useEffect(() => {
    if (settings?.provider === 'ollama' && ollamaModels.length === 0) {
      fetchOllamaModels();
    }
  }, [settings?.provider]);

  const handleSave = async () => {
    if (!settings) return;
    setSaving(true);
    setError(null);
    setSaved(false);
    try {
      const resp = await apiFetch('/api/v1/settings/llm', {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(settings),
      });
      if (!resp.ok) {
        const err = await resp.json().catch(() => ({}));
        throw new Error((err as { error?: { message?: string } }).error?.message ?? `HTTP ${resp.status}`);
      }
      const updated: LlmSettings = await resp.json();
      setSettings(updated);
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    } catch (e) {
      setError(e instanceof Error ? e.message : '保存失败');
    } finally {
      setSaving(false);
    }
  };

  const update = (patch: Partial<LlmSettings>) => {
    setSettings((s) => (s ? { ...s, ...patch } : s));
  };

  const headerStyle: React.CSSProperties = {
    display: 'flex', alignItems: 'center', justifyContent: 'space-between',
    cursor: 'pointer', userSelect: 'none',
  };

  const labelStyle: React.CSSProperties = {
    fontSize: '12px', color: 'var(--text-secondary)', marginBottom: '4px', display: 'block',
  };

  const inputStyle: React.CSSProperties = {
    width: '100%', background: 'var(--bg-tertiary)',
    border: '1px solid var(--border)', borderRadius: '6px',
    padding: '8px 10px', color: 'var(--text-primary)',
    fontSize: '13px', outline: 'none', fontFamily: 'inherit',
    boxSizing: 'border-box',
  };

  const saveBtnStyle: React.CSSProperties = {
    padding: '8px 0', fontSize: '13px', fontWeight: 500,
    background: saved ? '#4caf50' : 'var(--accent)',
    color: '#fff', border: 'none', borderRadius: 6, cursor: 'pointer',
    opacity: saving ? 0.7 : 1, transition: 'background 0.2s',
  };

  return (
    <div style={{ borderBottom: '1px solid var(--border)' }}>
      <div style={{ ...headerStyle, padding: '12px 20px' }} onClick={() => setCollapsed(!collapsed)}>
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
            <span style={{ fontSize: '12px', color: 'var(--error)', whiteSpace: 'pre-wrap' }}>{error}</span>
          )}
          {saved && (
            <span style={{ fontSize: '12px', color: '#4caf50' }}>已保存</span>
          )}
          {settings && (
            <>
              {/* Provider */}
              <label>
                <span style={labelStyle}>LLM 提供商</span>
                <select
                  style={inputStyle}
                  value={settings.provider || 'ollama'}
                  disabled={saving}
                  onChange={(e) => {
                    update({ provider: e.target.value });
                    if (e.target.value === 'ollama') fetchOllamaModels();
                  }}
                >
                  <option value="ollama">Ollama（本地）</option>
                  <option value="openai">OpenAI / 云端兼容</option>
                </select>
              </label>

              {/* OpenAI fields */}
              {(settings.provider === 'openai') && (
                <>
                  <label>
                    <span style={labelStyle}>API Base URL</span>
                    <input style={inputStyle} value={settings.api_base_url || ''}
                      placeholder="https://api.openai.com"
                      disabled={saving}
                      onChange={(e) => update({ api_base_url: e.target.value })} />
                  </label>
                  <label>
                    <span style={labelStyle}>API Key</span>
                    <input style={inputStyle} type="password" value={settings.api_key || ''}
                      placeholder="sk-..."
                      disabled={saving}
                      onChange={(e) => update({ api_key: e.target.value })} />
                  </label>
                </>
              )}

              {/* Model — dropdown for Ollama, text for OpenAI */}
              <label>
                <span style={labelStyle}>模型</span>
                {settings.provider === 'ollama' ? (
                  loadingModels ? (
                    <span style={{ fontSize: 12, color: '#888' }}>加载模型列表...</span>
                  ) : ollamaModels.length > 0 ? (
                    <select
                      style={inputStyle}
                      value={settings.model}
                      disabled={saving}
                      onChange={(e) => update({ model: e.target.value })}
                    >
                      {ollamaModels.map((m) => (
                        <option key={m} value={m}>{m}</option>
                      ))}
                    </select>
                  ) : (
                    <input style={inputStyle} value={settings.model}
                      placeholder="例如 gemma4:e4b"
                      disabled={saving}
                      onChange={(e) => update({ model: e.target.value })} />
                  )
                ) : (
                  <input style={inputStyle} value={settings.model}
                    placeholder="例如 gpt-4o"
                    disabled={saving}
                    onChange={(e) => update({ model: e.target.value })} />
                )}
              </label>

              {/* Temperature */}
              <label>
                <span style={labelStyle}>温度 ({(settings.temperature ?? 0.7).toFixed(1)})</span>
                <input type="range" min="0" max="2" step="0.1"
                  style={{ width: '100%', accentColor: 'var(--accent)' }}
                  value={settings.temperature ?? 0.7}
                  disabled={saving}
                  onChange={(e) => update({ temperature: parseFloat(e.target.value) })} />
              </label>

              {/* Max tokens */}
              <label>
                <span style={labelStyle}>最大 Tokens</span>
                <input style={inputStyle} type="number" min={1} max={32768}
                  value={settings.max_tokens ?? 2048}
                  disabled={saving}
                  onChange={(e) => update({ max_tokens: parseInt(e.target.value, 10) || 0 })} />
              </label>

              {/* Save button */}
              <button type="button" onClick={handleSave} disabled={saving} style={saveBtnStyle}>
                {saving ? '保存中...' : saved ? '已保存' : '保存配置'}
              </button>
            </>
          )}
        </div>
      )}
    </div>
  );
}
