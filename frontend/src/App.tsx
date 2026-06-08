import { useState, useEffect, useCallback } from 'react';
import { AppLayout, type AppSectionKey } from './components/layout/AppLayout';
import { ChatWindow } from './components/chat/ChatWindow';
import { ChatSettings } from './components/chat/ChatSettings';
import { MemoryCenter } from './components/memory/MemoryCenter';
import { CalendarPanel } from './components/calendar/CalendarPanel';
import { PermissionSettings } from './components/settings/PermissionSettings';
import { PersonalityCenter } from './components/personality/PersonalityCenter';
import { ThoughtQueue } from './components/thoughts/ThoughtQueue';
import { ProjectManager } from './components/projects/ProjectManager';
import {
  type AvatarControlSettings,
  AvatarControlPanel,
} from './components/avatar/AvatarControlPanel';
import { ensureLocalSession, apiFetch } from './lib/api';
import { useProjectStore } from './stores/projectStore';

// ── Dashboard data types ────────────────────────────────────────

interface CalendarEvent {
  id: string;
  title: string;
  start_time: string;
  status: string;
}

interface TaskItem {
  id: string;
  project_id: string;
  title: string;
  status: string;
  priority: number;
  due_date: string | null;
  assignee_id: string | null;
}

interface ConversationItem {
  id: string;
  title: string | null;
  created_at: string;
}

interface MemoryItem {
  id: string;
  memory_type: string;
  content: string;
  importance: number;
}

function getErrorMessage(error: unknown, fallback: string): string {
  return error instanceof Error ? error.message : fallback;
}

