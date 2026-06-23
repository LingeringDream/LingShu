import { useState } from 'react';
import { apiFetch, postSignal } from '../../lib/api';
import { runAutomationAction } from '../../lib/automation';
import { isTauri, invokeTauri } from '../../lib/tauri';
import { MarkdownContent } from './MarkdownContent';
import {
  ThumbsUpIcon,
  ThumbsDownIcon,
  RulerIcon,
  ScissorsIcon,
  PenIcon,
  CheckIcon,
  UnlockIcon,
  StarClusterIcon,
} from '../icons';

export type FeedbackStyleTag = 'too_long' | 'too_short' | 'too_formal';

export interface EnhanceMember {
  id: string;
  label: string;
  provider: string;
  status: 'thinking' | 'done' | 'failed';
  draft?: string;
  error?: string;
}

export interface EnhanceState {
  phase: 'drafting' | 'aggregating' | 'done';
  members: EnhanceMember[];
  aggregatorLabel?: string;
}

export interface Message {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  timestamp: Date;
  /** Backend message UUID — set when the assistant response is persisted.
   *  Feedback buttons are disabled when this is undefined. */
  dbId?: string;
  /** Local content revision. Used by multi-window sync so a regenerated answer
   *  is not overwritten by an older, longer snapshot of the same bubble. */
  revision?: number;
  /** L2 permission requests from the backend — shown as inline grant buttons. */
  permissionRequests?: Array<{ kind: string; target: string; reason: string }>;
  /** True while this assistant message is being produced by the MoA endpoint. */
  enhancing?: boolean;
  /** Multi-model enhancement progress shown in the assistant bubble. */
  enhance?: EnhanceState;
}

interface MessageBubbleProps {
  message: Message;
  onStyleRegenerate?: (messageId: string, styleTag: FeedbackStyleTag) => void | Promise<void>;
}

// ── MessageBubble ──────────────────────────────────────────────────

