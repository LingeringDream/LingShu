import { useState, useEffect, useCallback } from 'react';
import { apiFetch } from '../../lib/api';

interface Thought {
  id: string;
  user_id: string;
  title: string;
  detail: string | null;
  reason: string | null;
  confidence: number;
  source_memory_ids: string[];
  requires_confirmation: boolean;
  status: string;
  scheduled_at: string | null;
  resolved_at: string | null;
  created_at: string;
  updated_at: string;
}

const STATUS_LABELS: Record<string, string> = {
  pending: '待处理',
  shown: '已展示',
  confirmed: '已确认',
  dismissed: '已忽略',
  expired: '已过期',
};

function getErrorMessage(error: unknown, fallback: string): string {
  return error instanceof Error ? error.message : fallback;
}

export function ThoughtQueue() {
  const [thoughts, setThoughts] = useState<Thought[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [filter, setFilter] = useState('pending');

  const fetchThoughts = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const params = new URLSearchParams({ status: filter, limit: '50' });
      const res = await apiFetch(`/api/v1/thoughts?${params}`);
      if (!res.ok) throw new Error('Failed to fetch thoughts');
      setThoughts(await res.json());
    } catch (e) {
      setError(getErrorMessage(e, 'Failed to load thoughts'));
    } finally {
      setLoading(false);
    }
  }, [filter]);

  useEffect(() => {
    fetchThoughts();
  }, [fetchThoughts]);

  const handleAction = async (id: string, status: string) => {
    setError(null);
    try {
      const res = await apiFetch(`/api/v1/thoughts/${id}`, {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ status }),
      });
      if (!res.ok) throw new Error('Failed to update thought');
      await fetchThoughts();
    } catch (e) {
      setError(getErrorMessage(e, 'Action failed'));
    }
  };

  return (
    <div style={{ padding: 16 }}>
      <h3 style={{ margin: '0 0 16px' }}>思维队列</h3>

      {error && (
        <p style={{ color: '#e05555', fontSize: 13, marginBottom: 12 }}>{error}</p>
      )}

      <div style={{ marginBottom: 16 }}>
        <label style={{ fontSize: 13, marginRight: 8 }}>筛选:</label>
        <select
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          style={{ fontSize: 13, padding: '2px 8px' }}
        >
          <option value="pending">待处理</option>
          <option value="shown">已展示</option>
          <option value="confirmed">已确认</option>
          <option value="dismissed">已忽略</option>
          <option value="expired">已过期</option>
        </select>
      </div>

      {loading ? (
        <p style={{ fontSize: 13 }}>加载中...</p>
      ) : thoughts.length === 0 ? (
        <p style={{ fontSize: 13, color: 'var(--color-secondary-text, #888)' }}>
          暂无{STATUS_LABELS[filter] || filter}的思维建议。
        </p>
      ) : (
        <div style={{ maxHeight: 400, overflowY: 'auto' }}>
          {thoughts.map((t) => (
            <div
              key={t.id}
              style={{
                padding: '10px 14px',
                marginBottom: 10,
                border: '1px solid var(--color-border, #ddd)',
                borderRadius: 6,
                fontSize: 13,
              }}
            >
              <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 4 }}>
                <strong>{t.title}</strong>
                <span
                  style={{
                    padding: '1px 8px',
                    borderRadius: 10,
                    fontSize: 11,
                    background: t.status === 'pending' ? '#fff3cd' : '#e8e8e8',
                  }}
                >
                  {STATUS_LABELS[t.status] || t.status}
                </span>
              </div>
              {t.detail && (
                <p style={{ margin: '4px 0', color: 'var(--color-secondary-text, #666)' }}>
                  {t.detail}
                </p>
              )}
              <div style={{ display: 'flex', gap: 16, fontSize: 12, color: '#888', marginTop: 4 }}>
                <span>置信度: {(t.confidence * 100).toFixed(0)}%</span>
                {t.requires_confirmation && <span>需确认</span>}
              </div>
              {t.reason && (
                <p style={{ margin: '4px 0', fontSize: 12, fontStyle: 'italic', color: '#999' }}>
                  原因: {t.reason}
                </p>
              )}
              {t.status === 'pending' && (
                <div style={{ marginTop: 8, display: 'flex', gap: 8 }}>
                  <button
                    type="button"
                    onClick={() => handleAction(t.id, 'confirmed')}
                    style={{
                      padding: '3px 12px',
                      fontSize: 12,
                      background: 'var(--color-accent, #0a73ff)',
                      color: '#fff',
                      border: 'none',
                      borderRadius: 4,
                      cursor: 'pointer',
                    }}
                  >
                    确认
                  </button>
                  <button
                    type="button"
                    onClick={() => handleAction(t.id, 'dismissed')}
                    style={{
                      padding: '3px 12px',
                      fontSize: 12,
                      border: '1px solid var(--color-border, #ddd)',
                      borderRadius: 4,
                      background: 'transparent',
                      cursor: 'pointer',
                    }}
                  >
                    忽略
                  </button>
                </div>
              )}
              {t.resolved_at && (
                <p style={{ fontSize: 11, color: '#999', marginTop: 4 }}>
                  处理于: {new Date(t.resolved_at).toLocaleString('zh-CN')}
                </p>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
