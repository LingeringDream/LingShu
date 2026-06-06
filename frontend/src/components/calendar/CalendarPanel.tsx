import { useState, useEffect } from 'react';
import { apiFetch } from '../../lib/api';
import { isTauri } from '../../lib/tauri';
import { syncEventToAppleCalendar } from '../../lib/eventkit';

interface CalendarEvent {
  id: string;
  title: string;
  description?: string;
  location?: string;
  start_time: string;
  end_time: string;
  status: string;
  calendar_name: string;
  parse_confidence?: number;
}

export function CalendarPanel() {
  const [text, setText] = useState('');
  const [parsing, setParsing] = useState(false);
  const [events, setEvents] = useState<CalendarEvent[]>([]);
  const [feedback, setFeedback] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [expanded, setExpanded] = useState(true);
  const [syncing, setSyncing] = useState<Set<string>>(new Set());
  const [syncResults, setSyncResults] = useState<Record<string, string>>({});
  const [inTauri, setInTauri] = useState(false);

  useEffect(() => {
    fetchEvents();
    setInTauri(isTauri());
  }, []);

  const fetchEvents = async () => {
    try {
      const resp = await apiFetch('/api/v1/calendar/events?limit=20');
      if (resp.ok) setEvents(await resp.json());
    } catch { /* silent */ }
  };

  const handleParse = async () => {
    if (!text.trim()) return;
    setParsing(true);
    setError(null);
    setFeedback(null);
    try {
      const resp = await apiFetch('/api/v1/calendar/parse', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ text }),
      });
      if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
      const event: CalendarEvent = await resp.json();
      setText('');
      setFeedback(
        event.status === 'pending_confirmation'
          ? `「${event.title}」已保存，待确认`
          : `「${event.title}」已创建`
      );
      fetchEvents();
    } catch (e) {
      setError(e instanceof Error ? e.message : '解析失败');
    } finally {
      setParsing(false);
    }
  };

  const handleConfirm = async (event: CalendarEvent) => {
    try {
      setError(null);
      const resp = await apiFetch(`/api/v1/calendar/events/${event.id}/confirm`, {
        method: 'POST',
      });
      if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
      const confirmed: CalendarEvent = await resp.json();
      setFeedback(`「${event.title}」已确认`);

      // If running in Tauri, auto-sync to Apple Calendar
      if (inTauri) {
        await handleSyncToApple(confirmed);
      } else {
        fetchEvents();
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : '确认失败');
    }
  };

  const handleSyncToApple = async (event: CalendarEvent) => {
    const eid = event.id;
    setSyncing((prev) => new Set(prev).add(eid));
    try {
      setError(null);
      const appleId = await syncEventToAppleCalendar(eid, {
        title: event.title,
        start_time: event.start_time,
        end_time: event.end_time,
        location: event.location,
        notes: event.description,
        calendar_name: event.calendar_name,
      });
      if (appleId) {
        setSyncResults((prev) => ({ ...prev, [eid]: appleId }));
        setFeedback(`「${event.title}」已同步到 Apple Calendar`);
      } else if (!inTauri) {
        setFeedback(`「${event.title}」已确认（浏览器模式，跳过 Apple Calendar 同步）`);
      } else {
        setError('Apple Calendar 同步失败，请检查日历权限');
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : '同步失败');
    } finally {
      setSyncing((prev) => {
        const next = new Set(prev);
        next.delete(eid);
        return next;
      });
      fetchEvents();
    }
  };

  const formatTime = (iso: string) =>
    new Date(iso).toLocaleString('zh-CN', {
      month: 'short', day: 'numeric',
      hour: '2-digit', minute: '2-digit',
    });

  const statusLabel = (status: string) => {
    switch (status) {
      case 'pending_confirmation': return '⏳ 待确认';
      case 'confirmed': return '✅ 已确认';
      case 'cancelled': return '❌ 已取消';
      default: return status;
    }
  };

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%' }}>
      <div
        style={{
          padding: '12px 20px', borderBottom: '1px solid var(--border)',
          display: 'flex', justifyContent: 'space-between', alignItems: 'center',
          cursor: 'pointer',
        }}
        onClick={() => setExpanded(!expanded)}
      >
        <span style={{ fontSize: '13px', fontWeight: 500, color: 'var(--text-secondary)' }}>
          日历 ({events.length})
          {inTauri && <span style={{ fontSize: 10, color: 'var(--accent)', marginLeft: 8 }}>Apple Calendar 已连接</span>}
          {!inTauri && <span style={{ fontSize: 10, color: '#888', marginLeft: 8 }}>浏览器模式</span>}
        </span>
        <span style={{ fontSize: '12px', color: 'var(--text-secondary)' }}>{expanded ? '▼' : '▶'}</span>
      </div>

      {expanded && (
        <div style={{ padding: '12px 20px', display: 'flex', flexDirection: 'column', gap: '10px', flex: 1, overflow: 'auto' }}>
          {/* Natural language input */}
          <input
            placeholder="试试：明天下午3点和张三开会1小时"
            value={text}
            onChange={(e) => setText(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && handleParse()}
            disabled={parsing}
            style={{
              background: 'var(--bg-tertiary)', border: '1px solid var(--border)',
              borderRadius: '6px', padding: '8px 10px', color: 'var(--text-primary)',
              fontSize: '13px', outline: 'none',
            }}
          />
          <button
            onClick={handleParse}
            disabled={parsing || !text.trim()}
            style={{
              background: 'var(--accent)', border: 'none', borderRadius: '6px',
              padding: '8px', color: '#fff', fontSize: '13px', cursor: 'pointer',
              opacity: parsing ? 0.6 : 1,
            }}
          >
            {parsing ? '解析中...' : '解析日程'}
          </button>
          {error && <span style={{ fontSize: '12px', color: 'var(--error)' }}>{error}</span>}
          {feedback && (
            <span style={{ fontSize: '12px', color: 'var(--text-secondary)' }}>{feedback}</span>
          )}

          {/* Event list */}
          {events.map((e) => {
            const alreadySynced = Boolean(syncResults[e.id]);
            return (
              <div key={e.id} style={{
                background: 'var(--bg-tertiary)', borderRadius: '6px',
                padding: '8px 10px', fontSize: '12px',
                borderLeft: `3px solid ${e.status === 'confirmed' ? '#4caf50' : 'var(--accent)'}`,
                display: 'flex', flexDirection: 'column', gap: 4,
              }}>
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                  <span style={{ fontWeight: 500 }}>{e.title}</span>
                  <span style={{ fontSize: 10, color: 'var(--text-secondary)' }}>{statusLabel(e.status)}</span>
                </div>
                <div style={{ color: 'var(--text-secondary)' }}>
                  {formatTime(e.start_time)} → {formatTime(e.end_time)}
                </div>
                {e.location && <div style={{ color: 'var(--text-secondary)', fontSize: 11 }}>📍 {e.location}</div>}

                {/* Action buttons */}
                <div style={{ display: 'flex', gap: 6, marginTop: 2 }}>
                  {e.status === 'pending_confirmation' && (
                    <button
                      onClick={() => handleConfirm(e)}
                      style={{
                        padding: '2px 10px', fontSize: 11, border: '1px solid var(--accent)',
                        borderRadius: 4, background: 'transparent', color: 'var(--accent)',
                        cursor: 'pointer',
                      }}
                    >
                      {inTauri ? '确认并同步' : '确认'}
                    </button>
                  )}
                  {e.status === 'confirmed' && inTauri && !alreadySynced && (
                    <button
                      onClick={() => handleSyncToApple(e)}
                      disabled={syncing.has(e.id)}
                      style={{
                        padding: '2px 10px', fontSize: 11, border: '1px solid #4caf50',
                        borderRadius: 4, background: 'transparent', color: '#4caf50',
                        cursor: syncing.has(e.id) ? 'not-allowed' : 'pointer',
                        opacity: syncing.has(e.id) ? 0.6 : 1,
                      }}
                    >
                      {syncing.has(e.id) ? '同步中...' : '同步到 Apple Calendar'}
                    </button>
                  )}
                  {alreadySynced && (
                    <span style={{ fontSize: 10, color: '#4caf50', alignSelf: 'center' }}>
                      ✓ 已同步
                    </span>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
