import { useState, useEffect, useCallback } from 'react';
import { apiFetch } from '../../lib/api';
import { useProjectStore, type Project } from '../../stores/projectStore';
import { isTauri } from '../../lib/tauri';
import { syncEventToAppleCalendar } from '../../lib/eventkit';

// ── Types ────────────────────────────────────────────────────────────

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
  external_event_id?: string;
}

interface TaskItem {
  id: string;
  title: string;
  status: string;
  priority: number;
  description?: string;
}

// ── Helpers ──────────────────────────────────────────────────────────

function fmtTime(iso: string): string {
  return new Date(iso).toLocaleString('zh-CN', {
    month: '2-digit', day: '2-digit',
    hour: '2-digit', minute: '2-digit',
  });
}

function getErrorMessage(e: unknown, fallback: string): string {
  return e instanceof Error ? e.message : fallback;
}

// ── Section: Calendar ────────────────────────────────────────────────

function CalendarSection() {
  const [text, setText] = useState('');
  const [parsing, setParsing] = useState(false);
  const [events, setEvents] = useState<CalendarEvent[]>([]);
  const [feedback, setFeedback] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [syncing, setSyncing] = useState<Set<string>>(new Set());
  const [syncResults, setSyncResults] = useState<Record<string, string>>({});
  const [inTauri, setInTauri] = useState(false);
  const [collapsed, setCollapsed] = useState(false);

  const fetchEvents = useCallback(async () => {
    try {
      const resp = await apiFetch('/api/v1/calendar/events?limit=20');
      if (resp.ok) setEvents(await resp.json());
    } catch { /* silent */ }
  }, []);

  useEffect(() => { fetchEvents(); setInTauri(isTauri()); }, [fetchEvents]);

  const handleParse = async () => {
    if (!text.trim()) return;
    setParsing(true); setError(null); setFeedback(null);
    try {
      const resp = await apiFetch('/api/v1/calendar/parse', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ text }),
      });
      if (!resp.ok) {
        const err = await resp.json().catch(() => ({}));
        throw new Error((err as { error?: { message?: string } }).error?.message ?? `HTTP ${resp.status}`);
      }
      const ev: CalendarEvent = await resp.json();
      setFeedback(`已创建：${ev.title}（${fmtTime(ev.start_time)} — ${fmtTime(ev.end_time)}）`);
      setText('');
      await fetchEvents();
    } catch (e) {
      setError(getErrorMessage(e, '解析失败'));
    } finally {
      setParsing(false);
    }
  };

  const handleConfirm = async (ev: CalendarEvent) => {
    try {
      const resp = await apiFetch(`/api/v1/calendar/events/${ev.id}/confirm`, { method: 'POST' });
      if (resp.ok) await fetchEvents();
    } catch { /* silent */ }
  };

  const handleDeleteEvent = async (ev: CalendarEvent) => {
    if (!window.confirm(`确定要删除「${ev.title}」吗？`)) return;
    try {
      const resp = await apiFetch(`/api/v1/calendar/events/${ev.id}`, { method: 'DELETE' });
      if (resp.ok) await fetchEvents();
    } catch { /* silent */ }
  };

  const handleSync = async (ev: CalendarEvent) => {
    setSyncing((s) => new Set(s).add(ev.id));
    try {
      await syncEventToAppleCalendar(ev.id, {
        title: ev.title,
        start_time: ev.start_time,
        end_time: ev.end_time,
        location: ev.location,
        notes: ev.description,
      });
      setSyncResults((r) => ({ ...r, [ev.id]: '已同步到系统日历' }));
    } catch (e) {
      setSyncResults((r) => ({ ...r, [ev.id]: getErrorMessage(e, '同步失败') }));
    } finally {
      setSyncing((s) => { const n = new Set(s); n.delete(ev.id); return n; });
    }
  };

  return (
    <section style={{ marginBottom: 24 }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 12 }}>
        <h4 style={{ margin: 0 }}>日历</h4>
        <button type="button" onClick={() => setCollapsed(!collapsed)} style={{ background: 'none', border: 'none', cursor: 'pointer', fontSize: 12, color: 'var(--color-secondary-text, #888)' }}>
          {collapsed ? '展开' : '收起'}
        </button>
      </div>

      {!collapsed && (
        <>
          {/* Parse input */}
          <div style={{ display: 'flex', gap: 8, marginBottom: 12 }}>
            <input
              value={text}
              onChange={(e) => setText(e.target.value)}
              onKeyDown={(e) => { if (e.key === 'Enter') handleParse(); }}
              placeholder="输入自然语言日程，如「明天下午3点和张三开会」"
              disabled={parsing}
              style={{
                flex: 1, padding: '8px 10px', fontSize: 13,
                background: 'var(--bg-tertiary, #f5f5f5)',
                border: '1px solid var(--color-border, #ddd)',
                borderRadius: 6, color: 'var(--text-primary, #333)',
                outline: 'none', fontFamily: 'inherit',
              }}
            />
            <button onClick={handleParse} disabled={parsing || !text.trim()}
              style={{
                padding: '8px 16px', fontSize: 13, whiteSpace: 'nowrap',
                background: 'var(--color-accent, #0a73ff)', color: '#fff',
                border: 'none', borderRadius: 6, cursor: 'pointer',
                opacity: parsing ? 0.7 : 1,
              }}
            >{parsing ? '解析中...' : '解析'}</button>
          </div>

          {feedback && <p style={{ fontSize: 13, color: '#4caf50', margin: '0 0 8px' }}>{feedback}</p>}
          {error && <p style={{ fontSize: 13, color: '#e05555', margin: '0 0 8px' }}>{error}</p>}

          {/* Event list */}
          <div style={{ maxHeight: 240, overflowY: 'auto' }}>
            {events.length === 0 ? (
              <p style={{ fontSize: 13, color: 'var(--color-secondary-text, #888)' }}>暂无日程</p>
            ) : (
              events.map((ev) => (
                <div key={ev.id} style={{
                  padding: '8px 12px', marginBottom: 6, fontSize: 13,
                  border: '1px solid var(--color-border, #ddd)', borderRadius: 6,
                  background: ev.status === 'pending_confirmation' ? '#fff8e1' : 'transparent',
                }}>
                  <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                    <strong>{ev.title}</strong>
                    <span style={{ fontSize: 11, color: 'var(--color-secondary-text, #888)' }}>
                      {ev.status === 'pending_confirmation' ? '待确认' : ev.status}
                    </span>
                  </div>
                  <div style={{ color: 'var(--color-secondary-text, #888)', fontSize: 12, marginTop: 2 }}>
                    {fmtTime(ev.start_time)} — {fmtTime(ev.end_time)}
                    {ev.location ? ` · ${ev.location}` : ''}
                  </div>
                  <div style={{ display: 'flex', gap: 4, marginTop: 4 }}>
                    {ev.status === 'pending_confirmation' && (
                      <button
                        onClick={() => handleConfirm(ev)}
                        style={{ padding: '2px 8px', fontSize: 11, cursor: 'pointer', border: '1px solid #4caf50', borderRadius: 3, background: 'transparent', color: '#4caf50' }}
                      >确认</button>
                    )}
                    <button
                      onClick={() => handleDeleteEvent(ev)}
                      style={{ padding: '2px 8px', fontSize: 11, cursor: 'pointer', border: '1px solid #e05555', borderRadius: 3, background: 'transparent', color: '#e05555' }}
                    >删除</button>
                    {inTauri && (
                      <button
                        onClick={() => handleSync(ev)}
                        disabled={syncing.has(ev.id)}
                        style={{ padding: '2px 8px', fontSize: 11, cursor: 'pointer', border: '1px solid var(--color-accent, #0a73ff)', borderRadius: 3, background: 'transparent', color: 'var(--color-accent, #0a73ff)' }}
                      >{syncing.has(ev.id) ? '同步中...' : (syncResults[ev.id] || '同步到系统日历')}</button>
                    )}
                  </div>
                </div>
              ))
            )}
          </div>
        </>
      )}
    </section>
  );
}

