import { ReactNode } from 'react';

interface AppLayoutProps {
  children: ReactNode;
}

export function AppLayout({ children }: AppLayoutProps) {
  return (
    <div style={{
      display: 'flex',
      height: '100vh',
      background: 'var(--bg-primary)',
    }}>
      {/* Sidebar */}
      <aside style={{
        width: '240px',
        background: 'var(--bg-secondary)',
        borderRight: '1px solid var(--border)',
        padding: '16px',
        display: 'flex',
        flexDirection: 'column',
        gap: '16px',
      }}>
        <div style={{
          fontSize: '20px',
          fontWeight: 'bold',
          color: 'var(--accent-light)',
          padding: '8px 0',
        }}>
          LingShu
        </div>
        <div style={{
          fontSize: '12px',
          color: 'var(--text-secondary)',
        }}>
          AI 项目经理助理
        </div>
        <nav style={{ flex: 1, display: 'flex', flexDirection: 'column', gap: '4px' }}>
          {['对话', '项目', '任务', '记忆', '设置'].map((item) => (
            <button
              key={item}
              style={{
                background: item === '对话' ? 'var(--bg-tertiary)' : 'transparent',
                border: 'none',
                color: item === '对话' ? 'var(--text-primary)' : 'var(--text-secondary)',
                padding: '10px 12px',
                borderRadius: '8px',
                cursor: 'pointer',
                textAlign: 'left',
                fontSize: '14px',
              }}
            >
              {item}
            </button>
          ))}
        </nav>
      </aside>

      {/* Main content */}
      <main style={{ flex: 1, overflow: 'hidden' }}>
        {children}
      </main>
    </div>
  );
}
