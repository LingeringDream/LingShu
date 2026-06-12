/* global CustomEvent, StorageEvent */
import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import type { Message } from '../components/chat/MessageBubble';
import { apiFetch } from '../lib/api';
import { deleteAppleCalendarEvent } from '../lib/eventkit';
import { runAutomationAction, readScreen } from '../lib/automation';
import { isTauri } from '../lib/tauri';

export const CHAT_SESSION_STORAGE_KEY = 'lingshu-chat-session';
export const CHAT_SESSION_SYNC_EVENT = 'lingshu:chat-session-sync';
export const CHAT_SESSION_SYNC_DEBOUNCE_MS = 80;

const CHAT_SYNC_SOURCE_ID = Math.random().toString(36).slice(2);

interface ChatSessionSnapshot {
  sourceId?: string;
  messages: Message[];
  isLoading?: boolean;
  streamingId?: string | null;
  sessionId: string | null;
}

interface ChatState {
  messages: Message[];
  isLoading: boolean;
  streamingId: string | null;
  /** Active conversation id — created lazily on first message so the backend
   *  persists the exchange and can replay history on later turns. */
  sessionId: string | null;
  sendMessage: (content: string) => Promise<void>;
  clearMessages: () => void;
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
  const permissionRequests = normalizePermissionRequests(value.permissionRequests);
  if (permissionRequests && permissionRequests.length > 0) {
    message.permissionRequests = permissionRequests;
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
  };
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

export const useChatStore = create<ChatState>()(
  persist(
    (set, get) => ({
      messages: [],
      isLoading: false,
      streamingId: null,
      sessionId: null,

      sendMessage: async (content: string) => {
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

          const baseBody: Record<string, unknown> = sessionId
            ? { message: content, session_id: sessionId }
            : { message: content };

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

      // Start a fresh conversation: drop the session so the next message creates
      // a new one rather than appending to the previous thread.
      clearMessages: () => set({ messages: [], sessionId: null }),
    }),
    {
      name: CHAT_SESSION_STORAGE_KEY,
      partialize: (state) => ({
        messages: state.messages,
        sessionId: state.sessionId,
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
  };
}

function applyChatSessionSnapshot(value: unknown) {
  const snapshot = normalizeChatSessionSnapshot(value);
  if (!snapshot || snapshot.sourceId === CHAT_SYNC_SOURCE_ID) return;

  const current = useChatStore.getState();
  applyingRemoteSnapshot = true;
  try {
    useChatStore.setState({
      messages: snapshot.messages,
      isLoading: snapshot.isLoading ?? current.isLoading,
      streamingId: snapshot.streamingId ?? null,
      sessionId: snapshot.sessionId,
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
