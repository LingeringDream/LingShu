import { useState, useEffect, useCallback } from 'react';
import { apiFetch } from '../../lib/api';

interface PersonalityTraits {
  directness: number;
  warmth: number;
  proactivity: number;
  risk_tolerance: number;
  verbosity: number;
  formality: number;
  humor: number;
}

interface Snapshot {
  id: string;
  user_id: string;
  trait_values: PersonalityTraits;
  change_reason: string | null;
  source_memory_ids: string[];
  is_active: boolean;
  created_at: string;
}

const TRAIT_LABELS: Record<keyof PersonalityTraits, string> = {
  directness: '直接度',
  warmth: '亲和度',
  proactivity: '主动性',
  risk_tolerance: '风险容忍',
  verbosity: '详略度',
  formality: '正式度',
  humor: '幽默度',
};

const DEFAULT_TRAITS: PersonalityTraits = {
  directness: 0.5,
  warmth: 0.5,
  proactivity: 0.5,
  risk_tolerance: 0.5,
  verbosity: 0.5,
  formality: 0.5,
  humor: 0.5,
};

function getErrorMessage(error: unknown, fallback: string): string {
  return error instanceof Error ? error.message : fallback;
}

// ── Role Prompt (inlined for style consistency) ────────────────────

function RolePromptSection() {
  const [prompt, setPrompt] = useState('');
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    apiFetch('/api/v1/settings/role-prompt')
      .then((r) => (r.ok ? r.json() : Promise.reject(`HTTP ${r.status}`)))
      .then((d: { role_prompt: string }) => setPrompt(d.role_prompt))
      .catch(() => {}) // silently use empty default
      .finally(() => setLoading(false));
  }, []);

  const save = async () => {
    setSaving(true);
    setSaved(false);
    try {
      const resp = await apiFetch('/api/v1/settings/role-prompt', {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ role_prompt: prompt }),
      });
      if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    } catch {
      // silently ignore — keep the local value
    } finally {
      setSaving(false);
    }
  };

  if (loading) return null;

  return (
    <section style={{ marginBottom: 24 }}>
      <h4 style={{ margin: '0 0 12px' }}>角色扮演提示词</h4>
      <p style={{ fontSize: 13, color: 'var(--color-secondary-text, #888)', margin: '0 0 8px' }}>
        自定义 AI 角色设定（例如：猫娘、顾问、演员…），留空则使用默认人格。
      </p>
      <textarea
        value={prompt}
        onChange={(e) => setPrompt(e.target.value)}
        rows={4}
        placeholder="例如：你是一只名叫「小灵」的猫娘助手，说话时偶尔带喵~的口癖，性格活泼可爱但专业。"
        style={{
          width: '100%',
          background: 'var(--bg-tertiary, #f5f5f5)',
          border: '1px solid var(--color-border, #ddd)',
          borderRadius: 6,
          padding: '10px 12px',
          color: 'var(--text-primary, #333)',
          fontSize: 13,
          outline: 'none',
          fontFamily: 'inherit',
          resize: 'vertical',
          lineHeight: 1.5,
          boxSizing: 'border-box',
        }}
      />
      <button
        type="button"
        onClick={save}
        disabled={saving}
        style={{
          marginTop: 8,
          padding: '6px 16px',
          background: saved ? '#4caf50' : 'var(--color-accent, #0a73ff)',
          color: '#fff',
          border: 'none',
          borderRadius: 4,
          fontSize: 13,
          cursor: saving ? 'not-allowed' : 'pointer',
          opacity: saving ? 0.7 : 1,
          transition: 'background 0.2s',
        }}
      >
        {saving ? '保存中...' : saved ? '已保存' : '保存'}
      </button>
    </section>
  );
}

// ── Main Component ──────────────────────────────────────────────────

