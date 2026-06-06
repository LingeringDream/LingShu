import { useState } from 'react';
import { postSignal } from '../../lib/api';

export interface Message {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  timestamp: Date;
  /** Backend message UUID — set when the assistant response is persisted.
   *  Feedback buttons are disabled when this is undefined. */
  dbId?: string;
}

interface MessageBubbleProps {
  message: Message;
}

export function MessageBubble({ message }: MessageBubbleProps) {
  const isUser = message.role === 'user';
  const [feedbackSent, setFeedbackSent] = useState<string | null>(null);
  const canSendFeedback = !isUser && message.dbId !== undefined;

  const sendFeedback = async (eventType: string, metadata?: Record<string, unknown>) => {
    if (!canSendFeedback) return;
    try {
      await postSignal({
        event_type: eventType,
        entity_type: 'message',
        entity_id: message.dbId,
        metadata: metadata ?? {},
      });
      setFeedbackSent(eventType);
    } catch {
      // best-effort — don't interrupt the UI
    }
  };

  return (
    <div style={{
      display: 'flex',
      flexDirection: 'column',
      alignItems: isUser ? 'flex-end' : 'flex-start',
      gap: '6px',
    }}>
      <div style={{
        maxWidth: '70%',
        padding: '12px 16px',
        borderRadius: '12px',
        background: isUser ? 'var(--accent)' : 'var(--bg-tertiary)',
        color: isUser ? '#fff' : 'var(--text-primary)',
        fontSize: '14px',
        lineHeight: '1.5',
        whiteSpace: 'pre-wrap',
        wordBreak: 'break-word',
      }}>
        {message.content}
      </div>

      {/* Feedback row — only for assistant messages with a dbId */}
      {!isUser && (
        <div style={{
          display: 'flex',
          gap: '6px',
          opacity: canSendFeedback ? 1 : 0.3,
          transition: 'opacity 0.15s',
        }}>
          {/* Thumbs up */}
          <button
            disabled={!canSendFeedback}
            onClick={() => sendFeedback('reply_thumb_up')}
            title="点赞"
            style={feedbackChipStyle(feedbackSent === 'reply_thumb_up')}
          >
            👍
          </button>

          {/* Thumbs down */}
          <button
            disabled={!canSendFeedback}
            onClick={() => sendFeedback('reply_thumb_down')}
            title="点踩"
            style={feedbackChipStyle(feedbackSent === 'reply_thumb_down')}
          >
            👎
          </button>

          {/* Style chips */}
          <button
            disabled={!canSendFeedback}
            onClick={() => sendFeedback('reply_style_tag', { tag: 'too_long' })}
            title="太长了"
            style={feedbackChipStyle(feedbackSent === 'reply_style_tag')}
          >
            📏 太长了
          </button>
          <button
            disabled={!canSendFeedback}
            onClick={() => sendFeedback('reply_style_tag', { tag: 'too_short' })}
            title="太短了"
            style={feedbackChipStyle()}
          >
            ✂️ 太短了
          </button>
          <button
            disabled={!canSendFeedback}
            onClick={() => sendFeedback('reply_style_tag', { tag: 'too_formal' })}
            title="太正式"
            style={feedbackChipStyle()}
          >
            📝 太正式
          </button>
        </div>
      )}
    </div>
  );
}

function feedbackChipStyle(active?: boolean): React.CSSProperties {
  return {
    border: '1px solid var(--border)',
    borderRadius: '14px',
    padding: '2px 8px',
    fontSize: '11px',
    background: active ? 'var(--accent)' : 'transparent',
    color: active ? '#fff' : 'var(--text-secondary)',
    cursor: 'pointer',
    lineHeight: '1.4',
  };
}
