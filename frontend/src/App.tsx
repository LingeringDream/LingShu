import { useState, useEffect, useCallback } from 'react';
import { AppLayout, resolveAppSection, type AppSectionKey } from './components/layout/AppLayout';
import { ChatWindow } from './components/chat/ChatWindow';
import { ChatSettings } from './components/chat/ChatSettings';
import { MemoryCenter } from './components/memory/MemoryCenter';
import { PermissionSettings } from './components/settings/PermissionSettings';
import { AiToolsPanel } from './components/settings/AiToolsPanel';
import { CloudAccountPanel } from './components/settings/CloudAccountPanel';
import { useCloudAccountStore } from './stores/cloudAccountStore';
import { ensureLocalSession, apiFetch } from './lib/api';
import { isTauri, MAIN_NAVIGATE_EVENT } from './lib/tauri';
import { isMemoryLabAvailable } from './lib/edition';
import {
  DESKTOP_BACKEND_STARTUP_TIMEOUT_MS,
  WEB_BACKEND_STARTUP_TIMEOUT_MS,
  localSessionStartupErrorMessage,
  retryLocalSession,
} from './lib/localSessionBoot';
import { installChatSessionSync, useChatStore } from './stores/chatStore';

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

function MemoryLabRoute() {
  const [El, setEl] = useState<React.FC | null>(null);
  useEffect(() => {
    if (isMemoryLabAvailable) {
      import('./components/memory/MemoryEvolutionLab').then((m) => setEl(() => m.MemoryEvolutionLab));
    }
  }, []);
  if (!El) {
    return (
      <section className="dashboard-card page-tool-card">
        <div className="card-title-row"><h3>功能未开放</h3></div>
        <p>此功能在当前版本中不可用。</p>
      </section>
    );
  }
  return <El />;
}