export function PersonalityCenter() {
  const [snapshots, setSnapshots] = useState<Snapshot[]>([]);
  const [active, setActive] = useState<Snapshot | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [traitValues, setTraitValues] = useState<PersonalityTraits>(DEFAULT_TRAITS);
  const [reason, setReason] = useState('manual-edit');
  const [saving, setSaving] = useState(false);

  const fetchSnapshots = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [snapRes, activeRes] = await Promise.all([
        apiFetch('/api/v1/personality/snapshots'),
        apiFetch('/api/v1/personality/snapshots/active'),
      ]);
      if (snapRes.ok) setSnapshots(await snapRes.json());
      if (activeRes.ok) {
        const a: Snapshot = await activeRes.json();
        setActive(a);
        setTraitValues(a.trait_values);
      }
    } catch (e) {
      setError(getErrorMessage(e, 'Failed to load personality data'));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchSnapshots();
  }, [fetchSnapshots]);

  const handleSave = async () => {
    setSaving(true);
    setError(null);
    try {
      const res = await apiFetch('/api/v1/personality/snapshots', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          trait_values: traitValues,
          change_reason: reason || null,
          source_memory_ids: [],
        }),
      });
      if (!res.ok) throw new Error('Failed to create snapshot');
      await fetchSnapshots();
    } catch (e) {
      setError(getErrorMessage(e, 'Save failed'));
    } finally {
      setSaving(false);
    }
  };

  const handleActivate = async (id: string) => {
    setError(null);
    try {
      const res = await apiFetch(`/api/v1/personality/snapshots/${id}/activate`, {
        method: 'POST',
      });
      if (!res.ok) throw new Error('Failed to activate');
      await fetchSnapshots();
    } catch (e) {
      setError(getErrorMessage(e, 'Activation failed'));
    }
  };

  const setTrait = (key: keyof PersonalityTraits, value: number) => {
    setTraitValues((prev) => ({ ...prev, [key]: Math.round(value * 10) / 10 }));
  };

  const traitKeys = Object.keys(TRAIT_LABELS) as (keyof PersonalityTraits)[];

  if (loading) return <p style={{ padding: 16 }}>加载人格数据中...</p>;

  return (
    <div style={{ padding: 16 }}>
      <h3 style={{ margin: '0 0 16px' }}>
        人格中心 {active ? '(活跃)' : '(无活跃快照)'}
      </h3>

      {error && (
        <p style={{ color: '#e05555', fontSize: 13, marginBottom: 12 }}>{error}</p>
      )}

      {/* Role-play prompt — at the top, above trait sliders */}
      <RolePromptSection />

      {/* Trait editor */}
      <section style={{ marginBottom: 24 }}>
        <h4 style={{ margin: '0 0 12px' }}>调整人格参数</h4>
        {traitKeys.map((key) => (
          <div key={key} style={{ marginBottom: 10 }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: 13 }}>
              <span>{TRAIT_LABELS[key]}</span>
              <span style={{ color: 'var(--color-secondary-text, #888)' }}>
                {traitValues[key].toFixed(1)}
              </span>
            </div>
            <input
              type="range"
              min={0}
              max={1}
              step={0.1}
              value={traitValues[key]}
              onChange={(e) => setTrait(key, parseFloat(e.target.value))}
              style={{ width: '100%' }}
            />
          </div>
        ))}
        <div style={{ marginTop: 8 }}>
          <label style={{ fontSize: 13, marginRight: 8 }}>变更原因:</label>
          <select value={reason} onChange={(e) => setReason(e.target.value)} style={{ fontSize: 13 }}>
            <option value="manual-edit">手动编辑</option>
            <option value="auto-evolution">自动演化</option>
            <option value="rollback">回滚</option>
            <option value="identity-core-reset">重置身份核心</option>
          </select>
        </div>
        <button
          type="button"
          onClick={handleSave}
          disabled={saving}
          style={{
            marginTop: 12,
            padding: '6px 16px',
            background: 'var(--color-accent, #0a73ff)',
            color: '#fff',
            border: 'none',
            borderRadius: 4,
            cursor: 'pointer',
          }}
        >
          {saving ? '保存中...' : '保存新快照'}
        </button>
      </section>

      {/* History */}
      <section>
        <h4 style={{ margin: '0 0 12px' }}>快照历史 ({snapshots.length})</h4>
        {snapshots.length === 0 ? (
          <p style={{ fontSize: 13, color: 'var(--color-secondary-text, #888)' }}>
            暂无快照。调整上方参数并保存以创建第一个快照。
          </p>
        ) : (
          <div style={{ maxHeight: 300, overflowY: 'auto' }}>
            {snapshots.map((s) => (
              <div
                key={s.id}
                style={{
                  padding: '8px 12px',
                  marginBottom: 8,
                  border: '1px solid var(--color-border, #ddd)',
                  borderRadius: 6,
                  background: s.is_active ? '#e8f4ff' : 'transparent',
                  fontSize: 13,
                }}
              >
                <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 4 }}>
                  <span>
                    {new Date(s.created_at).toLocaleString('zh-CN')}
                    {s.is_active && <strong style={{ marginLeft: 8, color: '#0a73ff' }}>活跃</strong>}
                  </span>
                  {!s.is_active && (
                    <button
                      type="button"
                      onClick={() => handleActivate(s.id)}
                      style={{
                        padding: '2px 8px',
                        fontSize: 12,
                        border: '1px solid var(--color-accent, #0a73ff)',
                        borderRadius: 3,
                        background: 'transparent',
                        color: 'var(--color-accent, #0a73ff)',
                        cursor: 'pointer',
                      }}
                    >
                      激活
                    </button>
                  )}
                </div>
                <div style={{ color: 'var(--color-secondary-text, #888)', fontSize: 12 }}>
                  {s.change_reason || '未知原因'}
                </div>
              </div>
            ))}
          </div>
        )}
      </section>
    </div>
  );
}
