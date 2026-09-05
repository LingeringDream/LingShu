/* global StorageEvent */
import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import type {
  EnhanceMember,
  EnhanceState,
  FeedbackStyleTag,
  Message,
} from '../components/chat/MessageBubble';
import { apiFetch } from '../lib/api';
import { deleteAppleCalendarEvent } from '../lib/eventkit';
import { runAutomationAction, readScreen } from '../lib/automation';
import { getPlatformCapabilities, isTauri } from '../lib/tauri';

export const CHAT_SESSION_STORAGE_KEY = 'lingshu-chat-session';
export const CHAT_SESSION_SYNC_EVENT = 'lingshu:chat-session-sync';
export const CHAT_SESSION_SYNC_DEBOUNCE_MS = 80;

const CHAT_SYNC_SOURCE_ID = Math.random().toString(36).slice(2);

function nextMessagesRevision(current: number): number {
  return Math.max(current + 1, Date.now());
}

interface ChatSessionSnapshot {
  sourceId?: string;
  messages: Message[];
  isLoading?: boolean;
  streamingId?: string | null;
  sessionId: string | null;
  messagesRevision?: number;
  enhanceEnabled?: boolean;
  enhanceDefaultLoaded?: boolean;
}

export interface EnhanceMemberConfig {
  provider_id: string;
  model: string;
}

export interface EnhanceRequestOverride {
  members?: EnhanceMemberConfig[];
  aggregator?: EnhanceMemberConfig | null;
  per_model_timeout_ms?: number;
  max_parallel?: number;
}

interface ChatState {
  messages: Message[];
  /** Increments when a whole message set is intentionally replaced (load,
   *  withdraw, clear) so another window cannot merge removed messages back. */
  messagesRevision: number;
  isLoading: boolean;
  streamingId: string | null;
  /** Active conversation id — created lazily on first message so the backend
   *  persists the exchange and can replay history on later turns. */
  sessionId: string | null;
  enhanceEnabled: boolean;
  enhanceDefaultLoaded: boolean;
  setEnhanceEnabled: (enabled: boolean) => void;
  loadEnhanceDefault: () => Promise<void>;
  sendMessage: (content: string, enhanceOverride?: EnhanceRequestOverride) => Promise<void>;
  regenerateAssistantMessage: (messageId: string, styleTag: FeedbackStyleTag) => Promise<void>;
  loadSession: (sessionId: string) => Promise<boolean>;
  editUserMessage: (messageId: string, content: string) => Promise<void>;
  withdrawUserMessage: (messageId: string) => Promise<void>;
  clearMessages: () => void;
}