export function MessageBubble({ message, onStyleRegenerate }: MessageBubbleProps) {
  const isUser = message.role === 'user';
  const [feedbackSent, setFeedbackSent] = useState<string | null>(null);
  const [enhanceOpen, setEnhanceOpen] = useState(() => message.enhance?.phase !== 'done');
  const [expandedMemberId, setExpandedMemberId] = useState<string | null>(null);
  const canSendFeedback = !isUser && message.dbId !== undefined;
  const hasEnhance = !isUser && message.enhance !== undefined;

  const feedbackKey = (eventType: string, metadata?: Record<string, unknown>) => {
    if (eventType === 'reply_style_tag' && typeof metadata?.tag === 'string') {
      return `${eventType}:${metadata.tag}`;
    }
    return eventType;
  };

  const styleTagFromMetadata = (metadata?: Record<string, unknown>): FeedbackStyleTag | null => {
    const tag = metadata?.tag;
    return tag === 'too_long' || tag === 'too_short' || tag === 'too_formal' ? tag : null;
  };

  const sendFeedback = async (eventType: string, metadata?: Record<string, unknown>) => {
    if (!canSendFeedback) return;
    const key = feedbackKey(eventType, metadata);
    try {
      await postSignal({
        event_type: eventType,
        entity_type: 'message',
        entity_id: message.dbId,
        metadata: metadata ?? {},
      });
    } catch {
      // best-effort — don't interrupt the UI
    }
    setFeedbackSent(key);
    if (eventType === 'reply_style_tag') {
      const styleTag = styleTagFromMetadata(metadata);
      if (styleTag) void onStyleRegenerate?.(message.id, styleTag);
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
          <>
            {hasEnhance && message.enhance && (
              <EnhanceProcess
                enhance={message.enhance}
                open={enhanceOpen || message.enhance.phase !== 'done'}
                expandedMemberId={expandedMemberId}
                onToggleOpen={() => setEnhanceOpen((open) => !open)}
                onToggleMember={(id) => setExpandedMemberId((current) => (current === id ? null : id))}
              />
            )}
            {hasEnhance && message.content.trim() && (
              <div style={enhanceBadgeRowStyle}>
                <span style={enhanceBadgeStyle}>
                  <StarClusterIcon size={12} />
                  ✦增强
                </span>
              </div>
            )}
            <MarkdownContent content={message.content} />
          </>
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
          flexDirection: 'column',
          alignItems: 'flex-start',
          gap: '4px',
          opacity: canSendFeedback ? 1 : 0.3,
          transition: 'opacity 0.15s',
        }}>
          <div style={{ display: 'flex', gap: '6px', flexWrap: 'wrap' }}>
            {/* Thumbs up */}
            <button
              disabled={!canSendFeedback}
              onClick={() => sendFeedback('reply_thumb_up')}
              title="点赞"
              style={feedbackChipStyle(feedbackSent === 'reply_thumb_up')}
            >
              <ThumbsUpIcon size={14} title="点赞" />
            </button>

            {/* Thumbs down */}
            <button
              disabled={!canSendFeedback}
              onClick={() => sendFeedback('reply_thumb_down')}
              title="点踩"
              style={feedbackChipStyle(feedbackSent === 'reply_thumb_down')}
            >
              <ThumbsDownIcon size={14} title="点踩" />
            </button>

            {/* Style chips */}
            <button
              disabled={!canSendFeedback}
              onClick={() => sendFeedback('reply_style_tag', { tag: 'too_long' })}
              title="太长了"
              style={feedbackChipStyle(feedbackSent === 'reply_style_tag:too_long')}
            >
              <RulerIcon size={13} style={{ verticalAlign: '-2px', marginRight: 4 }} />太长了
            </button>
            <button
              disabled={!canSendFeedback}
              onClick={() => sendFeedback('reply_style_tag', { tag: 'too_short' })}
              title="太短了"
              style={feedbackChipStyle(feedbackSent === 'reply_style_tag:too_short')}
            >
              <ScissorsIcon size={13} style={{ verticalAlign: '-2px', marginRight: 4 }} />太短了
            </button>
            <button
              disabled={!canSendFeedback}
              onClick={() => sendFeedback('reply_style_tag', { tag: 'too_formal' })}
              title="太正式"
              style={feedbackChipStyle(feedbackSent === 'reply_style_tag:too_formal')}
            >
              <PenIcon size={13} style={{ verticalAlign: '-2px', marginRight: 4 }} />太正式
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

// ── Enhancement Timeline ──────────────────────────────────────────────

function EnhanceProcess({
  enhance,
  open,
  expandedMemberId,
  onToggleOpen,
  onToggleMember,
}: {
  enhance: EnhanceState;
  open: boolean;
  expandedMemberId: string | null;
  onToggleOpen: () => void;
  onToggleMember: (id: string) => void;
}) {
  const count = enhance.members.length;

  if (!open && enhance.phase === 'done') {
    return (
      <button type="button" onClick={onToggleOpen} style={enhanceCollapsedStyle}>
        <StarClusterIcon size={13} />
        由 {count} 个模型增强·查看过程
      </button>
    );
  }

  return (
    <div style={enhanceTimelineStyle}>
      {enhance.phase !== 'done' && (
        <div style={enhanceStageStyle}>
          <StarClusterIcon size={14} />
          正在征询 {count} 个模型
        </div>
      )}

      <div style={enhanceMembersStyle}>
        {enhance.members.map((member) => (
          <div key={member.id} style={{ display: 'flex', flexDirection: 'column', gap: 5 }}>
            <button
              type="button"
              onClick={() => onToggleMember(member.id)}
              className={member.status === 'thinking' ? 'enhance-chip-thinking' : undefined}
              style={enhanceMemberChipStyle(member.status)}
            >
              <span style={enhanceStatusDotStyle(member.status)} />
              {member.label}
            </button>
            {expandedMemberId === member.id && member.draft && (
              <div style={enhanceDraftStyle}>{member.draft}</div>
            )}
            {member.status === 'failed' && member.error && (
              <div style={enhanceErrorStyle}>{member.error}</div>
            )}
          </div>
        ))}
      </div>

      {(enhance.phase === 'aggregating' || enhance.phase === 'done') && (
        <div style={enhanceStageStyle}>
          <CheckIcon size={14} />
          正在综合各方观点
          {enhance.aggregatorLabel ? (
            <span style={enhanceAggregatorStyle}>{enhance.aggregatorLabel}</span>
          ) : null}
        </div>
      )}

      {enhance.phase === 'done' && (
        <button type="button" onClick={onToggleOpen} style={enhanceHideStyle}>
          收起过程
        </button>
      )}
    </div>
  );
}

const enhanceCollapsedStyle: React.CSSProperties = {
  width: '100%',
  minHeight: 28,
  display: 'inline-flex',
  alignItems: 'center',
  justifyContent: 'flex-start',
  gap: 6,
  marginBottom: 8,
  padding: '5px 8px',
  border: '1px solid var(--border)',
  borderRadius: 6,
  background: 'var(--surface-soft)',
  color: 'var(--text-secondary)',
  cursor: 'pointer',
  fontSize: 12,
  textAlign: 'left',
};

const enhanceTimelineStyle: React.CSSProperties = {
  display: 'flex',
  flexDirection: 'column',
  gap: 8,
  marginBottom: 10,
  paddingBottom: 10,
  borderBottom: '1px solid var(--border)',
};

const enhanceStageStyle: React.CSSProperties = {
  display: 'flex',
  alignItems: 'center',
  gap: 7,
  minHeight: 22,
  color: 'var(--text-secondary)',
  fontSize: 12,
  fontWeight: 560,
};

const enhanceMembersStyle: React.CSSProperties = {
  display: 'flex',
  flexWrap: 'wrap',
  gap: 6,
};

function enhanceMemberChipStyle(status: EnhanceMember['status']): React.CSSProperties {
  return {
    display: 'inline-flex',
    alignItems: 'center',
    gap: 6,
    minHeight: 26,
    maxWidth: 240,
    padding: '4px 8px',
    border: '1px solid var(--border)',
    borderRadius: 6,
    background: status === 'done'
      ? 'rgba(34, 197, 94, 0.1)'
      : status === 'failed'
        ? 'rgba(152, 162, 179, 0.1)'
        : 'var(--accent-soft)',
    color: status === 'failed' ? 'var(--text-tertiary)' : 'var(--text-primary)',
    cursor: 'pointer',
    fontSize: 12,
    overflow: 'hidden',
    textOverflow: 'ellipsis',
    whiteSpace: 'nowrap',
  };
}

function enhanceStatusDotStyle(status: EnhanceMember['status']): React.CSSProperties {
  return {
    width: 7,
    height: 7,
    flex: '0 0 7px',
    borderRadius: 999,
    background: status === 'done'
      ? 'var(--success)'
      : status === 'failed'
        ? 'var(--text-tertiary)'
        : 'var(--accent)',
  };
}

const enhanceDraftStyle: React.CSSProperties = {
  maxWidth: 420,
  padding: '7px 9px',
  borderRadius: 6,
  background: 'var(--surface-soft)',
  color: 'var(--text-secondary)',
  border: '1px solid var(--border)',
  fontSize: 12,
  whiteSpace: 'pre-wrap',
};

const enhanceErrorStyle: React.CSSProperties = {
  color: 'var(--text-tertiary)',
  fontSize: 12,
  paddingLeft: 4,
};

const enhanceAggregatorStyle: React.CSSProperties = {
  color: 'var(--text-tertiary)',
  fontWeight: 400,
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap',
};

const enhanceHideStyle: React.CSSProperties = {
  alignSelf: 'flex-start',
  color: 'var(--text-tertiary)',
  background: 'transparent',
  cursor: 'pointer',
  fontSize: 12,
};

const enhanceBadgeRowStyle: React.CSSProperties = {
  display: 'flex',
  justifyContent: 'flex-start',
  marginBottom: 6,
};

const enhanceBadgeStyle: React.CSSProperties = {
  display: 'inline-flex',
  alignItems: 'center',
  gap: 4,
  minHeight: 22,
  padding: '2px 7px',
  borderRadius: 6,
  background: 'var(--accent-light)',
  color: 'var(--accent)',
  fontSize: 12,
  fontWeight: 620,
};

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
          {granted.has(req.target)
            ? <CheckIcon size={14} style={{ verticalAlign: '-2px', marginRight: 6 }} />
            : <UnlockIcon size={14} style={{ verticalAlign: '-2px', marginRight: 6 }} />}
          {req.kind === 'accessibility'
            ? granted.has(req.target)
              ? '已开启 L3 权限——请在系统设置中勾选「灵枢」后重试'
              : '开启屏幕识别权限'
            : granted.has(req.target)
              ? `已授权并打开 ${req.target}`
              : `授予权限并打开 ${req.target}`}
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
