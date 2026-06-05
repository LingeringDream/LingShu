import { useState, useEffect } from 'react';
import { apiFetch } from '../../lib/api';

interface Permissions {
  l0_enabled: boolean;
  l1_calendar: boolean;
  l1_require_confirmation: boolean;
  l2_automation: boolean;
  l2_whitelist_only: boolean;
  l3_accessibility: boolean;
  l4_autonomous: boolean;
}

const TIER_INFO: { key: keyof Permissions; label: string; desc: string; editable: boolean }[] = [
  { key: 'l0_enabled', label: 'L0 · 聊天与宠物', desc: '桌面宠物、对话、记忆展示', editable: false },
  { key: 'l1_calendar', label: 'L1 · 日历', desc: '创建/修改 Apple Calendar 日程', editable: true },
  { key: 'l1_require_confirmation', label: 'L1 · 日程需确认', desc: '每个日程创建前展示确认卡片', editable: true },
  { key: 'l2_automation', label: 'L2 · 自动化', desc: '打开 App、URL、运行 Shortcuts', editable: true },
  { key: 'l3_accessibility', label: 'L3 · 辅助操控', desc: '键盘输入、辅助功能树读取', editable: true },
  { key: 'l4_autonomous', label: 'L4 · 自主操控', desc: '屏幕识别 + 自主点击（远期）', editable: true },
];

export function PermissionSettings() {
  const [perms, setPerms] = useState<Permissions | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [expanded, setExpanded] = useState(false);

  useEffect(() => {
    apiFetch('/api/v1/permissions')
      .then((r) => (r.ok ? r.json() : Promise.reject(`HTTP ${r.status}`)))
      .then((data: Permissions) => { setPerms(data); setLoading(false); })
      .catch((e) => { setError(e); setLoading(false); });
  }, []);

  const toggle = async (key: keyof Permissions) => {
    if (!perms) return;
    const newValue = !perms[key];
    setPerms({ ...perms, [key]: newValue });
    try {
      await apiFetch('/api/v1/permissions', {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ [key]: newValue }),
      });
    } catch (e) {
      setPerms({ ...perms, [key]: !newValue });
      setError(e instanceof Error ? e.message : '保存失败');
    }
  };

  return (
    <div>
      <div
        style={{
          padding: '12px 20px', borderBottom: '1px solid var(--border)',
          display: 'flex', justifyContent: 'space-between', alignItems: 'center',
          cursor: 'pointer',
        }}
        onClick={() => setExpanded(!expanded)}
      >
        <span style={{ fontSize: '13px', fontWeight: 500, color: 'var(--text-secondary)' }}>
          权限分级
        </span>
        <span style={{ fontSize: '12px', color: 'var(--text-secondary)' }}>{expanded ? '▼' : '▶'}</span>
      </div>

      {expanded && (
        <div style={{ padding: '12px 20px', display: 'flex', flexDirection: 'column', gap: '10px' }}>
          {loading && <span style={{ fontSize: '12px', color: 'var(--text-secondary)' }}>加载中...</span>}
          {error && <span style={{ fontSize: '12px', color: 'var(--error)' }}>{error}</span>}

          {perms && TIER_INFO.map((tier) => (
            <label
              key={tier.key}
              style={{
                display: 'flex', alignItems: 'center', gap: '10px',
                padding: '8px 10px', background: 'var(--bg-tertiary)',
                borderRadius: '6px', cursor: tier.editable ? 'pointer' : 'default',
                opacity: tier.editable ? 1 : 0.8,
              }}
            >
              <input
                type="checkbox"
                checked={perms[tier.key] as boolean}
                disabled={!tier.editable}
                onChange={() => tier.editable && toggle(tier.key)}
                style={{ accentColor: 'var(--accent)' }}
              />
              <div>
                <div style={{ fontSize: '12px', fontWeight: 500 }}>{tier.label}</div>
                <div style={{ fontSize: '11px', color: 'var(--text-secondary)' }}>{tier.desc}</div>
              </div>
            </label>
          ))}
        </div>
      )}
    </div>
  );
}