interface StoredChatMessage {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  model_id?: string | null;
  created_at: string;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

function normalizeTimestamp(value: unknown): Date {
  if (value instanceof Date && !Number.isNaN(value.getTime())) return value;
  if (typeof value === 'string' || typeof value === 'number') {
    const parsed = new Date(value);
    if (!Number.isNaN(parsed.getTime())) return parsed;
  }
  return new Date();
}

function normalizePermissionRequests(value: unknown): Message['permissionRequests'] {
  if (!Array.isArray(value)) return undefined;
  return value
    .filter(isRecord)
    .map((request) => ({
      kind: typeof request.kind === 'string' ? request.kind : '',
      target: typeof request.target === 'string' ? request.target : '',
      reason: typeof request.reason === 'string' ? request.reason : '',
    }))
    .filter((request) => request.kind && request.target && request.reason);
}

function normalizeEnhanceMember(value: unknown): EnhanceMember | null {
  if (!isRecord(value)) return null;
  if (typeof value.id !== 'string' || typeof value.label !== 'string') return null;
  if (value.status !== 'thinking' && value.status !== 'done' && value.status !== 'failed') return null;
  return {
    id: value.id,
    label: value.label,
    provider: typeof value.provider === 'string' ? value.provider : '',
    status: value.status,
    draft: typeof value.draft === 'string' ? value.draft : undefined,
    error: typeof value.error === 'string' ? value.error : undefined,
  };
}

function normalizeEnhanceState(value: unknown): EnhanceState | undefined {
  if (!isRecord(value)) return undefined;
  if (value.phase !== 'drafting' && value.phase !== 'aggregating' && value.phase !== 'done') {
    return undefined;
  }
  const members = Array.isArray(value.members)
    ? value.members.map(normalizeEnhanceMember).filter((member): member is EnhanceMember => member !== null)
    : [];
  return {
    phase: value.phase,
    members,
    aggregatorLabel: typeof value.aggregatorLabel === 'string' ? value.aggregatorLabel : undefined,
  };
}

function normalizeMessage(value: unknown): Message | null {
  if (!isRecord(value)) return null;
  if (value.role !== 'user' && value.role !== 'assistant') return null;
  if (typeof value.id !== 'string' || typeof value.content !== 'string') return null;

  const message: Message = {
    id: value.id,
    role: value.role,
    content: value.content,
    timestamp: normalizeTimestamp(value.timestamp),
  };
  if (typeof value.dbId === 'string') message.dbId = value.dbId;
  if (typeof value.revision === 'number' && Number.isFinite(value.revision)) {
    message.revision = value.revision;
  }
  const permissionRequests = normalizePermissionRequests(value.permissionRequests);
  if (permissionRequests && permissionRequests.length > 0) {
    message.permissionRequests = permissionRequests;
  }
  if (typeof value.enhancing === 'boolean') {
    message.enhancing = value.enhancing;
  }
  const enhance = normalizeEnhanceState(value.enhance);
  if (enhance) {
    message.enhance = enhance;
  }
  return message;
}

function normalizeChatSessionSnapshot(value: unknown): ChatSessionSnapshot | null {
  if (!isRecord(value)) return null;
  const state = isRecord(value.state) ? value.state : value;
  const messages = Array.isArray(state.messages)
    ? state.messages.map(normalizeMessage).filter((message): message is Message => message !== null)
    : [];

  return {
    sourceId: typeof state.sourceId === 'string'
      ? state.sourceId
      : typeof value.sourceId === 'string'
        ? value.sourceId
        : undefined,
    messages,
    isLoading: typeof state.isLoading === 'boolean' ? state.isLoading : undefined,
    streamingId: typeof state.streamingId === 'string' ? state.streamingId : null,
    sessionId: typeof state.sessionId === 'string' ? state.sessionId : null,
    messagesRevision: typeof state.messagesRevision === 'number' && Number.isFinite(state.messagesRevision)
      ? state.messagesRevision
      : 0,
    enhanceEnabled: typeof state.enhanceEnabled === 'boolean' ? state.enhanceEnabled : undefined,
    enhanceDefaultLoaded: typeof state.enhanceDefaultLoaded === 'boolean' ? state.enhanceDefaultLoaded : undefined,
  };
}

function canMergeChatSessionSnapshot(currentSessionId: string | null, nextSessionId: string | null): boolean {
  return currentSessionId === nextSessionId || currentSessionId === null || nextSessionId === null;
}

function chooseMessageWithMostContent(current: Message | undefined, incoming: Message): Message {
  if (!current || current.role !== incoming.role) return incoming;

  const currentRevision = current.revision ?? 0;
  const incomingRevision = incoming.revision ?? 0;
  const primary = currentRevision !== incomingRevision
    ? currentRevision > incomingRevision ? current : incoming
    : current.content.length > incoming.content.length ? current : incoming;
  const fallback = primary === current ? incoming : current;
  const merged: Message = {
    ...primary,
    dbId: primary.dbId ?? fallback.dbId,
    revision: Math.max(primary.revision ?? 0, fallback.revision ?? 0),
  };

  if (primary.permissionRequests && primary.permissionRequests.length > 0) {
    merged.permissionRequests = primary.permissionRequests;
  } else if (fallback.permissionRequests && fallback.permissionRequests.length > 0) {
    merged.permissionRequests = fallback.permissionRequests;
  }
  merged.enhancing = primary.enhancing ?? fallback.enhancing;
  merged.enhance = primary.enhance ?? fallback.enhance;

  return merged;
}

function mergeChatSessionMessages(currentMessages: Message[], incomingMessages: Message[]): Message[] {
  if (incomingMessages.length === 0) return [];

  const currentById = new Map(currentMessages.map((message) => [message.id, message]));
  const incomingIds = new Set(incomingMessages.map((message) => message.id));
  const merged = incomingMessages.map((message) =>
    chooseMessageWithMostContent(currentById.get(message.id), message),
  );

  for (const message of currentMessages) {
    if (!incomingIds.has(message.id)) merged.push(message);
  }

  return merged;
}

function snapshotWouldRegressCompletedStream(current: ChatState, snapshot: ChatSessionSnapshot): boolean {
  if (current.isLoading || snapshot.isLoading !== true || !snapshot.streamingId) return false;

  const currentMessage = current.messages.find((message) => message.id === snapshot.streamingId);
  if (!currentMessage || currentMessage.role !== 'assistant') return false;
  if (currentMessage.dbId) return true;

  const incomingMessage = snapshot.messages.find((message) => message.id === snapshot.streamingId);
  return Boolean(incomingMessage && currentMessage.content.length > incomingMessage.content.length);
}

/** Ensure a conversation exists, creating one on first use. Returns the
 *  conversation id, or `null` if creation failed (chat still proceeds as an
 *  ephemeral, non-persisted exchange in that case). */
async function ensureSessionId(
  current: string | null,
  setSessionId: (id: string | null) => void,
): Promise<string | null> {
  if (current) {
    // Verify the persisted session still exists on the backend.
    // If the database was reset or the session was deleted, discard it
    // and create a fresh one so the chat doesn't fail with a 404.
    try {
      const resp = await apiFetch(`/api/v1/chat/sessions/${current}`);
      if (resp.ok) return current;
    } catch {
      // validation failed — fall through to create a new session below
    }
    // Stale session — discard and create a new one
    setSessionId(null);
  }
  try {
    const resp = await apiFetch('/api/v1/conversations', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({}),
    });
    if (!resp.ok) return null;
    const data: { id: string } = await resp.json();
    setSessionId(data.id);
    return data.id;
  } catch {
    return null;
  }
}