// ── Section: Tasks ────────────────────────────────────────────────────

function TasksSection() {
  const { projects, fetchProjects } = useProjectStore();
  const [selectedPid, setSelectedPid] = useState<string | null>(null);
  const [taskList, setTaskList] = useState<TaskItem[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [showCreate, setShowCreate] = useState(false);
  const [newTitle, setNewTitle] = useState('');
  const [newPriority, setNewPriority] = useState(3);
  const [collapsed, setCollapsed] = useState(false);

  useEffect(() => { fetchProjects(); }, [fetchProjects]);

  useEffect(() => {
    if (projects.length > 0 && !selectedPid) setSelectedPid(projects[0].id);
  }, [projects, selectedPid]);

  const loadTasks = useCallback(async () => {
    if (!selectedPid) return;
    setLoading(true); setError(null);
    try {
      const res = await apiFetch(`/api/v1/projects/${selectedPid}/tasks?limit=50`);
      if (!res.ok) throw new Error('Failed to load tasks');
      setTaskList(await res.json());
    } catch (e) { setError(getErrorMessage(e, 'Load failed')); }
    finally { setLoading(false); }
  }, [selectedPid]);

  useEffect(() => { loadTasks(); }, [loadTasks]);

  const handleCreate = async () => {
    if (!newTitle.trim() || !selectedPid) return;
    try {
      const res = await apiFetch(`/api/v1/projects/${selectedPid}/tasks`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ title: newTitle.trim(), priority: newPriority }),
      });
      if (!res.ok) throw new Error('Failed');
      setNewTitle(''); setShowCreate(false);
      await loadTasks();
    } catch (e) { setError(getErrorMessage(e, 'Create failed')); }
  };

  const toggleStatus = async (t: TaskItem) => {
    if (!selectedPid) return;
    const newStatus = t.status === 'done' ? 'todo' : 'done';
    try {
      await apiFetch(`/api/v1/projects/${selectedPid}/tasks/${t.id}`, {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ status: newStatus }),
      });
      await loadTasks();
    } catch { /* silent */ }
  };

  const handleDelete = async (tid: string) => {
    if (!selectedPid) return;
    try { await apiFetch(`/api/v1/projects/${selectedPid}/tasks/${tid}`, { method: 'DELETE' }); await loadTasks(); }
    catch { /* silent */ }
  };

  const doneCount = taskList.filter((t) => t.status === 'done' || t.status === 'completed').length;

  return (
    <section style={{ marginBottom: 24 }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 12 }}>
        <h4 style={{ margin: 0 }}>
          任务 {projects.length > 0 ? `（${taskList.length - doneCount} 待完成 / ${taskList.length} 总计）` : ''}
        </h4>
        <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
          {projects.length > 1 && (
            <select value={selectedPid || ''} onChange={(e) => setSelectedPid(e.target.value)}
              style={{ fontSize: 12, padding: '2px 6px', borderRadius: 4 }}>
              {projects.map((p) => (<option key={p.id} value={p.id}>{p.name}</option>))}
            </select>
          )}
          <button type="button" onClick={() => setShowCreate(true)} style={{ padding: '4px 12px', fontSize: 12, border: '1px solid var(--color-accent, #0a73ff)', borderRadius: 4, background: 'transparent', color: 'var(--color-accent, #0a73ff)', cursor: 'pointer' }}>+ 新建</button>
          <button type="button" onClick={() => setCollapsed(!collapsed)} style={{ background: 'none', border: 'none', cursor: 'pointer', fontSize: 12, color: 'var(--color-secondary-text, #888)' }}>
            {collapsed ? '展开' : '收起'}
          </button>
        </div>
      </div>

      {!collapsed && (
        <>
          {error && <p style={{ color: '#e05555', fontSize: 13 }}>{error}</p>}

          {showCreate && (
            <div style={{ padding: 10, marginBottom: 10, border: '1px solid var(--color-accent, #0a73ff)', borderRadius: 6, background: '#f8faff' }}>
              <input placeholder="任务标题" value={newTitle} onChange={(e) => setNewTitle(e.target.value)}
                style={{ display: 'block', width: '100%', padding: 6, marginBottom: 6, fontSize: 13, border: '1px solid var(--color-border, #ddd)', borderRadius: 4, boxSizing: 'border-box' }} />
              <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
                <select value={newPriority} onChange={(e) => setNewPriority(Number(e.target.value))} style={{ fontSize: 13 }}>
                  <option value={1}>P1 紧急</option><option value={2}>P2 高</option><option value={3}>P3 中</option><option value={4}>P4 低</option>
                </select>
                <button onClick={handleCreate} style={{ padding: '4px 12px', fontSize: 12, background: 'var(--color-accent, #0a73ff)', color: '#fff', border: 'none', borderRadius: 4, cursor: 'pointer' }}>创建</button>
                <button onClick={() => setShowCreate(false)} style={{ padding: '4px 12px', fontSize: 12, background: 'transparent', border: 'none', cursor: 'pointer', color: 'var(--color-secondary-text, #888)' }}>取消</button>
              </div>
            </div>
          )}

          {loading ? <p style={{ fontSize: 13, color: '#888' }}>加载中...</p> :
            projects.length === 0 ? <p style={{ fontSize: 13, color: '#888' }}>暂无项目。请在下方项目管理中创建。</p> :
            taskList.length === 0 ? <p style={{ fontSize: 13, color: '#888' }}>暂无任务。</p> :
            <div style={{ maxHeight: 220, overflowY: 'auto' }}>
              {taskList.map((t) => (
                <div key={t.id} style={{
                  display: 'flex', alignItems: 'center', gap: 8, padding: '6px 10px', marginBottom: 4,
                  border: '1px solid var(--color-border, #ddd)', borderRadius: 6, fontSize: 13,
                  opacity: t.status === 'done' ? 0.6 : 1,
                }}>
                  <input type="checkbox" checked={t.status === 'done' || t.status === 'completed'} onChange={() => toggleStatus(t)} />
                  <span style={{ flex: 1, textDecoration: t.status === 'done' ? 'line-through' : 'none' }}>{t.title}</span>
                  <span style={{ fontSize: 11, color: t.priority <= 2 ? '#e05555' : '#888' }}>P{t.priority}</span>
                  <button onClick={() => handleDelete(t.id)} style={{ background: 'none', border: 'none', cursor: 'pointer', color: '#ccc', fontSize: 14, padding: 0 }}>×</button>
                </div>
              ))}
            </div>
          }
        </>
      )}
    </section>
  );
}

