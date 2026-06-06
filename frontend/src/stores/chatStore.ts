import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import type { Message } from '../components/chat/MessageBubble';
import { apiFetch } from '../lib/api';

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

/** Ensure a conversation exists, creating one on first use. Returns the
 *  conversation id, or `null` if creation failed (chat still proceeds as an
 *  ephemeral, non-persisted exchange in that case). */
async function ensureSessionId(
  current: string | null,
  setSessionId: (id: string) => void,
): Promise<string | null> {
  if (current) return current;
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

          const response = await apiFetch('/api/v1/chat', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(
              sessionId ? { message: content, session_id: sessionId } : { message: content },
            ),
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
                    if (data.done) {
                      // Streaming complete
                      set({ isLoading: false, streamingId: null });
                      return;
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
        } catch (error) {
          set((state) => ({
            isLoading: false,
            streamingId: null,
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
      name: 'lingshu-chat-session',
      partialize: (state) => ({
        messages: state.messages,
        sessionId: state.sessionId,
      }),
    },
  ),
);