function regenerationStatus(styleTag: FeedbackStyleTag): string {
  switch (styleTag) {
    case 'too_long':
      return '（正在按“更精简”重写…）';
    case 'too_short':
      return '（正在按“更展开”重写…）';
    case 'too_formal':
      return '（正在按“更口语”重写…）';
  }
}

function shouldBypassEnhancementForToolTurn(content: string): boolean {
  const text = content.trim().toLowerCase();
  if (!text) return false;

  return [
    '重试',
    '再试一次',
    '重新来',
    '重新搜',
    '重新搜索',
    'retry',
    '搜索',
    '搜一下',
    '上网',
    '联网',
    '查一下',
    '帮我查',
    '最新',
    '新闻',
    '价格',
    '政策',
    '版本',
    '日程',
    '提醒',
    '安排',
    '创建',
    '删除',
    '取消',
    '打开',
    '启动',
    '屏幕识别',
    '屏幕阅读',
    '读取屏幕',
    '看屏幕',
    'read screen',
    'open ',
    'search',
  ].some((keyword) => text.includes(keyword));
}

export const useChatStore = create<ChatState>()(
  persist(
    (set, get) => ({
      messages: [],
      messagesRevision: 0,
      isLoading: false,
      streamingId: null,
      sessionId: null,
      enhanceEnabled: false,
      enhanceDefaultLoaded: false,

      setEnhanceEnabled: (enabled: boolean) => set({
        enhanceEnabled: enabled,
        enhanceDefaultLoaded: true,
      }),

      loadEnhanceDefault: async () => {
        if (get().enhanceDefaultLoaded) return;
        try {
          const resp = await apiFetch('/api/v1/settings/enhance');
          if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
          const data = await resp.json();
          if (get().enhanceDefaultLoaded) return;
          set({
            enhanceEnabled: Boolean(data.enabled_default),
            enhanceDefaultLoaded: true,
          });
        } catch {
          if (get().enhanceDefaultLoaded) return;
          set({ enhanceDefaultLoaded: true });
        }
      },

      sendMessage: async (content: string, enhanceOverride?: EnhanceRequestOverride) => {
        const enhanceActive = get().enhanceEnabled && !shouldBypassEnhancementForToolTurn(content);
        const userMessage: Message = {
          id: crypto.randomUUID(),
          role: 'user',
          content,
          timestamp: new Date(),
        };

        // Placeholder for the assistant response — updated incrementally during streaming
        const assistantId = crypto.randomUUID();
        const assistantMessage: Message = {
          id: assistantId,
          role: 'assistant',
          content: '',
          timestamp: new Date(),
          enhancing: enhanceActive,
          enhance: enhanceActive ? { phase: 'drafting', members: [] } : undefined,
        };

        set((state) => ({
          messages: [...state.messages, userMessage, assistantMessage],
          isLoading: true,
          streamingId: assistantId,
        }));

        try {
          // Lazily create/reuse a conversation so the backend persists messages
          // and can supply prior-turn history as context.
          const sessionId = await ensureSessionId(get().sessionId, (id) =>
            set({ sessionId: id }),
          );
          const clientPlatform = (await getPlatformCapabilities()).platform;

          const baseBody: Record<string, unknown> = sessionId
            ? { message: content, session_id: sessionId, client_platform: clientPlatform }
            : { message: content, client_platform: clientPlatform };

          const updateAssistantMessage = (updater: (message: Message) => Message) => {
            set((state) => ({
              messages: state.messages.map((message) =>
                message.id === assistantId ? updater(message) : message,
              ),
            }));
          };

          const runEnhancedChatStream = async (
            body: Record<string, unknown>,
          ): Promise<void> => {
            const response = await apiFetch('/api/v1/chat/enhance', {
              method: 'POST',
              headers: { 'Content-Type': 'application/json' },
              body: JSON.stringify(body),
            });

            if (!response.ok) {
              throw new Error(`HTTP ${response.status}`);
            }

            const applyEnhanceEvent = (data: unknown): boolean => {
              if (!isRecord(data) || typeof data.type !== 'string') return false;

              switch (data.type) {
                case 'start': {
                  const members = Array.isArray(data.members)
                    ? data.members.filter(isRecord).map((member, index): EnhanceMember => {
                      const id = typeof member.id === 'string' ? member.id : String(index);
                      return {
                        id,
                        label: typeof member.label === 'string' ? member.label : id,
                        provider: typeof member.provider_id === 'string'
                          ? member.provider_id
                          : typeof member.provider === 'string'
                            ? member.provider
                            : '',
                        status: 'thinking',
                      };
                    })
                    : [];
                  const aggregatorLabel = typeof data.aggregator === 'string' ? data.aggregator : undefined;
                  updateAssistantMessage((message) => ({
                    ...message,
                    enhancing: true,
                    enhance: { phase: 'drafting', members, aggregatorLabel },
                  }));
                  return false;
                }
                case 'candidate': {
                  const id = typeof data.id === 'string' ? data.id : '';
                  if (!id) return false;
                  const status: EnhanceMember['status'] =
                    data.status === 'done' ? 'done' : data.status === 'failed' ? 'failed' : 'thinking';
                  const draft = typeof data.draft === 'string' ? data.draft : undefined;
                  const error = typeof data.error === 'string' ? data.error : undefined;
                  updateAssistantMessage((message) => {
                    const current = message.enhance ?? { phase: 'drafting', members: [] };
                    const hasMember = current.members.some((member) => member.id === id);
                    const members = hasMember
                      ? current.members.map((member) =>
                        member.id === id ? { ...member, status, draft, error } : member,
                      )
                      : [...current.members, { id, label: id, provider: '', status, draft, error }];
                    return {
                      ...message,
                      enhancing: true,
                      enhance: { ...current, members },
                    };
                  });
                  return false;
                }
                case 'aggregate': {
                  updateAssistantMessage((message) => {
                    const current = message.enhance ?? { phase: 'drafting', members: [] };
                    return {
                      ...message,
                      enhancing: true,
                      enhance: { ...current, phase: 'aggregating' },
                    };
                  });
                  return false;
                }
                case 'content': {
                  if (typeof data.content !== 'string' || data.content.length === 0) return false;
                  updateAssistantMessage((message) => ({ ...message, content: message.content + data.content }));
                  return false;
                }
                case 'error': {
                  const messageText = typeof data.message === 'string' ? data.message : '增强输出失败。';
                  updateAssistantMessage((message) => ({
                    ...message,
                    content: message.content.trim()
                      ? `${message.content}\n\n错误: ${messageText}`
                      : `错误: ${messageText}`,
                  }));
                  return false;
                }
                case 'done': {
                  const dbId = typeof data.assistant_message_id === 'string'
                    ? data.assistant_message_id
                    : undefined;
                  const userDbId = typeof data.user_message_id === 'string'
                    ? data.user_message_id
                    : undefined;
                  set((state) => ({
                    isLoading: false,
                    streamingId: null,
                    messages: state.messages.map((message) => {
                      if (message.id === userMessage.id && userDbId) {
                        return { ...message, dbId: userDbId };
                      }
                      if (message.id !== assistantId) return message;
                      const current = message.enhance ?? { phase: 'done', members: [] };
                      return {
                        ...message,
                        dbId,
                        enhancing: false,
                        enhance: { ...current, phase: 'done' },
                      };
                    }),
                  }));
                  return true;
                }
                default:
                  return false;
              }
            };

            const reader = response.body?.getReader();
            const decoder = new TextDecoder();
            let buffer = '';

            if (reader) {
              while (true) {
                const { done, value } = await reader.read();
                if (done) break;

                buffer += decoder.decode(value, { stream: true });

                while (true) {
                  const newlineIdx = buffer.indexOf('\n');
                  if (newlineIdx === -1) break;

                  const line = buffer.slice(0, newlineIdx).trim();
                  buffer = buffer.slice(newlineIdx + 1);

                  if (!line.startsWith('data:')) continue;

                  try {
                    const finished = applyEnhanceEvent(JSON.parse(line.slice(5).trimStart()));
                    if (finished) return;
                  } catch {
                    // Skip malformed JSON lines
                  }
                }
              }
            }

            set((state) => ({
              isLoading: false,
              streamingId: null,
              messages: state.messages.map((message) => {
                if (message.id !== assistantId) return message;
                const current = message.enhance ?? { phase: 'done', members: [] };
                return {
                  ...message,
                  enhancing: false,
                  content: message.content || '（无响应）',
                  enhance: { ...current, phase: 'done' },
                };
              }),
            }));
          };

          // Run one chat request and stream its SSE response into the assistant
          // bubble. Returns 'screen_read' when the backend asked the desktop
          // client to capture screen text (the read_screen handoff): the caller
          // then captures it in the authorized main-app process and runs a
          // second pass with `screen_context`. Returns 'done' otherwise.
          const runChatStream = async (
            body: Record<string, unknown>,
          ): Promise<'screen_read' | 'done'> => {
            const response = await apiFetch('/api/v1/chat', {
              method: 'POST',
              headers: { 'Content-Type': 'application/json' },
              body: JSON.stringify(body),
            });

            if (!response.ok) {
              throw new Error(`HTTP ${response.status}`);
            }

            // Read SSE stream and update the assistant message incrementally
            const reader = response.body?.getReader();
            const decoder = new TextDecoder();
            let buffer = '';

            if (reader) {
              while (true) {
                const { done, value } = await reader.read();
                if (done) break;

                buffer += decoder.decode(value, { stream: true });

                // Parse complete SSE events from the buffer
                while (true) {
                  const newlineIdx = buffer.indexOf('\n');
                  if (newlineIdx === -1) break;

                  const line = buffer.slice(0, newlineIdx).trim();
                  buffer = buffer.slice(newlineIdx + 1);

                  if (line.startsWith('data: ')) {
                    try {
                      const data = JSON.parse(line.slice(6));
                      if (data.content) {
                        // Update the assistant message in-place
                        set((state) => ({
                          messages: state.messages.map((m) =>
                            m.id === assistantId
                              ? { ...m, content: m.content + data.content }
                              : m,
                          ),
                        }));
                      }
                      // Apple Calendar delete sync: when the backend deletes a
                      // calendar event via the chat tool, it includes any EventKit
                      // eventIdentifiers that the frontend needs to remove from the
                      // system calendar.
                      if (data.apple_calendar_deletes && Array.isArray(data.apple_calendar_deletes)) {
                        for (const id of data.apple_calendar_deletes) {
                          deleteAppleCalendarEvent(id).catch(() => {});
                        }
                        // Calendar was modified — notify components to refresh
                        window.dispatchEvent(new CustomEvent('calendar-changed'));
                      }
                      // L2 permission requests: show inline grant button.
                      // Store on the message so MessageBubble can render it.
                      if (data.permission_requests && Array.isArray(data.permission_requests)) {
                        set((state) => ({
                          messages: state.messages.map((m) =>
                            m.id === assistantId
                              ? { ...m, permissionRequests: data.permission_requests }
                              : m,
                          ),
                        }));
                      }
                      // L2 automation actions (open app/url/file) approved by the
                      // backend — forward each to its Tauri command.
                      if (data.automation_actions && Array.isArray(data.automation_actions)) {
                        for (const action of data.automation_actions) {
                          runAutomationAction(action).catch(() => {});
                        }
                      }
                      if (data.done) {
                        const userDbId: string | undefined = data.user_message_id;
                        if (userDbId) {
                          set((state) => ({
                            messages: state.messages.map((m) =>
                              m.id === userMessage.id ? { ...m, dbId: userDbId } : m,
                            ),
                          }));
                        }
                        // Screen-read handoff: do NOT finalize — let the caller
                        // capture screen text and run a second pass.
                        if (data.screen_read_request) {
                          return 'screen_read';
                        }
                        // Streaming complete — capture the backend message id for feedback
                        const dbId: string | undefined = data.assistant_message_id;
                        set((state) => ({
                          isLoading: false,
                          streamingId: null,
                          messages: state.messages.map((m) =>
                            m.id === assistantId
                              ? { ...m, dbId }
                              : m,
                          ),
                        }));
                        return 'done';
                      }
                    } catch {
                      // Skip malformed JSON lines
                    }
                  }
                }
              }
            }

            // If we exit without a done marker, mark complete anyway
            set((state) => ({
              isLoading: false,
              streamingId: null,
              messages: state.messages.map((m) =>
                m.id === assistantId && m.content === ''
                  ? { ...m, content: '（无响应）' }
                  : m,
              ),
            }));
            return 'done';
          };

          if (enhanceActive) {
            await runEnhancedChatStream(enhanceOverride ? { ...baseBody, enhance: enhanceOverride } : baseBody);
            return;
          }

          const outcome = await runChatStream(baseBody);
          if (outcome === 'screen_read') {
            // The model asked to read the screen and L3 is granted. Capture the
            // frontmost window's text in the MAIN-app process (com.lingshu.desktop
            // — the TCC subject the user can actually authorize), then resend so
            // the model answers using it.
            set((state) => ({
              messages: state.messages.map((m) =>
                m.id === assistantId ? { ...m, content: '（正在读取屏幕…）' } : m,
              ),
            }));
            let screenText = '';
            try {
              screenText = await readScreen();
            } catch (e) {
              set((state) => ({
                isLoading: false,
                streamingId: null,
                messages: state.messages.map((m) =>
                  m.id === assistantId
                    ? {
                        ...m,
                        content: e instanceof Error ? e.message : '无法读取屏幕。',
                        permissionRequests: [
                          {
                            kind: 'accessibility',
                            target: '屏幕识别',
                            reason: '需要在系统设置中为「灵枢」开启辅助功能',
                          },
                        ],
                      }
                    : m,
                ),
              }));
              return;
            }
            if (!screenText || screenText.trim().length === 0) {
              set((state) => ({
                isLoading: false,
                streamingId: null,
                messages: state.messages.map((m) =>
                  m.id === assistantId
                    ? { ...m, content: '未能读取到屏幕文字（前台窗口可能没有可读文本元素）。请切换到有文字内容的窗口后重试。' }
                    : m,
                ),
              }));
              return;
            }
            // Clear transient status, then run the second pass.
            set((state) => ({
              messages: state.messages.map((m) =>
                m.id === assistantId ? { ...m, content: '' } : m,
              ),
            }));
            const secondOutcome = await runChatStream({ ...baseBody, screen_context: screenText });
            // If the second pass ended without streaming any content (model error,
            // empty response, etc.), surface an error so the bubble isn't blank.
            if (secondOutcome !== 'done') {
              const current = get().messages.find((m) => m.id === assistantId);
              if (!current || current.content.trim() === '') {
                set((state) => ({
                  isLoading: false,
                  streamingId: null,
                  messages: state.messages.map((m) =>
                    m.id === assistantId
                      ? { ...m, content: '模型未返回响应。请重试或检查模型配置。' }
                      : m,
                  ),
                }));
              }
            }
          }
        } catch (error) {
          const is404 = error instanceof Error && error.message === 'HTTP 404';
          set((state) => ({
            isLoading: false,
            streamingId: null,
            // If the session was not found (expired / DB reset), clear it
            // so the next message creates a fresh one automatically.
            sessionId: is404 ? null : state.sessionId,
            messages: state.messages.map((m) =>
              m.id === assistantId && m.content === ''
                ? {
                    ...m,
                    content: `错误: ${error instanceof Error ? error.message : '未知错误'}`,
                  }
                : m,
            ),
          }));
        }
      },

      regenerateAssistantMessage: async (messageId: string, styleTag: FeedbackStyleTag) => {
        if (get().isLoading) return;

        const target = get().messages.find((message) =>
          message.id === messageId && message.role === 'assistant' && message.dbId,
        );
        if (!target?.dbId) return;

        const targetDbId = target.dbId;
        const previousContent = target.content;
        const previousPermissionRequests = target.permissionRequests;
        const nextRevision = (target.revision ?? 0) + 1;

        set((state) => ({
          isLoading: true,
          streamingId: messageId,
          messages: state.messages.map((message) =>
            message.id === messageId
              ? {
                  ...message,
                  content: regenerationStatus(styleTag),
                  dbId: undefined,
                  permissionRequests: undefined,
                  revision: nextRevision,
                }
              : message,
          ),
        }));

        try {
          const clientPlatform = (await getPlatformCapabilities()).platform;
          const response = await apiFetch(`/api/v1/chat/messages/${targetDbId}/regenerate`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ style_tag: styleTag, client_platform: clientPlatform }),
          });

          if (!response.ok) {
            throw new Error(`HTTP ${response.status}`);
          }

          const reader = response.body?.getReader();
          const decoder = new TextDecoder();
          let buffer = '';
          let regeneratedContent = '';

          if (reader) {
            while (true) {
              const { done, value } = await reader.read();
              if (done) break;

              buffer += decoder.decode(value, { stream: true });

              while (true) {
                const newlineIdx = buffer.indexOf('\n');
                if (newlineIdx === -1) break;

                const line = buffer.slice(0, newlineIdx).trim();
                buffer = buffer.slice(newlineIdx + 1);

                if (!line.startsWith('data: ')) continue;

                try {
                  const data = JSON.parse(line.slice(6));
                  if (data.content) {
                    regeneratedContent += data.content;
                    set((state) => ({
                      messages: state.messages.map((message) =>
                        message.id === messageId
                          ? { ...message, content: regeneratedContent, revision: nextRevision }
                          : message,
                      ),
                    }));
                  }

                  if (data.done) {
                    const dbId = typeof data.assistant_message_id === 'string'
                      ? data.assistant_message_id
                      : targetDbId;
                    set((state) => ({
                      isLoading: false,
                      streamingId: null,
                      messages: state.messages.map((message) =>
                        message.id === messageId
                          ? {
                              ...message,
                              content: regeneratedContent || message.content || '（无响应）',
                              dbId,
                              revision: nextRevision,
                            }
                          : message,
                      ),
                    }));
                    return;
                  }
                } catch {
                  // Skip malformed JSON lines
                }
              }
            }
          }

          set((state) => ({
            isLoading: false,
            streamingId: null,
            messages: state.messages.map((message) =>
              message.id === messageId
                ? {
                    ...message,
                    content: regeneratedContent || '（无响应）',
                    dbId: targetDbId,
                    revision: nextRevision,
                  }
                : message,
            ),
          }));
        } catch (error) {
          set((state) => ({
            isLoading: false,
            streamingId: null,
            messages: state.messages.map((message) =>
              message.id === messageId
                ? {
                    ...message,
                    content: previousContent || `错误: ${error instanceof Error ? error.message : '未知错误'}`,
                    dbId: targetDbId,
                    permissionRequests: previousPermissionRequests,
                    revision: target.revision,
                  }
                : message,
            ),
          }));
        }
      },

      loadSession: async (sessionId: string) => {
        if (get().isLoading) return false;
        set({ isLoading: true, streamingId: null });
        try {
          const response = await apiFetch(`/api/v1/chat/sessions/${sessionId}/messages`);
          if (!response.ok) throw new Error(`HTTP ${response.status}`);
          const rows: StoredChatMessage[] = await response.json();
          const messages: Message[] = rows
            .filter((row) => row.role === 'user' || row.role === 'assistant')
            .map((row) => ({
              id: row.id,
              dbId: row.id,
              role: row.role,
              content: row.content,
              timestamp: normalizeTimestamp(row.created_at),
            }));
          set((state) => ({
            messages,
            messagesRevision: nextMessagesRevision(state.messagesRevision),
            sessionId,
            isLoading: false,
            streamingId: null,
          }));
          return true;
        } catch {
          set({ isLoading: false, streamingId: null });
          return false;
        }
      },

      editUserMessage: async (messageId: string, content: string) => {
        if (get().isLoading) return;
        const trimmed = content.trim();
        const target = get().messages.find((message) => message.id === messageId);
        if (!target || target.role !== 'user' || !target.dbId || !trimmed) return;

        const response = await apiFetch(`/api/v1/chat/messages/${target.dbId}`, {
          method: 'PATCH',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ content: trimmed }),
        });
        if (!response.ok) throw new Error(`HTTP ${response.status}`);

        set((state) => ({
          messages: state.messages.map((message) =>
            message.id === messageId
              ? { ...message, content: trimmed, revision: (message.revision ?? 0) + 1 }
              : message,
          ),
        }));
      },

      withdrawUserMessage: async (messageId: string) => {
        if (get().isLoading) return;
        const target = get().messages.find((message) => message.id === messageId);
        if (!target || target.role !== 'user' || !target.dbId) return;

        const response = await apiFetch(`/api/v1/chat/messages/${target.dbId}`, {
          method: 'DELETE',
        });
        if (!response.ok) throw new Error(`HTTP ${response.status}`);

        set((state) => ({
          messages: state.messages.filter((message) => message.id !== messageId),
          messagesRevision: nextMessagesRevision(state.messagesRevision),
        }));
      },

      // Start a fresh conversation: drop the session so the next message creates
      // a new one rather than appending to the previous thread.
      clearMessages: () => set((state) => ({
        messages: [],
        messagesRevision: nextMessagesRevision(state.messagesRevision),
        sessionId: null,
        isLoading: false,
        streamingId: null,
        enhanceDefaultLoaded: false,
      })),
    }),
    {
      name: CHAT_SESSION_STORAGE_KEY,
      partialize: (state) => ({
        messages: state.messages,
        messagesRevision: state.messagesRevision,
        sessionId: state.sessionId,
        enhanceEnabled: state.enhanceEnabled,
        enhanceDefaultLoaded: state.enhanceDefaultLoaded,
      }),
    },
  ),
);

