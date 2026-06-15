import { useState, useEffect, useCallback } from 'react';
import { apiFetch } from '../../lib/api';
import { ProviderSelector, type ProvidersResponse, type SavedProvider } from './ProviderSelector';
import { ChevronIcon } from '../icons';

interface LlmSettings {
  provider: string;
  api_key: string;
  api_base_url: string;
  model: string;
  temperature: number;
  max_tokens: number;
  context_messages: number;
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
  // The currently-active provider, used to populate the model combobox and to
  // persist a newly-typed model back into its list. Kept in sync by
  // ProviderSelector, which re-emits on every fetch (load / activate / edit).
  const [activeProvider, setActiveProvider] = useState<SavedProvider | null>(null);

  const handleProvidersLoaded = useCallback((data: ProvidersResponse) => {
    setActiveProvider(data.saved.find((s) => s.is_active) ?? null);
  }, []);

  const loadSettings = useCallback(async () => {
    setLoading(true);
    try {
      const resp = await apiFetch('/api/v1/settings/llm');
      if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
      const data: LlmSettings = await resp.json();
      setSettings(data);
      setError(null);
    } catch (e) {
      setError(`加载配置失败: ${e instanceof Error ? e.message : e}`);
    } finally {
      setLoading(false);
    }
  }, []);

  // Load settings on mount
  useEffect(() => {
    void loadSettings();
  }, [loadSettings]);

  const handleSave = async () => {
    if (!settings) return;
    setSaving(true);
    setError(null);
    setSaved(false);
    try {
      const resp = await apiFetch('/api/v1/settings/llm', {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          model: settings.model,
          temperature: settings.temperature,
          max_tokens: settings.max_tokens,
          context_messages: settings.context_messages,
        }),
      });
      if (!resp.ok) {
        const err = await resp.json().catch(() => ({}));
        throw new Error((err as { error?: { message?: string } }).error?.message ?? `HTTP ${resp.status}`);
      }
      const updated: LlmSettings = await resp.json();
      setSettings(updated);

      // Combobox "input adds to list": if the user typed a model that isn't yet
      // in the active provider's list, persist it back so it appears in the
      // dropdown next time. Best-effort — the model already works regardless.
      const model = updated.model.trim();
      if (model && activeProvider && !activeProvider.models.some((m) => m.id === model)) {
        const newModels = [...activeProvider.models, { id: model, name: model, display_name: model }];
        try {
          await apiFetch(`/api/v1/settings/providers/${activeProvider.id}`, {
            method: 'PATCH',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ display_name: activeProvider.display_name, models: newModels }),
          });
          setActiveProvider({ ...activeProvider, models: newModels });
        } catch { /* ignore — model still usable as the active model */ }
      }

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

  // Suggestions for the model combobox — the active provider's models.
  const providerModels = activeProvider?.models ?? [];

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
        <ChevronIcon
          size={14}
          style={{ color: 'var(--text-secondary)', transform: collapsed ? 'none' : 'rotate(90deg)', transition: 'transform 0.15s' }}
        />
      </div>

      {!collapsed && (
        <div style={{ display: 'flex', flexDirection: 'column' }}>
          {/* Provider selector */}
          <ProviderSelector
            onProviderChange={(providerId, model) => {
              setSettings((s) => s ? { ...s, provider: providerId, model } : s);
              void loadSettings();
            }}
            onProvidersLoaded={handleProvidersLoaded}
          />

          {/* Model-specific settings */}
          {loading && <span style={{ padding: '0 20px', fontSize: '13px', color: 'var(--text-secondary)' }}>加载中...</span>}
          {error && (
            <span style={{ padding: '0 20px', fontSize: '12px', color: 'var(--error)', whiteSpace: 'pre-wrap' }}>{error}</span>
          )}
          {saved && (
            <span style={{ padding: '0 20px', fontSize: '12px', color: '#4caf50' }}>已保存</span>
          )}
          {settings && (
            <div style={{ padding: '12px 20px', display: 'flex', flexDirection: 'column', gap: '12px' }}>
              {/* Current provider display */}
              <div style={{ fontSize: '12px', color: 'var(--text-secondary)' }}>
                当前服务商：<strong>{settings.provider || '未选择'}</strong>
                {settings.model && <> · 模型：<strong>{settings.model}</strong></>}
              </div>

              {/* Model — a combobox: pick from the active provider's models, or
                  type a new one. A newly-typed model is persisted into the
                  provider's list on save (see handleSave). */}
              <label>
                <span style={labelStyle}>模型</span>
                <input style={inputStyle} value={settings.model}
                  list="lingshu-model-options"
                  placeholder={providerModels.length ? '从列表选择，或直接输入模型名' : '输入模型名（建议先在上方配置并激活服务商）'}
                  disabled={saving}
                  onChange={(e) => update({ model: e.target.value })} />
                <datalist id="lingshu-model-options">
                  {providerModels.map((m) => (
                    <option key={m.id} value={m.id}>{m.display_name || m.name || m.id}</option>
                  ))}
                </datalist>
                {providerModels.length === 0 && (
                  <span style={{ fontSize: '11px', color: 'var(--text-secondary)' }}>
                    还没有可选模型；输入的模型在保存后会加入列表。
                  </span>
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
                <input style={inputStyle} type="number" min={1} max={131072}
                  value={settings.max_tokens ?? 4096}
                  disabled={saving}
                  onChange={(e) => update({ max_tokens: parseInt(e.target.value, 10) || 0 })} />
              </label>

              {/* Context messages */}
              <label>
                <span style={labelStyle}>上下文轮数</span>
                <input style={inputStyle} type="number" min={1} max={10000}
                  value={settings.context_messages ?? 20}
                  disabled={saving}
                  onChange={(e) => update({ context_messages: parseInt(e.target.value, 10) || 0 })} />
              </label>

              {/* Save button */}
              <button type="button" onClick={handleSave} disabled={saving} style={saveBtnStyle}>
                {saving ? '保存中...' : saved ? '已保存' : '保存模型配置'}
              </button>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
