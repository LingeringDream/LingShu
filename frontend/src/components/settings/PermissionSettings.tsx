import { useState, useEffect } from 'react';
import { apiFetch } from '../../lib/api';

interface Permissions {
  l0_enabled: boolean;
  l1_calendar: boolean;
  l1_require_confirmation: boolean;
  l2_automation: boolean;
  l2_whitelist_only: boolean;
  l2_whitelist: string[];
  l3_accessibility: boolean;
  l4_autonomous: boolean;
}

const TIER_INFO: { key: keyof Permissions; label: string; desc: string; editable: boolean }[] = [
  { key: 'l0_enabled', label: 'L0 · 聊天与宠物', desc: '桌面宠物、对话、记忆展示', editable: false },
  { key: 'l1_calendar', label: 'L1 · 日历', desc: '创建/修改 Apple Calendar 日程', editable: true },
  { key: 'l1_require_confirmation', label: 'L1 · 日程需确认', desc: '每个日程创建前展示确认卡片', editable: true },
  { key: 'l2_automation', label: 'L2 · 自动化', desc: '让助手打开 App / URL / 文件（白名单内）', editable: true },
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
      .then((data: Permissions) => { setPerms({ ...data, l2_whitelist: data.l2_whitelist ?? [] }); setLoading(false); })
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

  const [newEntry, setNewEntry] = useState('');

  const saveWhitelist = async (next: string[]) => {
    if (!perms) return;
    const prev = perms.l2_whitelist;
    setPerms({ ...perms, l2_whitelist: next });
    try {
      await apiFetch('/api/v1/permissions', {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ l2_whitelist: next }),
      });
    } catch (e) {
      setPerms({ ...perms, l2_whitelist: prev });
      setError(e instanceof Error ? e.message : '保存失败');
    }
  };

  const addEntry = () => {
    const v = newEntry.trim();
    if (!v || !perms) return;
    if (!perms.l2_whitelist.includes(v)) saveWhitelist([...perms.l2_whitelist, v]);
    setNewEntry('');
  };

  const removeEntry = (entry: string) => {
    if (!perms) return;
    saveWhitelist(perms.l2_whitelist.filter((e) => e !== entry));
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

          {perms && perms.l2_automation && (
            <div style={{
              padding: '10px', background: 'var(--bg-tertiary)', borderRadius: '6px',
              display: 'flex', flexDirection: 'column', gap: '8px',
              borderLeft: '2px solid var(--accent)',
            }}>
              <label style={{ display: 'flex', alignItems: 'center', gap: '10px', cursor: 'pointer' }}>
                <input
                  type="checkbox"
                  checked={perms.l2_whitelist_only}
                  onChange={() => toggle('l2_whitelist_only')}
                  style={{ accentColor: 'var(--accent)' }}
                />
                <div style={{ fontSize: '12px', fontWeight: 500 }}>仅白名单内可执行</div>
              </label>
              <div style={{ fontSize: '11px', color: 'var(--text-secondary)' }}>
                允许助手打开的 App 名称或网址前缀（如「计算器」「https://github.com」）。
                {perms.l2_whitelist_only && perms.l2_whitelist.length === 0 &&
                  ' 当前为空 → 不允许任何操作。'}
              </div>

              {perms.l2_whitelist.map((entry) => (
                <div key={entry} style={{
                  display: 'flex', alignItems: 'center', justifyContent: 'space-between',
                  gap: '8px', fontSize: '12px',
                }}>
                  <span style={{ wordBreak: 'break-all' }}>{entry}</span>
                  <button
                    onClick={() => removeEntry(entry)}
                    style={{
                      fontSize: '11px', padding: '2px 8px', cursor: 'pointer',
                      background: 'transparent', color: 'var(--error)',
                      border: '1px solid var(--border)', borderRadius: '4px',
                    }}
                  >移除</button>
                </div>
              ))}

              <div style={{ display: 'flex', gap: '6px' }}>
                <input
                  value={newEntry}
                  onChange={(e) => setNewEntry(e.target.value)}
                  onKeyDown={(e) => { if (e.key === 'Enter') addEntry(); }}
                  placeholder="App 名称或 https:// 前缀"
                  style={{
                    flex: 1, fontSize: '12px', padding: '4px 8px',
                    background: 'var(--bg-secondary)', color: 'var(--text)',
                    border: '1px solid var(--border)', borderRadius: '4px',
                  }}
                />
                <button
                  onClick={addEntry}
                  style={{
                    fontSize: '12px', padding: '4px 12px', cursor: 'pointer',
                    background: 'var(--accent)', color: '#fff',
                    border: 'none', borderRadius: '4px',
                  }}
                >添加</button>
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