let applyingRemoteSnapshot = false;
let pendingSyncTimer: ReturnType<typeof setTimeout> | null = null;
let pendingSyncSnapshot: ChatSessionSnapshot | null = null;

function createChatSessionSnapshot(state = useChatStore.getState()): ChatSessionSnapshot {
  return {
    sourceId: CHAT_SYNC_SOURCE_ID,
    messages: state.messages,
    isLoading: state.isLoading,
    streamingId: state.streamingId,
    sessionId: state.sessionId,
    messagesRevision: state.messagesRevision,
    enhanceEnabled: state.enhanceEnabled,
    enhanceDefaultLoaded: state.enhanceDefaultLoaded,
  };
}

function applyChatSessionSnapshot(value: unknown) {
  const snapshot = normalizeChatSessionSnapshot(value);
  if (!snapshot || snapshot.sourceId === CHAT_SYNC_SOURCE_ID) return;

  const current = useChatStore.getState();
  const snapshotRevision = snapshot.messagesRevision ?? 0;
  const snapshotIsNewerReplacement = snapshotRevision > current.messagesRevision;
  const snapshotIsStaleReplacement = snapshotRevision < current.messagesRevision;
  const shouldMerge = !snapshotIsNewerReplacement
    && !snapshotIsStaleReplacement
    && snapshot.messages.length > 0
    && current.messages.length > 0
    && canMergeChatSessionSnapshot(current.sessionId, snapshot.sessionId);
  const wouldRegressCompletedStream = shouldMerge && snapshotWouldRegressCompletedStream(current, snapshot);
  applyingRemoteSnapshot = true;
  try {
    useChatStore.setState({
      messages: snapshotIsStaleReplacement
        ? current.messages
        : shouldMerge
          ? mergeChatSessionMessages(current.messages, snapshot.messages)
          : snapshot.messages,
      messagesRevision: Math.max(current.messagesRevision, snapshotRevision),
      isLoading: wouldRegressCompletedStream
        ? current.isLoading
        : snapshot.isLoading ?? current.isLoading,
      streamingId: wouldRegressCompletedStream
        ? current.streamingId
        : snapshot.streamingId ?? null,
      sessionId: snapshotIsStaleReplacement
        ? current.sessionId
        : snapshot.sessionId ?? (snapshot.messages.length > 0 ? current.sessionId : null),
      enhanceEnabled: snapshot.enhanceEnabled ?? current.enhanceEnabled,
      enhanceDefaultLoaded: snapshot.enhanceDefaultLoaded ?? current.enhanceDefaultLoaded,
    });
  } finally {
    applyingRemoteSnapshot = false;
  }
}

