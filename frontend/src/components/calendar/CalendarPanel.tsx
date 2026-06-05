import { useState, useEffect } from 'react';
import { apiFetch } from '../../lib/api';

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

  useEffect(() => {
    fetchEvents();
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

  const formatTime = (iso: string) =>
    new Date(iso).toLocaleString('zh-CN', {
      month: 'short', day: 'numeric',
      hour: '2-digit', minute: '2-digit',
    });

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
          {events.map((e) => (
            <div key={e.id} style={{
              background: 'var(--bg-tertiary)', borderRadius: '6px',
              padding: '8px 10px', fontSize: '12px', borderLeft: '3px solid var(--accent)',
            }}>
              <div style={{ fontWeight: 500 }}>{e.title}</div>
              <div style={{ color: 'var(--text-secondary)' }}>
                {formatTime(e.start_time)} → {formatTime(e.end_time)}
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
