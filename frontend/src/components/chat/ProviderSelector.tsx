import { useState, useEffect, useCallback } from 'react';
import { apiFetch } from '../../lib/api';
import { CheckIcon, CloseIcon, DotIcon } from '../icons';

// ── Types ──────────────────────────────────────────────────────────

export interface ProviderModel {
  id: string;
  name: string;
  display_name?: string;
}

interface BuiltInProvider {
  provider_id: string;
  name: string;
  base_url: string;
  protocol: string;
  needs_api_key: boolean;
  default_models: ProviderModel[];
}

export interface SavedProvider {
  id: string;
  provider_id: string;
  display_name: string;
  has_api_key: boolean;
  api_base_url: string | null;
  models: ProviderModel[];
  is_active: boolean;
  created_at: string;
}

export interface ProvidersResponse {
  built_in: BuiltInProvider[];
  saved: SavedProvider[];
}

interface ConnectivityResult {
  ok: boolean;
  message: string;
  latency_ms: number;
}

// ── Provider icons (emoji-based for MVP) ──────────────────────────

// Brand-ish accent color per provider; unknown ids fall back to a stable
// hashed hue. Rendered as a colored rounded-square avatar showing the
// provider's initial (replaces the old per-provider emoji map).
const PROVIDER_COLORS: Record<string, string> = {
  ollama: '#5a5a5a',
  openai: '#10a37f',
  anthropic: '#d97757',
  google: '#4285f4',
  deepseek: '#4d6bfe',
  openrouter: '#6566f1',
  siliconflow: '#7c3aed',
  qwen: '#615ced',
  zhipu: '#3859ff',
  moonshot: '#5b5bd6',
  yi: '#00a0e9',
  qianfan: '#2932e1',
  hunyuan: '#0052d9',
  doubao: '#2b5fff',
  mistral: '#ff7000',
  groq: '#f55036',
  custom: '#6b7280',
};

function hashHue(s: string): number {
  let h = 0;
  for (let i = 0; i < s.length; i += 1) h = (h * 31 + s.charCodeAt(i)) | 0;
  return Math.abs(h) % 360;
}

function getProviderColor(providerId: string): string {
  return PROVIDER_COLORS[providerId] || `hsl(${hashHue(providerId)} 55% 48%)`;
}

function getProviderInitial(providerId: string): string {
  return (providerId.trim()[0] || '?').toUpperCase();
}

/** Colored rounded-square avatar with the provider's initial. */
function ProviderAvatar({ providerId, size = 18 }: { providerId: string; size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" aria-hidden style={{ flexShrink: 0, display: 'block' }}>
      <rect width="24" height="24" rx="6" fill={getProviderColor(providerId)} />
      <text
        x="12"
        y="13"
        textAnchor="middle"
        dominantBaseline="central"
        fontSize="13"
        fontWeight="700"
        fill="#fff"
        fontFamily="system-ui, sans-serif"
      >
        {getProviderInitial(providerId)}
      </text>
    </svg>
  );
}

// ── Component ──────────────────────────────────────────────────────

interface ProviderSelectorProps {
  onProviderChange?: (providerId: string, model: string) => void;
  /** Called after each providers fetch so a parent (ChatSettings) can populate
   *  a model dropdown from the active provider — and stay in sync when the user
   *  activates or edits a provider here (each save re-fetches and re-emits). */
  onProvidersLoaded?: (data: ProvidersResponse) => void;
}