function publishChatSessionSnapshot(snapshot: ChatSessionSnapshot) {
  window.dispatchEvent(new CustomEvent(CHAT_SESSION_SYNC_EVENT, { detail: snapshot }));

  if (!isTauri()) return;
  import('@tauri-apps/api/event')
    .then(({ emitTo }) => Promise.all([
      emitTo('main', CHAT_SESSION_SYNC_EVENT, snapshot),
      emitTo('pet', CHAT_SESSION_SYNC_EVENT, snapshot),
    ]))
    .catch((error) => {
      console.error('[chat] failed to emit chat session snapshot:', error);
    });
}

function flushChatSessionSnapshot() {
  if (pendingSyncTimer) {
    clearTimeout(pendingSyncTimer);
    pendingSyncTimer = null;
  }
  const snapshot = pendingSyncSnapshot;
  pendingSyncSnapshot = null;
  if (snapshot) publishChatSessionSnapshot(snapshot);
}

function scheduleChatSessionSnapshot(state = useChatStore.getState()) {
  pendingSyncSnapshot = createChatSessionSnapshot(state);
  if (pendingSyncTimer) return;
  pendingSyncTimer = setTimeout(flushChatSessionSnapshot, CHAT_SESSION_SYNC_DEBOUNCE_MS);
}

