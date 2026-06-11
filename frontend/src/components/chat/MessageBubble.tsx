import { useState } from 'react';
import ReactMarkdown, { type Components } from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { apiFetch, postSignal } from '../../lib/api';
import { runAutomationAction } from '../../lib/automation';
import { isTauri, invokeTauri } from '../../lib/tauri';

export interface Message {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  timestamp: Date;
  /** Backend message UUID — set when the assistant response is persisted.
   *  Feedback buttons are disabled when this is undefined. */
  dbId?: string;
  /** L2 permission requests from the backend — shown as inline grant buttons. */
  permissionRequests?: Array<{ kind: string; target: string; reason: string }>;
}

interface MessageBubbleProps {
  message: Message;
}

// ── Shared Markdown component styles ──────────────────────────────

const markdownStyles: React.CSSProperties = {
  fontSize: '14px',
  lineHeight: '1.65',
  wordBreak: 'break-word',
  overflowWrap: 'break-word',
};

// ── Components passed to ReactMarkdown for styling ─────────────────

const mdComponents: Components = {
  code(props) {
    const { className, children } = props;
    const inline = !className;
    if (inline) {
      return (
        <code style={{
          background: 'var(--bg-secondary, rgba(0,0,0,0.06))',
          borderRadius: 3,
          padding: '1px 5px',
          fontSize: '0.9em',
          fontFamily: 'ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, monospace',
        }}>
          {children}
        </code>
      );
    }
    return (
      <pre style={{
        background: 'var(--bg-secondary, rgba(0,0,0,0.05))',
        borderRadius: 8,
        padding: '12px 14px',
        overflowX: 'auto',
        fontSize: '13px',
        fontFamily: 'ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, monospace',
        lineHeight: 1.5,
        margin: '8px 0',
      }}>
        <code className={className}>{children}</code>
      </pre>
    );
  },

  p({ children }) { return <p style={{ margin: '0 0 6px' }}>{children}</p>; },
  ul({ children }) { return <ul style={{ margin: '4px 0', paddingLeft: 20 }}>{children}</ul>; },
  ol({ children }) { return <ol style={{ margin: '4px 0', paddingLeft: 20 }}>{children}</ol>; },
  li({ children }) { return <li style={{ margin: '2px 0' }}>{children}</li>; },
  blockquote({ children }) {
    return (
      <blockquote style={{
        borderLeft: '3px solid var(--accent, #0a73ff)',
        margin: '6px 0',
        paddingLeft: 12,
        color: 'var(--text-secondary, #666)',
      }}>
        {children}
      </blockquote>
    );
  },
  table({ children }) {
    return (
      <div style={{ overflowX: 'auto', margin: '8px 0' }}>
        <table style={{ borderCollapse: 'collapse', fontSize: '13px', width: '100%' }}>{children}</table>
      </div>
    );
  },
  th({ children }) {
    return (
      <th style={{
        border: '1px solid var(--border, #ddd)', padding: '6px 10px',
        background: 'var(--bg-secondary, rgba(0,0,0,0.04))', textAlign: 'left', fontWeight: 600,
      }}>{children}</th>
    );
  },
  td({ children }) {
    return <td style={{ border: '1px solid var(--border, #ddd)', padding: '5px 10px' }}>{children}</td>;
  },
  hr() {
    return <hr style={{ border: 'none', borderTop: '1px solid var(--border, #ddd)', margin: '10px 0' }} />;
  },
  a({ children, href }) {
    return (
      <a href={href} target="_blank" rel="noopener noreferrer"
        style={{ color: 'var(--accent, #0a73ff)', textDecoration: 'underline' }}>
        {children}
      </a>
    );
  },
  h1({ children }) { return <h1 style={{ fontSize: '1.3em', fontWeight: 700, margin: '10px 0 4px' }}>{children}</h1>; },
  h2({ children }) { return <h2 style={{ fontSize: '1.15em', fontWeight: 600, margin: '8px 0 4px' }}>{children}</h2>; },
  h3({ children }) { return <h3 style={{ fontSize: '1.05em', fontWeight: 600, margin: '6px 0 3px' }}>{children}</h3>; },
  strong({ children }) { return <strong style={{ fontWeight: 600 }}>{children}</strong>; },
  em({ children }) { return <em>{children}</em>; },
};