export default function App() {
  const [sessionState, setSessionState] = useState<'loading' | 'ready' | 'error'>('loading');
  const [sessionError, setSessionError] = useState<string | null>(null);
  const [activeSection, setActiveSection] = useState<AppSectionKey>('home');
  const [avatarSettings, setAvatarSettings] = useState<AvatarControlSettings>({
    visible: true,
    mood: 'idle',
    size: 'medium',
    bubbleText: '我在这里，需要时叫我。',
  });

  // Dashboard state
  const [scheduleItems, setScheduleItems] = useState<{ id: string; time: string; title: string; state: string }[]>([]);
  const [todoItems, setTodoItems] = useState<{ id: string; title: string; tag?: string }[]>([]);
  const [recentConversations, setRecentConversations] = useState<{ id: string; title: string; time: string }[]>([]);
  const [memoryBullets, setMemoryBullets] = useState<string[]>([]);
  const [dashboardLoading, setDashboardLoading] = useState(false);

  // Stats
  const [stats, setStats] = useState({ events: 0, tasks: 0, pendingTasks: 0 });
  const [l1CalendarEnabled, setL1CalendarEnabled] = useState(false);

  const { fetchProjects } = useProjectStore();

  const bootLocalSession = useCallback(async () => {
    setSessionState('loading');
    setSessionError(null);
    try {
      await ensureLocalSession(true);
      setSessionState('ready');
    } catch (e) {
      setSessionError(getErrorMessage(e, '本地会话启动失败'));
      setSessionState('error');
    }
  }, []);

  useEffect(() => {
    bootLocalSession();
  }, [bootLocalSession]);

  // ── Fetch dashboard data ──────────────────────────────────────

  const loadDashboard = useCallback(async () => {
    setDashboardLoading(true);
    try {
      // Gate calendar on L1 permission to avoid 403 noise
      let l1Calendar = false;
      try {
        const permRes = await apiFetch('/api/v1/permissions');
        if (permRes.ok) {
          const perms: { l1_calendar: boolean } = await permRes.json();
          l1Calendar = perms.l1_calendar ?? false;
        }
      } catch {
        // permissions fetch is best-effort; default to disabled
      }
      setL1CalendarEnabled(l1Calendar);

      // Calendar events — only when L1 calendar is enabled
      if (l1Calendar) {
        const calRes = await apiFetch('/api/v1/calendar/events?limit=5');
        if (calRes.ok) {
          const events: CalendarEvent[] = await calRes.json();
          setScheduleItems(
            events.map((e) => ({
              id: e.id,
              time: new Date(e.start_time).toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' }),
              title: e.title,
              state: e.status === 'confirmed' ? 'active' : 'muted',
            }))
          );
          setStats((s) => ({ ...s, events: events.length }));
        }
      } else {
        setScheduleItems([]);
        setStats((s) => ({ ...s, events: 0 }));
      }

      // Conversations
      const convRes = await apiFetch('/api/v1/conversations');
      if (convRes.ok) {
        const convs: ConversationItem[] = await convRes.json();
        setRecentConversations(
          convs.slice(0, 5).map((c) => ({
            id: c.id,
            title: c.title || '无标题对话',
            time: new Date(c.created_at).toLocaleDateString('zh-CN'),
          }))
        );
      }

      // Memories
      const memRes = await apiFetch('/api/v1/memories?limit=5');
      if (memRes.ok) {
        const mems: MemoryItem[] = await memRes.json();
        setMemoryBullets(mems.map((m) => m.content));
      }

      // Tasks (via first project)
      await fetchProjects();
      const projRes = await apiFetch('/api/v1/projects');
      if (projRes.ok) {
        const projs: { id: string }[] = await projRes.json();
        if (projs.length > 0) {
          const tasksRes = await apiFetch(`/api/v1/projects/${projs[0].id}/tasks?limit=5`);
          if (tasksRes.ok) {
            const tasks: TaskItem[] = await tasksRes.json();
            setTodoItems(
              tasks.map((t) => ({
                id: t.id,
                title: t.title,
                tag: t.priority <= 2 ? '高优先级' : undefined,
              }))
            );
            setStats((s) => ({
              ...s,
              tasks: tasks.length,
              pendingTasks: tasks.filter((t) => t.status !== 'done' && t.status !== 'completed').length,
            }));
          }
        }
      }
    } catch {
      // Dashboard is best-effort; errors are non-fatal
    } finally {
      setDashboardLoading(false);
    }
  }, [fetchProjects]);

  useEffect(() => {
    if (sessionState === 'ready' && activeSection === 'home') {
      loadDashboard();
    }
  }, [sessionState, activeSection, loadDashboard]);

  // ── Local session gate ────────────────────────────────────────

  if (sessionState !== 'ready') {
    return (
      <AppLayout>
        <div className="workspace dashboard-workspace">
          <section className="dashboard-card local-session-card">
            <div className="card-title-row">
              <h3>{sessionState === 'loading' ? '正在打开本地控制台' : '本地控制台启动失败'}</h3>
            </div>
            {sessionState === 'loading' ? (
              <p>正在连接本地服务...</p>
            ) : (
              <>
                <p>{sessionError}</p>
                <button type="button" onClick={bootLocalSession}>
                  重试
                </button>
              </>
            )}
          </section>
        </div>
      </AppLayout>
    );
  }

  const dashboardDate = new Date().toLocaleDateString('zh-CN', {
    year: 'numeric',
    month: 'long',
    day: 'numeric',
    weekday: 'long',
  });

  // ── Section renderers ─────────────────────────────────────────

  const renderHome = () => (
    <>
      <section className="dashboard-hero">
        <div>
          <p className="eyebrow">Overview</p>
          <h2>你好，用户！</h2>
          <p>{dashboardDate}</p>
        </div>
        <div className="hero-hint">
          <span className="status-dot" />
          桌面助理在线，等待你的下一步指令
        </div>
      </section>

      <section className="stat-grid" aria-label="今日状态">
        <button
          type="button"
          className="stat-card stat-action-card"
          onClick={() => setActiveSection('calendar')}
        >
          <span className="stat-icon stat-blue" />
          <div>
            <p>今日日程</p>
            <strong>{stats.events} 项</strong>
            <span>{stats.events > 0 ? `下一个：${scheduleItems[0]?.time || ''} ${scheduleItems[0]?.title || ''}` : '暂无日程'}</span>
          </div>
        </button>
        <button
          type="button"
          className="stat-card stat-action-card"
          onClick={() => setActiveSection('tasks')}
        >
          <span className="stat-icon stat-orange" />
          <div>
            <p>待办事项</p>
            <strong>{stats.tasks} 项</strong>
            <span>{stats.pendingTasks} 项待完成</span>
          </div>
        </button>
        <button
          type="button"
          className="stat-card stat-action-card"
          onClick={() => setActiveSection('thoughts')}
        >
          <span className="stat-icon stat-green" />
          <div>
            <p>思维建议</p>
            <strong>查看</strong>
            <span>助理主动建议</span>
          </div>
        </button>
      </section>

      <section className="dashboard-grid">
        <article className="dashboard-card schedule-card">
          <div className="card-title-row">
            <h3>今日日程</h3>
            <button type="button" onClick={() => setActiveSection('calendar')}>
              查看全部
            </button>
          </div>
          {dashboardLoading ? (
            <p style={{ padding: 12, fontSize: 13 }}>加载中...</p>
          ) : !l1CalendarEnabled ? (
            <div style={{ padding: 12, fontSize: 13 }}>
              <p style={{ color: '#888', margin: '0 0 8px 0' }}>需要开启 L1 日历权限</p>
              <button
                type="button"
                onClick={() => setActiveSection('privacy')}
                style={{
                  padding: '4px 12px',
                  fontSize: 12,
                  border: '1px solid var(--accent)',
                  borderRadius: 4,
                  background: 'transparent',
                  color: 'var(--accent)',
                  cursor: 'pointer',
                }}
              >
                前往权限设置
              </button>
            </div>
          ) : scheduleItems.length === 0 ? (
            <p style={{ padding: 12, fontSize: 13, color: '#888' }}>暂无日程。去日历添加吧。</p>
          ) : (
            <div className="timeline-list">
              {scheduleItems.map((item) => (
                <div key={item.id} className="timeline-item">
                  <span className={`timeline-dot ${item.state}`} />
                  <time>{item.time}</time>
                  <span>{item.title}</span>
                </div>
              ))}
            </div>
          )}
        </article>

        <article className="dashboard-card todo-card">
          <div className="card-title-row">
            <h3>我的待办</h3>
            <button type="button" onClick={() => setActiveSection('tasks')}>
              新建待办
            </button>
          </div>
          {todoItems.length === 0 ? (
            <p style={{ padding: 12, fontSize: 13, color: '#888' }}>
              暂无任务。先创建一个项目再添加任务吧。
            </p>
          ) : (
            <div className="todo-list">
              {todoItems.map((item) => (
                <label key={item.id} className="todo-row">
                  <input type="checkbox" />
                  <span>{item.title}</span>
                  {item.tag && <em>{item.tag}</em>}
                </label>
              ))}
            </div>
          )}
        </article>

        <article className="dashboard-card conversation-card">
          <div className="card-title-row">
            <h3>最近对话 / 助理建议</h3>
            <button type="button" onClick={() => setActiveSection('chat')}>
              查看全部对话
            </button>
          </div>
          {recentConversations.length === 0 ? (
            <p style={{ padding: 12, fontSize: 13, color: '#888' }}>暂无对话。</p>
          ) : (
            <div className="conversation-list">
              {recentConversations.map((item) => (
                <div key={item.id} className="conversation-row">
                  <span className="conversation-avatar" />
                  <span>{item.title}</span>
                  <time>{item.time}</time>
                </div>
              ))}
            </div>
          )}
        </article>

        <article className="dashboard-card memory-card">
          <div className="card-title-row">
            <h3>记忆摘要</h3>
            <button type="button" onClick={() => setActiveSection('memory')}>
              查看全部记忆
            </button>
          </div>
          {memoryBullets.length === 0 ? (
            <p style={{ padding: 12, fontSize: 13, color: '#888' }}>暂无记忆。</p>
          ) : (
            <ul className="memory-bullets">
              {memoryBullets.map((b, i) => (
                <li key={i}>{b}</li>
              ))}
            </ul>
          )}
        </article>
      </section>
    </>
  );

  const renderActivePanel = () => {
    switch (activeSection) {
      case 'home':
        return renderHome();
      case 'chat':
        return (
          <section className="dashboard-card chat-console-card page-tool-card">
            <div className="card-title-row">
              <h3>聊天控制台</h3>
            </div>
            <ChatWindow />
          </section>
        );
      case 'calendar':
        return (
          <section className="dashboard-card live-tool-card page-tool-card">
            <div className="card-title-row">
              <h3>日历解析与 Apple Calendar 同步</h3>
            </div>
            <CalendarPanel />
          </section>
        );
      case 'memory':
        return (
          <section className="dashboard-card live-tool-card page-tool-card">
            <div className="card-title-row">
              <h3>记忆中心</h3>
            </div>
            <MemoryCenter />
          </section>
        );
      case 'tasks':
        return <TasksPage onNavigate={setActiveSection} />;
      case 'projects':
        return (
          <section className="dashboard-card page-tool-card">
            <ProjectManager />
          </section>
        );
      case 'personality':
        return (
          <section className="dashboard-card page-tool-card narrow-tool-card">
            <PersonalityCenter />
          </section>
        );
      case 'thoughts':
        return (
          <section className="dashboard-card page-tool-card narrow-tool-card">
            <ThoughtQueue />
          </section>
        );
      case 'preferences':
        return (
          <section className="dashboard-card page-tool-card narrow-tool-card">
            <AvatarControlPanel settings={avatarSettings} onChange={setAvatarSettings} />
          </section>
        );
      case 'privacy':
        return (
          <section className="dashboard-card page-tool-card narrow-tool-card">
            <PermissionSettings />
          </section>
        );
      case 'settings':
        return (
          <section className="dashboard-card page-tool-card narrow-tool-card">
            <ChatSettings collapsed={false} />
          </section>
        );
      default:
        return renderHome();
    }
  };

  return (
    <AppLayout activeSection={activeSection} onNavigate={setActiveSection}>
      <div className="workspace dashboard-workspace">
        <div id={`section-${activeSection}`} className="dashboard-scroll">
          {renderActivePanel()}
        </div>
      </div>
    </AppLayout>
  );
}