export function installChatSessionSync(): () => void {
  let disposed = false;
  let unlistenTauri: (() => void) | null = null;

  const handleCustomEvent = (event: Event) => {
    applyChatSessionSnapshot((event as CustomEvent).detail);
  };

  const handleStorageEvent = (event: StorageEvent) => {
    if (event.key !== CHAT_SESSION_STORAGE_KEY || !event.newValue) return;
    try {
      applyChatSessionSnapshot(JSON.parse(event.newValue));
    } catch {
      // Ignore malformed persisted data; the local store remains authoritative.
    }
  };

  window.addEventListener(CHAT_SESSION_SYNC_EVENT, handleCustomEvent);
  window.addEventListener('storage', handleStorageEvent);

  const unsubscribeStore = useChatStore.subscribe((state) => {
    if (!applyingRemoteSnapshot) scheduleChatSessionSnapshot(state);
  });

  if (isTauri()) {
    import('@tauri-apps/api/event')
      .then(({ listen }) => listen<ChatSessionSnapshot>(CHAT_SESSION_SYNC_EVENT, (event) => {
        applyChatSessionSnapshot(event.payload);
      }))
      .then((unlisten) => {
        if (disposed) {
          unlisten();
        } else {
          unlistenTauri = unlisten;
        }
      })
      .catch((error) => {
        console.error('[chat] failed to listen for chat session snapshots:', error);
      });
  }

  return () => {
    disposed = true;
    window.removeEventListener(CHAT_SESSION_SYNC_EVENT, handleCustomEvent);
    window.removeEventListener('storage', handleStorageEvent);
    unsubscribeStore();
    if (pendingSyncTimer) {
      clearTimeout(pendingSyncTimer);
      pendingSyncTimer = null;
    }
    if (unlistenTauri) unlistenTauri();
  };
}