// ── Section: Projects ─────────────────────────────────────────────────

function ProjectsSection() {
  const { projects, loading, error, fetchProjects, createProject, updateProject, deleteProject } = useProjectStore();
  const [showCreate, setShowCreate] = useState(false);
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editName, setEditName] = useState('');
  const [editDesc, setEditDesc] = useState('');
  const [collapsed, setCollapsed] = useState(false);

  useEffect(() => { fetchProjects(); }, [fetchProjects]);

  const handleCreate = async () => {
    if (!name.trim()) return;
    await createProject({ name: name.trim(), description: description.trim() || undefined });
    setName(''); setDescription(''); setShowCreate(false);
  };

  const handleUpdate = async (id: string) => {
    if (!editName.trim()) return;
    await updateProject(id, { name: editName.trim(), description: editDesc.trim() || undefined });
    setEditingId(null);
  };

  const handleDelete = async (id: string) => {
    if (!window.confirm('确定删除此项目及其中所有任务？')) return;
    await deleteProject(id);
  };

  const startEdit = (p: Project) => {
    setEditingId(p.id); setEditName(p.name); setEditDesc(p.description || '');
  };

  return (
    <section style={{ marginBottom: 24 }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 12 }}>
        <h4 style={{ margin: 0 }}>项目</h4>
        <div style={{ display: 'flex', gap: 8 }}>
          <button type="button" onClick={() => setShowCreate(true)} style={{ padding: '4px 12px', fontSize: 12, border: '1px solid var(--color-accent, #0a73ff)', borderRadius: 4, background: 'transparent', color: 'var(--color-accent, #0a73ff)', cursor: 'pointer' }}>+ 新建</button>
          <button type="button" onClick={() => setCollapsed(!collapsed)} style={{ background: 'none', border: 'none', cursor: 'pointer', fontSize: 12, color: 'var(--color-secondary-text, #888)' }}>
            {collapsed ? '展开' : '收起'}
          </button>
        </div>
      </div>

      {!collapsed && (
        <>
          {error && <p style={{ color: '#e05555', fontSize: 13, marginBottom: 8 }}>{error}</p>}

          {showCreate && (
            <div style={{ padding: 10, marginBottom: 10, border: '1px solid var(--color-accent, #0a73ff)', borderRadius: 6, background: '#f8faff' }}>
              <input placeholder="项目名称" value={name} onChange={(e) => setName(e.target.value)}
                style={{ display: 'block', width: '100%', padding: 6, marginBottom: 6, fontSize: 13, border: '1px solid var(--color-border, #ddd)', borderRadius: 4, boxSizing: 'border-box' }} />
              <input placeholder="描述（可选）" value={description} onChange={(e) => setDescription(e.target.value)}
                style={{ display: 'block', width: '100%', padding: 6, marginBottom: 6, fontSize: 13, border: '1px solid var(--color-border, #ddd)', borderRadius: 4, boxSizing: 'border-box' }} />
              <div style={{ display: 'flex', gap: 8 }}>
                <button onClick={handleCreate} style={{ padding: '4px 12px', fontSize: 12, background: 'var(--color-accent, #0a73ff)', color: '#fff', border: 'none', borderRadius: 4, cursor: 'pointer' }}>创建</button>
                <button onClick={() => setShowCreate(false)} style={{ padding: '4px 12px', fontSize: 12, background: 'transparent', border: 'none', cursor: 'pointer', color: '#888' }}>取消</button>
              </div>
            </div>
          )}

          {loading ? <p style={{ fontSize: 13, color: '#888' }}>加载中...</p> :
            projects.length === 0 ? <p style={{ fontSize: 13, color: '#888' }}>暂无项目</p> :
            <div>
              {projects.map((p) => (
                <div key={p.id} style={{ padding: '8px 12px', marginBottom: 6, border: '1px solid var(--color-border, #ddd)', borderRadius: 6, fontSize: 13 }}>
                  {editingId === p.id ? (
                    <div>
                      <input value={editName} onChange={(e) => setEditName(e.target.value)} style={{ display: 'block', width: '100%', padding: 4, marginBottom: 4, fontSize: 13 }} />
                      <input value={editDesc} onChange={(e) => setEditDesc(e.target.value)} style={{ display: 'block', width: '100%', padding: 4, marginBottom: 4, fontSize: 13 }} />
                      <button onClick={() => handleUpdate(p.id)} style={{ marginRight: 8, fontSize: 12 }}>保存</button>
                      <button onClick={() => setEditingId(null)} style={{ fontSize: 12 }}>取消</button>
                    </div>
                  ) : (
                    <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                      <div>
                        <strong>{p.name}</strong>
                        {p.description && <span style={{ color: '#888', marginLeft: 8, fontSize: 12 }}>{p.description}</span>}
                      </div>
                      <div style={{ display: 'flex', gap: 4 }}>
                        <button onClick={() => startEdit(p)} style={{ padding: '2px 6px', fontSize: 11, border: '1px solid #ccc', borderRadius: 3, background: 'transparent', cursor: 'pointer' }}>编辑</button>
                        <button onClick={() => handleDelete(p.id)} style={{ padding: '2px 6px', fontSize: 11, border: '1px solid #e05555', borderRadius: 3, background: 'transparent', color: '#e05555', cursor: 'pointer' }}>删除</button>
                      </div>
                    </div>
                  )}
                </div>
              ))}
            </div>
          }
        </>
      )}
    </section>
  );
}

// ── Main Page ─────────────────────────────────────────────────────────

export function WorkspacePage() {
  return (
    <section className="dashboard-card page-tool-card">
      <div className="card-title-row">
        <h3>工作台</h3>
      </div>
      <div style={{ padding: 16 }}>
        <CalendarSection />
        <hr style={{ border: 'none', borderTop: '1px solid var(--color-border, #ddd)', margin: '0 0 20px' }} />
        <TasksSection />
        <hr style={{ border: 'none', borderTop: '1px solid var(--color-border, #ddd)', margin: '0 0 20px' }} />
        <ProjectsSection />
      </div>
    </section>
  );
}