export function ProviderSelector({ onProviderChange, onProvidersLoaded }: ProviderSelectorProps) {
  const [providersData, setProvidersData] = useState<ProvidersResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Currently editing
  const [editingSavedId, setEditingSavedId] = useState<string | null>(null);
  const [showCreateForm, setShowCreateForm] = useState(false);

  // Create/edit form state
  const [formProviderId, setFormProviderId] = useState('');
  const [formDisplayName, setFormDisplayName] = useState('');
  const [formApiKey, setFormApiKey] = useState('');
  const [formBaseUrl, setFormBaseUrl] = useState('');
  const [formModels, setFormModels] = useState<ProviderModel[]>([]);
  const [formModelInput, setFormModelInput] = useState('');
  const [formActivate, setFormActivate] = useState(false);

  // Connectivity test
  const [testing, setTesting] = useState<string | null>(null);
  const [testResult, setTestResult] = useState<ConnectivityResult | null>(null);

  // Error from activate/delete actions (shown as a banner near the list).
  const [actionError, setActionError] = useState<string | null>(null);

  const [saving, setSaving] = useState(false);

  const fetchProviders = useCallback(async () => {
    try {
      const resp = await apiFetch('/api/v1/settings/providers');
      if (resp.ok) {
        const data: ProvidersResponse = await resp.json();
        setProvidersData(data);
        onProvidersLoaded?.(data);
      }
    } catch {
      // best-effort
    } finally {
      setLoading(false);
    }
  }, [onProvidersLoaded]);

  useEffect(() => {
    fetchProviders();
  }, [fetchProviders]);

  const resetForm = () => {
    setFormProviderId('');
    setFormDisplayName('');
    setFormApiKey('');
    setFormBaseUrl('');
    setFormModels([]);
    setFormModelInput('');
    setFormActivate(false);
    setEditingSavedId(null);
    setShowCreateForm(false);
  };

  const startCreate = (providerId: string) => {
    const builtIn = providersData?.built_in.find((p) => p.provider_id === providerId);
    setFormProviderId(providerId);
    setFormDisplayName(builtIn?.name || '');
    setFormApiKey('');
    setFormBaseUrl(builtIn?.base_url || '');
    setFormModels(builtIn?.default_models ? [...builtIn.default_models] : []);
    setFormActivate(true);
    setEditingSavedId(null);
    setShowCreateForm(true);
  };

  const startEdit = (saved: SavedProvider) => {
    setFormProviderId(saved.provider_id);
    setFormDisplayName(saved.display_name);
    setFormApiKey(''); // don't pre-fill, user can re-enter
    setFormBaseUrl(saved.api_base_url || '');
    setFormModels([...saved.models]);
    setFormActivate(saved.is_active);
    setEditingSavedId(saved.id);
    setShowCreateForm(true);
  };

  const addModel = () => {
    const trimmed = formModelInput.trim();
    if (!trimmed) return;
    if (formModels.some((m) => m.id === trimmed)) return;
    setFormModels([...formModels, { id: trimmed, name: trimmed, display_name: trimmed }]);
    setFormModelInput('');
  };

  const removeModel = (modelId: string) => {
    setFormModels(formModels.filter((m) => m.id !== modelId));
  };

  const handleSave = async () => {
    if (!formProviderId || !formDisplayName.trim()) return;
    setSaving(true);
    setError(null);
    try {
      if (editingSavedId) {
        // Update existing
        const body: Record<string, unknown> = {
          display_name: formDisplayName.trim(),
          models: formModels,
        };
        if (formApiKey) body.api_key = formApiKey;
        if (formBaseUrl) body.api_base_url = formBaseUrl;

        const resp = await apiFetch(`/api/v1/settings/providers/${editingSavedId}`, {
          method: 'PATCH',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(body),
        });
        if (!resp.ok) {
          const err = await resp.json().catch(() => ({}));
          throw new Error((err as { error?: { message?: string } }).error?.message ?? `HTTP ${resp.status}`);
        }

        // If activating, also call activate
        if (formActivate) {
          await apiFetch(`/api/v1/settings/providers/${editingSavedId}/activate`, {
            method: 'POST',
          });
        }
      } else {
        // Create new
        const body: Record<string, unknown> = {
          provider_id: formProviderId,
          display_name: formDisplayName.trim(),
          models: formModels,
          activate: formActivate,
        };
        if (formApiKey) body.api_key = formApiKey;
        if (formBaseUrl) body.api_base_url = formBaseUrl;

        const resp = await apiFetch('/api/v1/settings/providers', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(body),
        });
        if (!resp.ok) {
          const err = await resp.json().catch(() => ({}));
          throw new Error((err as { error?: { message?: string } }).error?.message ?? `HTTP ${resp.status}`);
        }
      }

      await fetchProviders();
      resetForm();
      if (formActivate && onProviderChange) {
        const model = formModels[0]?.id || '';
        onProviderChange(formProviderId, model);
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : '保存失败');
    } finally {
      setSaving(false);
    }
  };

  const handleActivate = async (savedId: string) => {
    setActionError(null);
    try {
      const resp = await apiFetch(`/api/v1/settings/providers/${savedId}/activate`, {
        method: 'POST',
      });
      if (resp.ok) {
        const activated: SavedProvider = await resp.json();
        await fetchProviders();
        const model = activated.models[0]?.id || '';
        onProviderChange?.(activated.provider_id, model);
      } else {
        const err = await resp.json().catch(() => ({}));
        setActionError(
          (err as { error?: { message?: string } }).error?.message ?? `激活失败：HTTP ${resp.status}`,
        );
      }
    } catch (e) {
      setActionError(e instanceof Error ? `激活失败：${e.message}` : '激活失败');
    }
  };

  const handleDelete = async (savedId: string) => {
    setActionError(null);
    try {
      const resp = await apiFetch(`/api/v1/settings/providers/${savedId}`, {
        method: 'DELETE',
      });
      if (resp.ok) {
        await fetchProviders();
        if (editingSavedId === savedId) resetForm();
      } else {
        const err = await resp.json().catch(() => ({}));
        setActionError(
          (err as { error?: { message?: string } }).error?.message ?? `删除失败：HTTP ${resp.status}`,
        );
      }
    } catch (e) {
      setActionError(e instanceof Error ? `删除失败：${e.message}` : '删除失败');
    }
  };

  const handleTest = async (savedId: string) => {
    setTesting(savedId);
    setTestResult(null);
    try {
      const resp = await apiFetch(`/api/v1/settings/providers/${savedId}/test`, {
        method: 'POST',
      });
      if (resp.ok) {
        const result: ConnectivityResult = await resp.json();
        setTestResult(result);
      } else {
        const err = await resp.json().catch(() => ({}));
        const msg = (err as { error?: { message?: string } }).error?.message ?? `HTTP ${resp.status}`;
        setTestResult({ ok: false, message: msg, latency_ms: 0 });
      }
    } catch (e) {
      setTestResult({ ok: false, message: e instanceof Error ? e.message : '请求失败', latency_ms: 0 });
    } finally {
      setTesting(null);
    }
  };

  // ── Styles ────────────────────────────────────────────────────

  const cardStyle: React.CSSProperties = {
    display: 'grid',
    gridTemplateColumns: 'repeat(auto-fill, minmax(140px, 1fr))',
    gap: '8px',
    padding: '12px 20px',
  };

  const chipStyle: React.CSSProperties = {
    display: 'flex',
    flexDirection: 'column',
    alignItems: 'center',
    gap: '4px',
    padding: '10px 8px',
    borderRadius: '8px',
    border: '1px solid var(--border)',
    background: 'var(--bg-secondary)',
    cursor: 'pointer',
    fontSize: '12px',
    color: 'var(--text-primary)',
    transition: 'all 0.15s',
  };

  const activeChipStyle: React.CSSProperties = {
    ...chipStyle,
    borderColor: 'var(--accent)',
    background: 'color-mix(in srgb, var(--accent) 10%, transparent)',
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
    boxSizing: 'border-box',
  };

  const labelStyle: React.CSSProperties = {
    fontSize: '12px',
    color: 'var(--text-secondary)',
    marginBottom: '4px',
    display: 'block',
  };

  const btnStyle: React.CSSProperties = {
    padding: '6px 12px',
    fontSize: '12px',
    fontWeight: 500,
    background: 'var(--accent)',
    color: '#fff',
    border: 'none',
    borderRadius: 6,
    cursor: 'pointer',
  };

  const smallBtnStyle: React.CSSProperties = {
    padding: '4px 8px',
    fontSize: '11px',
    background: 'transparent',
    color: 'var(--text-secondary)',
    border: '1px solid var(--border)',
    borderRadius: 4,
    cursor: 'pointer',
  };

  if (loading) {
    return <div style={{ padding: '12px 20px', fontSize: 13, color: 'var(--text-secondary)' }}>加载服务商...</div>;
  }

  return (
    <div style={{ borderBottom: '1px solid var(--border)' }}>
      {/* Header */}
      <div style={{ padding: '12px 20px', fontSize: '13px', fontWeight: 500, color: 'var(--text-secondary)' }}>
        LLM 服务商
      </div>

      {/* Built-in provider grid */}
      <div style={cardStyle}>
        {providersData?.built_in.map((p) => {
          const savedForProvider = providersData.saved.find(
            (s) => s.provider_id === p.provider_id && s.provider_id !== 'custom'
          );
          const isActive = savedForProvider?.is_active ?? false;
          return (
            <button
              key={p.provider_id}
              type="button"
              style={isActive ? activeChipStyle : chipStyle}
              onClick={() => startCreate(p.provider_id)}
              title={p.name}
            >
              <ProviderAvatar providerId={p.provider_id} size={22} />
              <span style={{ lineHeight: 1.2, textAlign: 'center' }}>
                {p.name.replace(/（.*）/, '')}
              </span>
              {isActive && (
                <span style={{ fontSize: '10px', color: 'var(--accent)' }}>已激活</span>
              )}
            </button>
          );
        })}
      </div>

      {/* Saved custom providers */}
      {providersData?.saved.filter((s) => s.provider_id === 'custom').length ? (
        <div style={{ padding: '0 20px 8px' }}>
          <div style={{ fontSize: '11px', color: 'var(--text-secondary)', marginBottom: '6px' }}>
            自定义服务商
          </div>
          <div style={{ display: 'flex', flexWrap: 'wrap', gap: '6px' }}>
            {providersData.saved
              .filter((s) => s.provider_id === 'custom')
              .map((s) => (
                <div
                  key={s.id}
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    gap: '6px',
                    padding: '4px 8px',
                    borderRadius: '6px',
                    border: `1px solid ${s.is_active ? 'var(--accent)' : 'var(--border)'}`,
                    background: s.is_active ? 'color-mix(in srgb, var(--accent) 10%, transparent)' : 'var(--bg-secondary)',
                    fontSize: '12px',
                    cursor: 'pointer',
                  }}
                  onClick={() => startEdit(s)}
                >
                  <ProviderAvatar providerId={s.provider_id} size={16} />
                  <span>{s.display_name}</span>
                  {s.is_active && <DotIcon size={8} style={{ color: 'var(--accent)' }} />}
                </div>
              ))}
          </div>
        </div>
      ) : null}

      {/* Saved providers action bar */}
      {providersData?.saved.length ? (
        <div style={{ padding: '0 20px 12px' }}>
          <div style={{ fontSize: '11px', color: 'var(--text-secondary)', marginBottom: '6px' }}>
            已配置的服务商
          </div>
          <div style={{ display: 'flex', flexDirection: 'column', gap: '6px' }}>
            {providersData.saved.map((s) => (
              <div
                key={s.id}
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: '8px',
                  padding: '8px 10px',
                  borderRadius: '6px',
                  border: `1px solid ${s.is_active ? 'var(--accent)' : 'var(--border)'}`,
                  background: s.is_active ? 'color-mix(in srgb, var(--accent) 8%, transparent)' : 'var(--bg-tertiary)',
                  fontSize: '12px',
                }}
              >
                <ProviderAvatar providerId={s.provider_id} size={18} />
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div style={{ fontWeight: 500 }}>{s.display_name}</div>
                  <div style={{ fontSize: '11px', color: 'var(--text-secondary)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                    {s.models[0]?.display_name || s.models[0]?.id || '未配置模型'}
                    {s.has_api_key ? (
                      <>
                        {' · API Key '}
                        <CheckIcon size={11} style={{ verticalAlign: '-1px' }} />
                      </>
                    ) : ' · 无 API Key'}
                  </div>
                </div>
                {s.is_active && (
                  <span style={{ fontSize: '10px', color: 'var(--accent)', fontWeight: 500 }}>使用中</span>
                )}
                <div style={{ display: 'flex', gap: '4px' }}>
                  {!s.is_active && (
                    <button
                      type="button"
                      style={{ ...smallBtnStyle, color: 'var(--accent)', borderColor: 'var(--accent)' }}
                      onClick={() => handleActivate(s.id)}
                    >
                      激活
                    </button>
                  )}
                  <button
                    type="button"
                    style={smallBtnStyle}
                    onClick={() => handleTest(s.id)}
                    disabled={testing === s.id}
                  >
                    {testing === s.id ? '测试中...' : '测试'}
                  </button>
                  <button
                    type="button"
                    style={smallBtnStyle}
                    onClick={() => startEdit(s)}
                  >
                    编辑
                  </button>
                  <button
                    type="button"
                    style={{ ...smallBtnStyle, color: '#e53935' }}
                    onClick={() => handleDelete(s.id)}
                  >
                    删除
                  </button>
                </div>
              </div>
            ))}
          </div>
        </div>
      ) : null}

      {/* Activate/delete action error */}
      {actionError && (
        <div
          style={{
            margin: '0 20px 12px',
            padding: '8px 12px',
            borderRadius: '6px',
            fontSize: '12px',
            background: 'rgba(229, 57, 53, 0.1)',
            color: '#e53935',
            border: '1px solid #e53935',
          }}
        >
          <CloseIcon size={13} style={{ verticalAlign: '-2px' }} /> {actionError}
        </div>
      )}

      {/* Connectivity test result */}
      {testResult && (
        <div
          style={{
            margin: '0 20px 12px',
            padding: '8px 12px',
            borderRadius: '6px',
            fontSize: '12px',
            background: testResult.ok ? 'rgba(76, 175, 80, 0.1)' : 'rgba(229, 57, 53, 0.1)',
            color: testResult.ok ? '#4caf50' : '#e53935',
            border: `1px solid ${testResult.ok ? '#4caf50' : '#e53935'}`,
          }}
        >
          {testResult.ok
            ? <CheckIcon size={13} style={{ verticalAlign: '-2px' }} />
            : <CloseIcon size={13} style={{ verticalAlign: '-2px' }} />}
          {' '}{testResult.message}
          {testResult.latency_ms > 0 && ` (${testResult.latency_ms}ms)`}
        </div>
      )}

      {/* Add custom provider button */}
      <div style={{ padding: '0 20px 12px' }}>
        <button
          type="button"
          style={{
            ...smallBtnStyle,
            width: '100%',
            padding: '8px',
            color: 'var(--accent)',
            borderColor: 'var(--accent)',
          }}
          onClick={() => startCreate('custom')}
        >
          + 添加自定义服务商
        </button>
      </div>

      {/* Create/Edit form */}
      {showCreateForm && (
        <div
          style={{
            margin: '0 20px 16px',
            padding: '16px',
            borderRadius: '8px',
            border: '1px solid var(--border)',
            background: 'var(--bg-tertiary)',
            display: 'flex',
            flexDirection: 'column',
            gap: '12px',
          }}
        >
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
            <span style={{ fontSize: '13px', fontWeight: 500 }}>
              {editingSavedId ? '编辑服务商' : '添加服务商'}
            </span>
            <button type="button" style={{ ...smallBtnStyle, fontSize: '14px' }} onClick={resetForm}>
              <CloseIcon size={14} />
            </button>
          </div>

          {/* Provider type (read-only for built-in) */}
          {formProviderId !== 'custom' && (
            <div style={{ fontSize: '12px', color: 'var(--text-secondary)' }}>
              类型：{providersData?.built_in.find((p) => p.provider_id === formProviderId)?.name || formProviderId}
            </div>
          )}

          {/* Display name */}
          <label>
            <span style={labelStyle}>名称</span>
            <input
              style={inputStyle}
              value={formDisplayName}
              placeholder="例如：我的 DeepSeek"
              onChange={(e) => setFormDisplayName(e.target.value)}
            />
          </label>

          {/* API Key */}
          <label>
            <span style={labelStyle}>API Key</span>
            <input
              style={inputStyle}
              type="password"
              value={formApiKey}
              placeholder={editingSavedId ? '留空则保持不变' : 'sk-...'}
              onChange={(e) => setFormApiKey(e.target.value)}
            />
          </label>

          {/* Base URL */}
          <label>
            <span style={labelStyle}>Base URL</span>
            <input
              style={inputStyle}
              value={formBaseUrl}
              placeholder="https://api.example.com"
              readOnly={formProviderId !== 'custom' && !!formBaseUrl}
              onChange={(e) => setFormBaseUrl(e.target.value)}
            />
          </label>

          {/* Models */}
          <label>
            <span style={labelStyle}>模型列表</span>
            <div style={{ display: 'flex', flexWrap: 'wrap', gap: '4px', marginBottom: '6px' }}>
              {formModels.map((m) => (
                <span
                  key={m.id}
                  style={{
                    display: 'inline-flex',
                    alignItems: 'center',
                    gap: '4px',
                    padding: '2px 8px',
                    borderRadius: '4px',
                    background: 'var(--bg-secondary)',
                    border: '1px solid var(--border)',
                    fontSize: '11px',
                  }}
                >
                  {m.display_name || m.id}
                  <button
                    type="button"
                    style={{ background: 'none', border: 'none', cursor: 'pointer', fontSize: '12px', color: 'var(--text-secondary)', padding: 0 }}
                    onClick={() => removeModel(m.id)}
                  >
                    ×
                  </button>
                </span>
              ))}
            </div>
            <div style={{ display: 'flex', gap: '4px' }}>
              <input
                style={{ ...inputStyle, flex: 1 }}
                value={formModelInput}
                placeholder="输入模型 ID，回车添加"
                onChange={(e) => setFormModelInput(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') {
                    e.preventDefault();
                    addModel();
                  }
                }}
              />
              <button type="button" style={smallBtnStyle} onClick={addModel}>
                添加
              </button>
            </div>
          </label>

          {/* Activate toggle */}
          <label style={{ display: 'flex', alignItems: 'center', gap: '8px', cursor: 'pointer', fontSize: '12px' }}>
            <input
              type="checkbox"
              checked={formActivate}
              onChange={(e) => setFormActivate(e.target.checked)}
            />
            设为当前使用的服务商
          </label>

          {/* Error */}
          {error && (
            <span style={{ fontSize: '12px', color: '#e53935' }}>{error}</span>
          )}

          {/* Save button */}
          <button
            type="button"
            style={{
              ...btnStyle,
              opacity: saving ? 0.7 : 1,
            }}
            disabled={saving || !formDisplayName.trim()}
            onClick={handleSave}
          >
            {saving ? '保存中...' : editingSavedId ? '更新' : '保存'}
          </button>
        </div>
      )}
    </div>
  );
}
