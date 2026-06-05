import { ReactNode } from 'react';

export type AppSectionKey =
  | 'home'
  | 'chat'
  | 'calendar'
  | 'memory'
  | 'tasks'
  | 'projects'
  | 'personality'
  | 'thoughts'
  | 'preferences'
  | 'privacy'
  | 'settings';

interface AppLayoutProps {
  children: ReactNode;
  activeSection?: AppSectionKey;
  onNavigate?: (section: AppSectionKey) => void;
}

const navItems: { key: AppSectionKey; label: string }[] = [
  { key: 'home', label: '首页' },
  { key: 'chat', label: '聊天' },
  { key: 'calendar', label: '日历' },
  { key: 'memory', label: '记忆中心' },
  { key: 'tasks', label: '我的任务' },
  { key: 'projects', label: '项目管理' },
  { key: 'personality', label: '人格中心' },
  { key: 'thoughts', label: '思维队列' },
  { key: 'preferences', label: '偏好设置' },
  { key: 'privacy', label: '权限与隐私' },
  { key: 'settings', label: '模型配置' },
];

export function AppLayout({ children, activeSection = 'home', onNavigate }: AppLayoutProps) {
  return (
    <div className="app-shell">
      <header className="app-topbar">
        <div className="topbar-brand">
          <img className="topbar-logo" src="/brand/lingshu-icon.svg" alt="灵枢" />
          <h1>个人助理本地控制台</h1>
        </div>
        <div className="topbar-actions">
          <button type="button" className="user-menu-button">
            <span>本地用户</span>
          </button>
        </div>
      </header>

      <div className="app-frame">
        <aside className="app-sidebar">
          <div className="brand-block">
            <img className="brand-mark" src="/brand/lingshu-icon.svg" alt="" aria-hidden="true" />
            <div>
              <div className="brand-title">灵枢</div>
              <div className="brand-subtitle">macOS 桌面 AI 个人助理</div>
            </div>
          </div>

          <nav className="app-nav">
            {navItems.map(({ key, label }) => (
              <button
                key={key}
                className={`nav-button ${activeSection === key ? 'nav-button-active' : ''}`}
                onClick={() => {
                  onNavigate?.(key);
                }}
              >
                <span>{label}</span>
              </button>
            ))}
          </nav>

          <div className="sidebar-footer">
            <div className="privacy-note">
              <strong>本地优先</strong>
              <span>在这里管理你的偏好、隐私和模型使用设置。</span>
            </div>
            <div className="status-pill">
              <span className="status-dot" />
              在线
            </div>
          </div>
        </aside>

        <main className="app-main">{children}</main>
      </div>
    </div>
  );
}
