import { useState, useEffect, useCallback } from 'react';
import { AppLayout, type AppSectionKey } from './components/layout/AppLayout';
import { ChatWindow } from './components/chat/ChatWindow';
import { ChatSettings } from './components/chat/ChatSettings';
import { MemoryCenter } from './components/memory/MemoryCenter';
import { WorkspacePage } from './components/workspace/WorkspacePage';
import { PermissionSettings } from './components/settings/PermissionSettings';
import { PersonalityCenter } from './components/personality/PersonalityCenter';
import { ThoughtQueue } from './components/thoughts/ThoughtQueue';

import {
  type AvatarControlSettings,
  AvatarControlPanel,
} from './components/avatar/AvatarControlPanel';
import {
  loadAvatarControlSettings,
  publishAvatarControlSettings,
} from './components/avatar/avatarControls';
import { ensureLocalSession, apiFetch } from './lib/api';
import { isTauri, MAIN_NAVIGATE_EVENT } from './lib/tauri';
import { installChatSessionSync } from './stores/chatStore';
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
  const [avatarSettings, setAvatarSettings] = useState<AvatarControlSettings>(() => loadAvatarControlSettings());

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
    // The bundled backend sidecar needs a few seconds to boot on launch
    // (connect to Postgres, run migrations, bind :8080). A single attempt that
    // fires the instant the window opens loses the race and strands the UI in
    // the error state even though the backend comes up moments later. Retry
    // with light backoff for up to ~30 s, staying in the 'loading' state so the
    // user sees "connecting" rather than "failed" during a normal cold start.
    const deadlineMs = Date.now() + 30_000;
    let lastError: unknown;
    for (let attempt = 0; ; attempt++) {
      try {
        await ensureLocalSession(true);
        setSessionState('ready');
        return;
      } catch (e) {
        lastError = e;
        if (Date.now() >= deadlineMs) break;
        await new Promise((resolve) =>
          setTimeout(resolve, Math.min(500 + attempt * 250, 2000)),
        );
      }
    }
    setSessionError(getErrorMessage(lastError, '本地会话启动失败'));
    setSessionState('error');
  }, []);

  useEffect(() => {
    bootLocalSession();
  }, [bootLocalSession]);

  useEffect(() => installChatSessionSync(), []);

  const updateAvatarSettings = useCallback((settings: AvatarControlSettings) => {
    setAvatarSettings(settings);
    publishAvatarControlSettings(settings).catch((error) => {
      console.error('[avatar] failed to publish control settings:', error);
    });
  }, []);

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

  // Deep-link from the pet window: when the user expands a reply (or opens the
  // console), jump straight to the requested section (e.g. 'chat') instead of
  // landing on whatever tab the main window last showed.
  useEffect(() => {
    if (!isTauri()) return;
    let unlisten: (() => void) | null = null;
    import('@tauri-apps/api/event')
      .then(({ listen }) =>
        listen<string>(MAIN_NAVIGATE_EVENT, (event) => {
          setActiveSection(event.payload as AppSectionKey);
        }),
      )
      .then((fn) => {
        unlisten = fn;
      })
      .catch(() => {});
    return () => {
      if (unlisten) unlisten();
    };
  }, []);

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
          onClick={() => setActiveSection('workspace')}
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
          onClick={() => setActiveSection('workspace')}
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
            <button type="button" onClick={() => setActiveSection('workspace')}>
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
            <button type="button" onClick={() => setActiveSection('workspace')}>
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
      case 'workspace':
        return <WorkspacePage />;
      case 'memory':
        return (
          <section className="dashboard-card live-tool-card page-tool-card">
            <div className="card-title-row">
              <h3>记忆中心</h3>
            </div>
            <MemoryCenter />
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
            <AvatarControlPanel settings={avatarSettings} onChange={updateAvatarSettings} />
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
