import { create } from 'zustand';
import type { Message } from '../components/chat/MessageBubble';
import { apiFetch } from '../lib/api';

interface ChatState {
  messages: Message[];
  isLoading: boolean;
  streamingId: string | null;
  sendMessage: (content: string) => Promise<void>;
  clearMessages: () => void;
}

export const useChatStore = create<ChatState>((set) => ({
  messages: [],
  isLoading: false,
  streamingId: null,

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
      const response = await apiFetch('/api/v1/chat', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ message: content }),
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

  clearMessages: () => set({ messages: [] }),
}));