// ── Tasks Page (real API) ───────────────────────────────────────

interface TasksPageProps {
  onNavigate: (section: AppSectionKey) => void;
}

function TasksPage({ onNavigate }: TasksPageProps) {
  const { projects, fetchProjects } = useProjectStore();
  const [selectedPid, setSelectedPid] = useState<string | null>(null);
  const [taskList, setTaskList] = useState<TaskItem[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [showCreate, setShowCreate] = useState(false);
  const [newTitle, setNewTitle] = useState('');
  const [newPriority, setNewPriority] = useState(3);

  useEffect(() => {
    fetchProjects();
  }, [fetchProjects]);

  useEffect(() => {
    if (projects.length > 0 && !selectedPid) {
      setSelectedPid(projects[0].id);
    }
  }, [projects, selectedPid]);

  const loadTasks = useCallback(async () => {
    if (!selectedPid) return;
    setLoading(true);
    setError(null);
    try {
      const res = await apiFetch(`/api/v1/projects/${selectedPid}/tasks?limit=50`);
      if (!res.ok) throw new Error('Failed to load tasks');
      setTaskList(await res.json());
    } catch (e) {
      setError(getErrorMessage(e, 'Load failed'));
    } finally {
      setLoading(false);
    }
  }, [selectedPid]);

  useEffect(() => {
    loadTasks();
  }, [loadTasks]);

  const handleCreate = async () => {
    if (!newTitle.trim() || !selectedPid) return;
    try {
      const res = await apiFetch(`/api/v1/projects/${selectedPid}/tasks`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ title: newTitle.trim(), priority: newPriority }),
      });
      if (!res.ok) throw new Error('Failed to create task');
      setNewTitle('');
      setShowCreate(false);
      await loadTasks();
    } catch (e) {
      setError(getErrorMessage(e, 'Create failed'));
    }
  };

  const handleDelete = async (tid: string) => {
    if (!selectedPid) return;
    try {
      await apiFetch(`/api/v1/projects/${selectedPid}/tasks/${tid}`, { method: 'DELETE' });
      await loadTasks();
    } catch (e) {
      setError(getErrorMessage(e, 'Delete failed'));
    }
  };

  const toggleStatus = async (t: TaskItem) => {
    if (!selectedPid) return;
    const newStatus = t.status === 'done' ? 'todo' : 'done';
    try {
      const res = await apiFetch(`/api/v1/projects/${selectedPid}/tasks/${t.id}`, {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ status: newStatus }),
      });
      if (!res.ok) throw new Error('Update failed');
      await loadTasks();
    } catch (e) {
      setError(getErrorMessage(e, 'Update failed'));
    }
  };

  return (
    <section className="dashboard-card page-tool-card">
      <div className="card-title-row">
        <h3>我的任务</h3>
        <button type="button" onClick={() => setShowCreate(true)}>新建任务</button>
      </div>

      {/* Project selector */}
      {projects.length > 1 && (
        <div style={{ marginBottom: 12 }}>
          <select
            value={selectedPid || ''}
            onChange={(e) => setSelectedPid(e.target.value)}
            style={{ fontSize: 13, padding: '4px 8px' }}
          >
            {projects.map((p) => (
              <option key={p.id} value={p.id}>{p.name}</option>
            ))}
          </select>
        </div>
      )}

      {error && <p style={{ color: '#e05555', fontSize: 13 }}>{error}</p>}

      {/* Create form */}
      {showCreate && (
        <div style={{ padding: 12, marginBottom: 12, border: '1px solid #0a73ff', borderRadius: 6, background: '#f8faff' }}>
          <input
            placeholder="任务标题"
            value={newTitle}
            onChange={(e) => setNewTitle(e.target.value)}
            style={{ display: 'block', width: '100%', padding: 6, marginBottom: 8, fontSize: 13 }}
          />
          <select value={newPriority} onChange={(e) => setNewPriority(Number(e.target.value))} style={{ marginBottom: 8, fontSize: 13 }}>
            <option value={1}>P1 - 紧急</option>
            <option value={2}>P2 - 高</option>
            <option value={3}>P3 - 中</option>
            <option value={4}>P4 - 低</option>
            <option value={5}>P5 - 极低</option>
          </select>
          <div style={{ display: 'flex', gap: 8 }}>
            <button type="button" onClick={handleCreate} style={{ padding: '4px 14px', background: '#0a73ff', color: '#fff', border: 'none', borderRadius: 4, cursor: 'pointer', fontSize: 13 }}>创建</button>
            <button type="button" onClick={() => setShowCreate(false)} style={{ padding: '4px 14px', border: '1px solid #ddd', borderRadius: 4, background: 'transparent', cursor: 'pointer', fontSize: 13 }}>取消</button>
          </div>
        </div>
      )}

      {/* Task list */}
      {loading ? (
        <p style={{ fontSize: 13 }}>加载中...</p>
      ) : projects.length === 0 ? (
        <p style={{ fontSize: 13, color: '#888' }}>
          暂无项目。请先在 <button type="button" onClick={() => onNavigate('projects')} style={{ color: '#0a73ff', border: 'none', background: 'none', cursor: 'pointer', textDecoration: 'underline' }}>项目管理</button> 中创建项目。
        </p>
      ) : taskList.length === 0 ? (
        <p style={{ fontSize: 13, color: '#888' }}>暂无任务。</p>
      ) : (
        <div className="todo-list spacious-list">
          {taskList.map((t) => (
            <div key={t.id} className="todo-row" style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '6px 0' }}>
              <input
                type="checkbox"
                checked={t.status === 'done'}
                onChange={() => toggleStatus(t)}
              />
              <span style={{ flex: 1, textDecoration: t.status === 'done' ? 'line-through' : 'none' }}>
                {t.title}
              </span>
              {t.priority <= 2 && <em>高优先级</em>}
              <button
                type="button"
                onClick={() => handleDelete(t.id)}
                style={{ padding: '1px 6px', fontSize: 11, color: '#c55', border: '1px solid #ecc', borderRadius: 3, background: 'transparent', cursor: 'pointer' }}
              >
                删除
              </button>
            </div>
          ))}
        </div>
      )}
    </section>
  );
}