// ── MessageBubble ──────────────────────────────────────────────────

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
        maxWidth: '78%',
        padding: '12px 16px',
        borderRadius: '12px',
        background: isUser ? 'var(--accent)' : 'var(--bg-tertiary)',
        color: isUser ? '#fff' : 'var(--text-primary)',
        fontSize: '14px',
        lineHeight: '1.5',
        wordBreak: 'break-word',
      }}>
        {isUser ? (
          <span style={{ whiteSpace: 'pre-wrap' }}>{message.content}</span>
        ) : (
          <div style={markdownStyles}>
            <ReactMarkdown remarkPlugins={[remarkGfm]} components={mdComponents}>
              {message.content}
            </ReactMarkdown>
          </div>
        )}
      </div>

      {/* Permission grant button — shown when L2 whitelist blocks an action */}
      {!isUser && message.permissionRequests && message.permissionRequests.length > 0 && (
        <PermissionGrantButton requests={message.permissionRequests} />
      )}

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

// ── Permission Grant Button ──────────────────────────────────────────

interface PermissionRequest {
  kind: string;
  target: string;
  reason: string;
}

function PermissionGrantButton({ requests }: { requests: PermissionRequest[] }) {
  const [granted, setGranted] = useState<Set<string>>(new Set());
  const [loading, setLoading] = useState(false);

  const handleGrant = async (req: PermissionRequest) => {
    setLoading(true);
    try {
      if (req.kind === 'accessibility') {
        // Screen reading needs two grants: the in-app L3 tier and the macOS
        // Accessibility permission. Enable L3 here, then fire the system
        // prompt from the desktop app (it registers 「灵枢 LingShu」 in the
        // Accessibility list with the current code signature).
        await apiFetch('/api/v1/permissions', {
          method: 'PATCH',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ l3_accessibility: true }),
        });
        if (isTauri()) {
          try {
            await invokeTauri('request_accessibility_permission');
          } catch (e) {
            console.error('[permission] accessibility prompt failed:', e);
          }
        }
        setGranted((s) => new Set(s).add(req.target));
        return;
      }
      // 1. Add target to whitelist
      const resp = await apiFetch('/api/v1/permissions');
      if (!resp.ok) throw new Error('无法读取权限');
      const perms = await resp.json();
      const currentWhitelist: string[] = perms.l2_whitelist ?? [];
      if (!currentWhitelist.includes(req.target)) {
        currentWhitelist.push(req.target);
      }
      await apiFetch('/api/v1/permissions', {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          l2_automation: true,
          l2_whitelist: currentWhitelist,
        }),
      });
      // 2. Execute the action
      await runAutomationAction({ kind: req.kind, target: req.target });
      setGranted((s) => new Set(s).add(req.target));
    } catch (e) {
      console.error('[permission] grant failed:', e);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 6, marginTop: 8 }}>
      {requests.map((req, i) => (
        <button
          key={i}
          disabled={loading || granted.has(req.target)}
          onClick={() => handleGrant(req)}
          style={{
            padding: '6px 14px',
            fontSize: 13,
            border: '1px solid var(--accent, #0a73ff)',
            borderRadius: 6,
            background: granted.has(req.target) ? '#4caf50' : 'var(--accent, #0a73ff)',
            color: '#fff',
            cursor: granted.has(req.target) ? 'default' : 'pointer',
            opacity: loading ? 0.6 : 1,
          }}
        >
          {req.kind === 'accessibility'
            ? granted.has(req.target)
              ? '✅ 已开启 L3 权限——请在系统设置中勾选「灵枢」后重试'
              : '🔓 开启屏幕识别权限'
            : granted.has(req.target)
              ? `✅ 已授权并打开 ${req.target}`
              : `🔓 授予权限并打开 ${req.target}`}
        </button>
      ))}
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
