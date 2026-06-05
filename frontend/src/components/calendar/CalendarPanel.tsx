import { useState, useEffect } from 'react';
import { apiFetch } from '../../lib/api';

interface ParsedEvent {
  title: string;
  description?: string;
  location?: string;
  start_time: string;
  end_time: string;
  attendees: string[];
  calendar_name: string;
  confidence: number;
}

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
  const [parsed, setParsed] = useState<ParsedEvent | null>(null);
  const [events, setEvents] = useState<CalendarEvent[]>([]);
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
    setParsed(null);
    try {
      const resp = await apiFetch('/api/v1/calendar/parse', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ text }),
      });
      if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
      const data: ParsedEvent = await resp.json();
      setParsed(data);
    } catch (e) {
      setError(e instanceof Error ? e.message : '解析失败');
    } finally {
      setParsing(false);
    }
  };

  const handleConfirm = async () => {
    if (!parsed) return;
    try {
      const resp = await apiFetch('/api/v1/calendar/events', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          title: parsed.title,
          description: parsed.description,
          location: parsed.location,
          start_time: parsed.start_time,
          end_time: parsed.end_time,
          attendees: parsed.attendees,
          calendar_name: parsed.calendar_name,
          parse_confidence: parsed.confidence,
          source_input: text,
        }),
      });
      if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
      setParsed(null);
      setText('');
      fetchEvents();
    } catch (e) {
      setError(e instanceof Error ? e.message : '创建失败');
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

          {/* Parsed event confirmation card */}
          {parsed && (
            <div style={{
              background: 'var(--bg-tertiary)', border: '1px solid var(--accent)',
              borderRadius: '8px', padding: '12px', fontSize: '13px',
            }}>
              <div style={{ fontWeight: 500, marginBottom: '6px' }}>{parsed.title}</div>
              <div style={{ color: 'var(--text-secondary)', fontSize: '12px' }}>
                时间：{formatTime(parsed.start_time)} → {formatTime(parsed.end_time)}
              </div>
              {parsed.location && <div style={{ fontSize: '12px' }}>地点：{parsed.location}</div>}
              {parsed.attendees.length > 0 && (
                <div style={{ fontSize: '12px' }}>参与人：{parsed.attendees.join(', ')}</div>
              )}
              <div style={{ fontSize: '11px', color: 'var(--text-secondary)', marginTop: '4px' }}>
                置信度: {(parsed.confidence * 100).toFixed(0)}%
              </div>
              <div style={{ display: 'flex', gap: '8px', marginTop: '8px' }}>
                <button onClick={handleConfirm} style={{
                  background: 'var(--success)', border: 'none', borderRadius: '4px',
                  padding: '6px 16px', color: '#fff', fontSize: '12px', cursor: 'pointer',
                }}>确认创建</button>
                <button onClick={() => setParsed(null)} style={{
                  background: 'var(--bg-tertiary)', border: '1px solid var(--border)',
                  borderRadius: '4px', padding: '6px 16px', fontSize: '12px', cursor: 'pointer',
                  color: 'var(--text-secondary)',
                }}>取消</button>
              </div>
            </div>
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
