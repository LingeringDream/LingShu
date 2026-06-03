import { AppLayout } from './components/layout/AppLayout';
import { ChatWindow } from './components/chat/ChatWindow';
import { AvatarPlaceholder } from './components/avatar/AvatarPlaceholder';

export default function App() {
  return (
    <AppLayout>
      <div style={{ display: 'flex', height: '100%', gap: '16px', padding: '16px' }}>
        <div style={{ flex: '0 0 320px', display: 'flex', flexDirection: 'column' }}>
          <AvatarPlaceholder />
        </div>
        <div style={{ flex: 1, display: 'flex', flexDirection: 'column', minWidth: 0 }}>
          <ChatWindow />
        </div>
      </div>
    </AppLayout>
  );
}
