import { useRef, useEffect, useCallback } from 'react';
import { useChatStore } from '../../stores/chatStore';
import { MessageBubble } from './MessageBubble';
import { ChatInput } from './ChatInput';
import { apiFetch } from '../../lib/api';

export function ChatWindow() {
  const { messages, isLoading, sendMessage, sessionId, clearMessages } = useChatStore();

  const handleClear = useCallback(async () => {
    // Delete the conversation on the backend (soft-delete)
    if (sessionId) {
      try {
        await apiFetch(`/api/v1/conversations/${sessionId}`, { method: 'DELETE' });
      } catch { /* best-effort */ }
    }
    // Clear local messages and reset session
    clearMessages();
  }, [sessionId, clearMessages]);
  const messagesEndRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages]);

  return (
    <div style={{
      display: 'flex',
      flexDirection: 'column',
      flex: 1,
      minHeight: 0,
      background: 'var(--bg-secondary)',
      borderRadius: '8px',
      border: '1px solid var(--border)',
      overflow: 'hidden',
    }}>
      {/* Header */}
      <div style={{
        padding: '14px 18px',
        borderBottom: '1px solid var(--border)',
        display: 'flex',
        alignItems: 'center',
        gap: '12px',
      }}>
        <div style={{
          width: '8px',
          height: '8px',
          borderRadius: '50%',
          background: 'var(--success)',
        }} />
        <span style={{ fontSize: '14px', fontWeight: 500, flex: 1 }}>灵枢对话</span>
        <span style={{ fontSize: '12px', color: 'var(--text-secondary)' }}>
          {isLoading ? '思考中...' : '在线'}
        </span>
        {messages.length > 0 && (
          <button
            onClick={handleClear}
            title="清空对话"
            style={{
              background: 'transparent',
              border: '1px solid var(--border)',
              borderRadius: 4,
              padding: '2px 10px',
              fontSize: 12,
              color: 'var(--text-secondary)',
              cursor: 'pointer',
            }}
          >
            清空
          </button>
        )}
      </div>

      {/* Messages */}
      <div style={{
        flex: 1,
        overflow: 'auto',
        padding: '24px 28px',
        display: 'flex',
        flexDirection: 'column',
        gap: '18px',
      }}>
        {messages.length === 0 && (
          <div style={{
            flex: 1,
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            color: 'var(--text-secondary)',
            fontSize: '14px',
          }}>
            向灵枢发送消息开始对话...
          </div>
        )}
        {messages.map((msg) => (
          <MessageBubble key={msg.id} message={msg} />
        ))}
        <div ref={messagesEndRef} />
      </div>

      {/* Input */}
      <ChatInput onSend={sendMessage} disabled={isLoading} />
    </div>
  );
}
