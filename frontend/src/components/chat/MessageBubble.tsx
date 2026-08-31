import { useState } from 'react';
import { apiFetch, postSignal } from '../../lib/api';
import { runAutomationAction } from '../../lib/automation';
import { getPlatformCapabilities, invokeTauri } from '../../lib/tauri';
import { MarkdownContent } from './MarkdownContent';
import {
  ThumbsUpIcon,
  ThumbsDownIcon,
  RulerIcon,
  ScissorsIcon,
  PenIcon,
  CheckIcon,
  CloseIcon,
  UndoIcon,
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
  onEditUserMessage?: (messageId: string, content: string) => Promise<void>;
  onWithdrawUserMessage?: (messageId: string) => Promise<void>;
  actionsDisabled?: boolean;
}

// ── MessageBubble ──────────────────────────────────────────────────

export function MessageBubble({
  message,
  onStyleRegenerate,
  onEditUserMessage,
  onWithdrawUserMessage,
  actionsDisabled = false,
}: MessageBubbleProps) {
  const isUser = message.role === 'user';
  const [feedbackSent, setFeedbackSent] = useState<string | null>(null);
  const [editing, setEditing] = useState(false);
  const [editValue, setEditValue] = useState(message.content);
  const [confirmingWithdraw, setConfirmingWithdraw] = useState(false);
  const [userActionPending, setUserActionPending] = useState(false);
  const [userActionError, setUserActionError] = useState<string | null>(null);
  const canSendFeedback = !isUser && message.dbId !== undefined;
  const canModifyUserMessage = isUser
    && message.dbId !== undefined
    && onEditUserMessage !== undefined
    && onWithdrawUserMessage !== undefined;
  const hasEnhance = !isUser && message.enhance !== undefined;

  const startEditing = () => {
    setEditValue(message.content);
    setUserActionError(null);
    setConfirmingWithdraw(false);
    setEditing(true);
  };

  const saveEdit = async () => {
    const content = editValue.trim();
    if (!content || !onEditUserMessage) return;
    setUserActionPending(true);
    setUserActionError(null);
    try {
      await onEditUserMessage(message.id, content);
      setEditing(false);
    } catch {
      setUserActionError('修改失败，请重试');
    } finally {
      setUserActionPending(false);
    }
  };

  const confirmWithdraw = async () => {
    if (!onWithdrawUserMessage) return;
    setUserActionPending(true);
    setUserActionError(null);
    try {
      await onWithdrawUserMessage(message.id);
    } catch {
      setUserActionError('撤回失败，请重试');
      setUserActionPending(false);
      setConfirmingWithdraw(false);
    }
  };

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
        {isUser ? editing ? (
          <textarea
            aria-label="修改消息内容"
            value={editValue}
            onChange={(event) => setEditValue(event.target.value)}
            rows={Math.min(6, Math.max(2, editValue.split('\n').length))}
            autoFocus
            disabled={userActionPending}
            style={editTextareaStyle}
          />
        ) : (
          <span style={{ whiteSpace: 'pre-wrap' }}>{message.content}</span>
        ) : (
          <>
            <MarkdownContent content={message.content} />
          </>
        )}
      </div>

      {canModifyUserMessage && (
        <div style={userActionRowStyle}>
          {editing ? (
            <>
              <button
                type="button"
                aria-label="取消修改"
                title="取消修改"
                onClick={() => setEditing(false)}
                disabled={userActionPending}
                style={messageIconButtonStyle}
              >
                <CloseIcon size={14} />
              </button>
              <button
                type="button"
                aria-label="保存修改"
                title="保存修改"
                onClick={() => void saveEdit()}
                disabled={userActionPending || !editValue.trim()}
                style={messageIconButtonStyle}
              >
                <CheckIcon size={14} />
              </button>
            </>
          ) : confirmingWithdraw ? (
            <>
              <span style={{ fontSize: 12, color: 'var(--text-secondary)' }}>确认撤回这条消息？</span>
              <button
                type="button"
                onClick={() => setConfirmingWithdraw(false)}
                disabled={userActionPending}
                style={messageTextButtonStyle}
              >
                取消
              </button>
              <button
                type="button"
                onClick={() => void confirmWithdraw()}
                disabled={userActionPending}
                style={{ ...messageTextButtonStyle, color: 'var(--danger)' }}
              >
                确认撤回
              </button>
            </>
          ) : (
            <>
              <button
                type="button"
                aria-label="修改消息"
                title="修改消息"
                onClick={startEditing}
                disabled={actionsDisabled}
                style={messageIconButtonStyle}
              >
                <PenIcon size={14} />
              </button>
              <button
                type="button"
                aria-label="撤回消息"
                title="撤回消息"
                onClick={() => {
                  setUserActionError(null);
                  setConfirmingWithdraw(true);
                }}
                disabled={actionsDisabled}
                style={messageIconButtonStyle}
              >
                <UndoIcon size={14} />
              </button>
            </>
          )}
          {userActionError && (
            <span role="alert" style={{ fontSize: 12, color: 'var(--danger)' }}>{userActionError}</span>
          )}
        </div>
      )}

      {hasEnhance && message.enhance && (
        <EnhanceMeta enhance={message.enhance} />
      )}

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

// ── Enhancement Meta ──────────────────────────────────────────────────

function EnhanceMeta({ enhance }: { enhance: EnhanceState }) {
  const count = enhance.members.length;
  const memberLabels = enhance.members.map((member) =>
    member.status === 'failed' ? `${member.label}（失败）` : member.label,
  );
  const phaseLabel = enhance.phase === 'done'
    ? '增强输出'
    : enhance.phase === 'aggregating'
      ? '正在综合'
      : '正在增强';
  const memberText = memberLabels.length > 0
    ? memberLabels.join('、')
    : count > 0 ? `${count} 个模型` : '等待模型响应';

  return (
    <div style={enhanceMetaStyle}>
      <span style={enhanceMetaTitleStyle}>
        <StarClusterIcon size={12} />
        {phaseLabel}
      </span>
      <span style={enhanceMetaTextStyle}>参与：{memberText}</span>
      {enhance.aggregatorLabel && (
        <span style={enhanceMetaTextStyle}>整合：{enhance.aggregatorLabel}</span>
      )}
    </div>
  );
}

const enhanceMetaStyle: React.CSSProperties = {
  maxWidth: '78%',
  display: 'flex',
  alignItems: 'center',
  flexWrap: 'wrap',
  gap: '4px 10px',
  color: 'var(--text-secondary)',
  fontSize: 12,
  lineHeight: 1.5,
};

const enhanceMetaTitleStyle: React.CSSProperties = {
  display: 'inline-flex',
  alignItems: 'center',
  gap: 4,
  color: 'var(--accent)',
  fontWeight: 650,
};

const enhanceMetaTextStyle: React.CSSProperties = {
  minWidth: 0,
  overflowWrap: 'anywhere',
};

const editTextareaStyle: React.CSSProperties = {
  display: 'block',
  width: 'min(440px, 58vw)',
  minWidth: 180,
  maxWidth: '100%',
  padding: 0,
  border: 0,
  outline: 0,
  resize: 'vertical',
  background: 'transparent',
  color: '#fff',
  font: 'inherit',
  lineHeight: 1.5,
};

const userActionRowStyle: React.CSSProperties = {
  minHeight: 26,
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'flex-end',
  gap: 6,
};

const messageIconButtonStyle: React.CSSProperties = {
  width: 26,
  height: 26,
  display: 'grid',
  placeItems: 'center',
  border: '1px solid var(--border)',
  borderRadius: '50%',
  background: 'transparent',
  color: 'var(--text-secondary)',
  cursor: 'pointer',
};

const messageTextButtonStyle: React.CSSProperties = {
  border: 0,
  padding: '3px 5px',
  background: 'transparent',
  color: 'var(--text-secondary)',
  fontSize: 12,
  cursor: 'pointer',
};

// ── Permission Grant Button ──────────────────────────────────────────

interface PermissionRequest {
  kind: string;
  target: string;
  reason: string;
}

function PermissionGrantButton({ requests }: { requests: PermissionRequest[] }) {
  const [granted, setGranted] = useState<Set<string>>(new Set());
  const [unavailable, setUnavailable] = useState<Set<string>>(new Set());
  const [loading, setLoading] = useState(false);

  const handleGrant = async (req: PermissionRequest) => {
    setLoading(true);
    try {
      const capabilities = await getPlatformCapabilities();
      const supported = req.kind === 'accessibility'
        ? capabilities.screenReader
        : req.kind === 'open_app'
          ? capabilities.applicationAutomation
          : req.kind === 'open_url'
            ? capabilities.urlOpener
            : req.kind === 'open_file'
              ? capabilities.pathOpener
              : true;
      if (!supported) {
        setUnavailable((targets) => new Set(targets).add(req.target));
        return;
      }

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
        try {
          await invokeTauri('request_accessibility_permission');
        } catch (e) {
          console.error('[permission] accessibility prompt failed:', e);
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
          disabled={loading || granted.has(req.target) || unavailable.has(req.target)}
          onClick={() => handleGrant(req)}
          style={{
            padding: '6px 14px',
            fontSize: 13,
            border: '1px solid var(--accent, #0a73ff)',
            borderRadius: 6,
            background: granted.has(req.target) ? '#4caf50' : 'var(--accent, #0a73ff)',
            color: '#fff',
            cursor: granted.has(req.target) || unavailable.has(req.target) ? 'default' : 'pointer',
            opacity: loading ? 0.6 : 1,
          }}
        >
          {granted.has(req.target)
            ? <CheckIcon size={14} style={{ verticalAlign: '-2px', marginRight: 6 }} />
            : <UnlockIcon size={14} style={{ verticalAlign: '-2px', marginRight: 6 }} />}
          {unavailable.has(req.target)
            ? req.kind === 'accessibility'
              ? '当前平台暂不支持屏幕识别'
              : '当前平台暂不支持此系统操作'
            : req.kind === 'accessibility'
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