export default function App() {
  const cloudAccountName = useCloudAccountStore((state) => state.account?.user.display_name);
  const [sessionState, setSessionState] = useState<'loading' | 'ready' | 'error'>('loading');
  const [sessionError, setSessionError] = useState<string | null>(null);
  const [activeSection, setActiveSection] = useState<AppSectionKey>(() =>
    resolveAppSection(new URLSearchParams(window.location.search).get('section')),
  );
  const [recentConversations, setRecentConversations] = useState<ConversationItem[]>([]);
  const [recentMemories, setRecentMemories] = useState<MemoryItem[]>([]);
  const [overviewLoading, setOverviewLoading] = useState(false);
  const [overviewError, setOverviewError] = useState<string | null>(null);

  const bootLocalSession = useCallback(async () => {
    setSessionState('loading');
    setSessionError(null);
    try {
      // A packaged Windows first launch may need well over 30 seconds to
      // extract and initialise the embedded PostgreSQL runtime. The desktop
      // shell waits before showing this window, so the frontend must not enter
      // a stale error state while that startup wait is still in progress.
      await retryLocalSession({
        request: () => ensureLocalSession(true),
        timeoutMs: isTauri()
          ? DESKTOP_BACKEND_STARTUP_TIMEOUT_MS
          : WEB_BACKEND_STARTUP_TIMEOUT_MS,
      });
      setSessionState('ready');
    } catch (error) {
      setSessionError(localSessionStartupErrorMessage(error, isTauri()));
      setSessionState('error');
    }
  }, []);

  useEffect(() => {
    bootLocalSession();
  }, [bootLocalSession]);

  useEffect(() => installChatSessionSync(), []);

  useEffect(() => {
    void useCloudAccountStore.getState().initialize();
    const refresh = () => {
      const state = useCloudAccountStore.getState();
      if (state.account && document.visibilityState === 'visible') void state.reload();
    };
    const timer = window.setInterval(refresh, 60_000);
    window.addEventListener('focus', refresh);
    window.addEventListener('online', refresh);
    return () => {
      window.clearInterval(timer);
      window.removeEventListener('focus', refresh);
      window.removeEventListener('online', refresh);
    };
  }, []);

  useEffect(() => {
    if (sessionState !== 'ready' || activeSection !== 'home') return;
    let cancelled = false;
    setOverviewLoading(true);
    setOverviewError(null);
    const load = async () => {
      const results = await Promise.allSettled([
        apiFetch('/api/v1/memories?limit=5').then(async (response) => {
          if (!response.ok) throw new Error(`HTTP ${response.status}`);
          return response.json() as Promise<MemoryItem[]>;
        }),
        apiFetch('/api/v1/conversations').then(async (response) => {
          if (!response.ok) throw new Error(`HTTP ${response.status}`);
          return response.json() as Promise<ConversationItem[]>;
        }),
      ]);
      if (cancelled) return;
      if (results[0].status === 'fulfilled') setRecentMemories(results[0].value);
      else setRecentMemories([]);
      if (results[1].status === 'fulfilled') setRecentConversations(results[1].value.slice(0, 5));
      else setRecentConversations([]);
      if (results.some((result) => result.status === 'rejected')) {
        setOverviewError('部分最近记录加载失败，请到记忆中心或聊天中查看。');
      }
      setOverviewLoading(false);
    };
    void load();
    return () => { cancelled = true; };
  }, [sessionState, activeSection]);

  const openRecentConversation = useCallback(async (conversationId: string) => {
    const loaded = await useChatStore.getState().loadSession(conversationId);
    if (loaded) setActiveSection('chat');
  }, []);

  // Deep-link from the pet window: when the user expands a reply (or opens the
  // console), jump straight to the requested section (e.g. 'chat') instead of
  // landing on whatever tab the main window last showed.
  useEffect(() => {
    if (!isTauri()) return;
    let unlisten: (() => void) | null = null;
    import('@tauri-apps/api/event')
      .then(({ listen }) =>
        listen<string>(MAIN_NAVIGATE_EVENT, (event) => {
          setActiveSection(resolveAppSection(event.payload));
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

  // ── 全局快捷键：Ctrl/Cmd + Shift + M 打开「接入 AI 工具」面板 ──
  useEffect(() => {
    if (sessionState !== 'ready') return undefined;
    const handler = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.shiftKey && (e.key === 'M' || e.key === 'm')) {
        const target = e.target as HTMLElement | null;
        const tag = target?.tagName;
        if (tag === 'INPUT' || tag === 'TEXTAREA' || target?.isContentEditable) return;
        e.preventDefault();
        setActiveSection('ai-tools');
      }
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [sessionState]);

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
              <p>
                {isTauri()
                  ? '正在启动本地服务；首次运行需要初始化内置数据库，可能需要 1–2 分钟...'
                  : '正在连接本地服务...'}
              </p>
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

  const renderHome = () => (
    <>
      <section className="dashboard-hero">
        <div>
          <p className="eyebrow">本地共享记忆</p>
          <h2>让不同 AI 接着你的上下文工作</h2>
          <p>在这里管理记忆，通过 MCP 接入常用 AI 工具；聊天也使用同一套记忆。</p>
        </div>
      </section>
      <section className="stat-grid" aria-label="开始使用">
        <button type="button" className="stat-card stat-action-card" onClick={() => setActiveSection('memory')}>
          <div className="stat-icon stat-blue" aria-hidden="true">记</div>
          <div><h3>管理记忆</h3><p>添加、搜索和修正你的长期记忆</p></div>
        </button>
        <button type="button" className="stat-card stat-action-card" onClick={() => setActiveSection('ai-tools')}>
          <div className="stat-icon stat-green" aria-hidden="true">连</div>
          <div><h3>接入 AI 工具</h3><p>让支持 MCP 的工具读取和写入记忆</p></div>
        </button>
        <button type="button" className="stat-card stat-action-card" onClick={() => setActiveSection('chat')}>
          <div className="stat-icon stat-orange" aria-hidden="true">聊</div>
          <div><h3>开始聊天</h3><p>配置模型后，用对话积累记忆</p></div>
        </button>
      </section>
      {overviewError && <p className="overview-message" role="alert">{overviewError}</p>}
      {overviewLoading && <p className="overview-message" role="status">正在加载最近记录…</p>}
      <section className="dashboard-grid">
        <article className="dashboard-card">
          <div className="card-title-row"><h3>最近记忆</h3><button type="button" onClick={() => setActiveSection('memory')}>查看全部</button></div>
          {!overviewLoading && recentMemories.length === 0 ? (
            <p className="overview-message">还没有记忆。到记忆中心添加一条，或让接入的 AI 工具写入。</p>
          ) : (
            <ul className="memory-bullets">{recentMemories.map((memory) => <li key={memory.id}>{memory.content}</li>)}</ul>
          )}
        </article>
        <article className="dashboard-card">
          <div className="card-title-row"><h3>最近对话</h3><button type="button" onClick={() => setActiveSection('chat')}>打开聊天</button></div>
          {!overviewLoading && recentConversations.length === 0 ? (
            <p className="overview-message">还没有对话。你也可以直接在常用 AI 工具中使用灵枢记忆。</p>
          ) : (
            <div className="conversation-list">{recentConversations.map((conversation) => (
              <button type="button" className="conversation-row" key={conversation.id} onClick={() => void openRecentConversation(conversation.id)}>
                <span className="conversation-avatar" aria-hidden="true">聊</span>
                <span>{conversation.title || '无标题对话'}</span>
                <time>{new Date(conversation.created_at).toLocaleDateString('zh-CN')}</time>
              </button>
            ))}</div>
          )}
        </article>
      </section>
    </>
  );

  const renderActivePanel = () => {
    switch (activeSection) {
      case 'account':
        return <CloudAccountPanel />;
      case 'ai-tools':
        return <AiToolsPanel />;
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
      case 'memory':
        return (
          <section className="dashboard-card live-tool-card page-tool-card">
            <div className="card-title-row">
              <h3>记忆中心</h3>
            </div>
            <MemoryCenter />
          </section>
        );
      case 'memory-lab':
        return isMemoryLabAvailable ? <MemoryLabRoute /> : renderHome();
      case 'settings':
        return (
          <section className="dashboard-card page-tool-card narrow-tool-card">
            <div className="card-title-row"><h3>模型与隐私设置</h3></div>
            <ChatSettings collapsed={false} />
            <details className="advanced-permissions">
              <summary>高级系统权限</summary>
              <p>管理已有聊天工具的系统访问权限。仅在需要时开启。</p>
              <PermissionSettings collapsed={false} />
            </details>
          </section>
        );
      default:
        return renderHome();
    }
  };

  return (
    <AppLayout activeSection={activeSection} onNavigate={setActiveSection} accountName={cloudAccountName}>
      <div className="workspace dashboard-workspace">
        <div
          id={`section-${activeSection}`}
          className={`dashboard-scroll ${activeSection === 'memory-lab' && isMemoryLabAvailable ? 'dashboard-scroll--memory-lab' : ''}`}
        >
          {renderActivePanel()}
        </div>
      </div>
    </AppLayout>
  );
}
