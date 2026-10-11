import { ReactNode } from 'react';
import { isMemoryLabAvailable } from '../../lib/edition';

export type AppSectionKey =
  | 'home'
  | 'chat'
  | 'memory'
  | 'memory-lab'
  | 'account'
  | 'settings'
  | 'ai-tools';

interface AppLayoutProps {
  children: ReactNode;
  activeSection?: AppSectionKey;
  onNavigate?: (section: AppSectionKey) => void;
  accountName?: string;
}

const navItems: { key: AppSectionKey; label: string }[] = [
  { key: 'home', label: '记忆概览' },
  // 写成条件展开而不是过滤，客户端构建里 isMemoryLabAvailable 会被折叠成 false，
  // 连这条导航文案都不会进客户端产物。
  ...(isMemoryLabAvailable ? [{ key: 'memory-lab' as AppSectionKey, label: '动态记忆引擎' }] : []),
  { key: 'memory', label: '记忆中心' },
  { key: 'ai-tools', label: '接入 AI 工具' },
  { key: 'chat', label: '聊天' },
  { key: 'settings', label: '设置' },
];

const memoryLabNavItems: { key: AppSectionKey; label: string }[] = [
  { key: 'memory-lab', label: '动态记忆引擎' },
  { key: 'chat', label: '对话演示' },
  { key: 'settings', label: '模型配置' },
  { key: 'home', label: '返回记忆概览' },
];

/** 旧入口与未知深链回到概览；客户端不能借深链打开开发实验台。 */
export function resolveAppSection(section: string | null): AppSectionKey {
  if (section === 'account') return 'account';
  return navItems.find(({ key }) => key === section)?.key ?? 'home';
}

export function AppLayout({ children, activeSection = 'home', onNavigate, accountName }: AppLayoutProps) {
  // 动态记忆引擎（Memory Console）只在开发版开放，桌面客户端不显示入口。
  const isMemoryLab = activeSection === 'memory-lab' && isMemoryLabAvailable;
  const displayedNavItems = isMemoryLab ? memoryLabNavItems : navItems;

  return (
    <div className={`app-shell ${isMemoryLab ? 'app-shell--memory-lab' : ''}`}>
      <header className="app-topbar">
        <div className="topbar-brand">
          <img className="topbar-logo" src="/icon.png" alt="灵枢" />
          <h1>{isMemoryLab ? 'Agent 动态记忆基础设施' : '灵枢记忆控制台'}</h1>
        </div>
        <div className="topbar-actions">
          <button type="button" className="user-menu-button" onClick={() => onNavigate?.('account')} title="管理云账户" aria-label="管理云账户">
            <span>{accountName || '登录云账户'}</span>
          </button>
        </div>
      </header>

      <div className="app-frame">
        <aside className="app-sidebar">
          <div className="brand-block">
            <img className="brand-mark" src="/icon.png" alt="" aria-hidden="true" />
            <div>
              <div className="brand-title">灵枢</div>
              <div className="brand-subtitle">
                {isMemoryLab ? 'Agent Memory Infrastructure' : '让 AI 工具共享你的记忆'}
              </div>
            </div>
          </div>

          <nav className="app-nav" aria-label="主导航">
            {displayedNavItems.map(({ key, label }) => (
              <button
                key={key}
                className={`nav-button ${activeSection === key ? 'nav-button-active' : ''}`}
                aria-current={activeSection === key ? 'page' : undefined}
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
              <span>记忆保存在本机。MCP 与聊天使用同一套记忆，云同步按需开启。</span>
            </div>
          </div>
        </aside>

        <main className="app-main">{children}</main>
      </div>
    </div>
  );
}
