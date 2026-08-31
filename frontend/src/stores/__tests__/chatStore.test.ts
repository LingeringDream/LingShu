/* global CustomEvent, TextEncoder, ReadableStream */
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import {
  CHAT_SESSION_SYNC_DEBOUNCE_MS,
  CHAT_SESSION_SYNC_EVENT,
  installChatSessionSync,
  useChatStore,
} from '../chatStore';
import { apiFetch } from '../../lib/api';

vi.mock('../../lib/api', () => ({
  apiFetch: vi.fn(),
}));

const mockedApiFetch = vi.mocked(apiFetch);

function sseResponse(chunks: Array<Record<string, unknown>>): Response {
  const encoder = new TextEncoder();
  return new Response(new ReadableStream({
    start(controller) {
      for (const chunk of chunks) {
        controller.enqueue(encoder.encode(`data: ${JSON.stringify(chunk)}\n\n`));
      }
      controller.close();
    },
  }), { status: 200 });
}

describe('chatStore', () => {
  beforeEach(() => {
    mockedApiFetch.mockReset();
    // Reset the store state between tests
    useChatStore.setState({
      messages: [],
      messagesRevision: 0,
      isLoading: false,
      streamingId: null,
      sessionId: null,
      enhanceEnabled: false,
      enhanceDefaultLoaded: false,
    });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('starts with empty messages', () => {
    const state = useChatStore.getState();
    expect(state.messages).toEqual([]);
    expect(state.isLoading).toBe(false);
    expect(state.streamingId).toBeNull();
  });

  it('clearMessages resets messages and session', () => {
    // Set up some state
    useChatStore.setState({
      messages: [
        { id: '1', role: 'user', content: 'hello', timestamp: new Date() },
        { id: '2', role: 'assistant', content: 'hi', timestamp: new Date() },
      ],
      sessionId: 'session-123',
    });

    useChatStore.getState().clearMessages();

    const state = useChatStore.getState();
    expect(state.messages).toEqual([]);
    expect(state.sessionId).toBeNull();
    expect(state.messagesRevision).toBeGreaterThan(0);
  });

  it('applies chat session snapshots from another window', () => {
    const unsubscribe = installChatSessionSync();

    window.dispatchEvent(new CustomEvent(CHAT_SESSION_SYNC_EVENT, {
      detail: {
        sourceId: 'pet-window',
        messages: [
          { id: 'u1', role: 'user', content: 'hello', timestamp: new Date().toISOString() },
          { id: 'a1', role: 'assistant', content: '**hi**', timestamp: new Date().toISOString() },
        ],
        isLoading: false,
        streamingId: null,
        sessionId: 'session-from-pet',
      },
    }));

    const state = useChatStore.getState();
    expect(state.sessionId).toBe('session-from-pet');
    expect(state.messages).toHaveLength(2);
    expect(state.messages[1].content).toBe('**hi**');

    unsubscribe();
  });

  it('does not let a stale shorter snapshot truncate a completed assistant reply', () => {
    useChatStore.setState({
      messages: [
        { id: 'u1', role: 'user', content: '识别屏幕', timestamp: new Date() },
        {
          id: 'a1',
          role: 'assistant',
          content: '看到你屏幕上的内容了。你一直在 **Codex** 中调试灵枢的聊天同步问题。',
          timestamp: new Date(),
          dbId: 'assistant-db-id',
        },
      ],
      isLoading: false,
      streamingId: null,
      sessionId: 'session-123',
    });
    const unsubscribe = installChatSessionSync();

    window.dispatchEvent(new CustomEvent(CHAT_SESSION_SYNC_EVENT, {
      detail: {
        sourceId: 'pet-window',
        messages: [
          { id: 'u1', role: 'user', content: '识别屏幕', timestamp: new Date().toISOString() },
          {
            id: 'a1',
            role: 'assistant',
            content: '看到你屏幕上的内容了。你一直在 **C',
            timestamp: new Date().toISOString(),
          },
        ],
        isLoading: true,
        streamingId: 'a1',
        sessionId: 'session-123',
      },
    }));

    const state = useChatStore.getState();
    expect(state.messages[1].content).toBe('看到你屏幕上的内容了。你一直在 **Codex** 中调试灵枢的聊天同步问题。');
    expect(state.messages[1].dbId).toBe('assistant-db-id');
    expect(state.isLoading).toBe(false);
    expect(state.streamingId).toBeNull();

    unsubscribe();
  });

  it('coalesces rapid local updates into one chat session sync event', () => {
    vi.useFakeTimers();
    const received: Event[] = [];
    const unsubscribe = installChatSessionSync();
    const handleSync = (event: Event) => received.push(event);
    window.addEventListener(CHAT_SESSION_SYNC_EVENT, handleSync);

    useChatStore.setState({
      messages: [{ id: 'u1', role: 'user', content: 'hello', timestamp: new Date() }],
      isLoading: true,
      streamingId: 'a1',
    });
    useChatStore.setState({
      messages: [
        { id: 'u1', role: 'user', content: 'hello', timestamp: new Date() },
        { id: 'a1', role: 'assistant', content: 'one', timestamp: new Date() },
      ],
    });
    useChatStore.setState({
      messages: [
        { id: 'u1', role: 'user', content: 'hello', timestamp: new Date() },
        { id: 'a1', role: 'assistant', content: 'one two', timestamp: new Date() },
      ],
    });

    expect(received).toHaveLength(0);
    vi.advanceTimersByTime(CHAT_SESSION_SYNC_DEBOUNCE_MS);
    expect(received).toHaveLength(1);

    window.removeEventListener(CHAT_SESSION_SYNC_EVENT, handleSync);
    unsubscribe();
  });

  it('regenerates an assistant message with the selected style and replaces the same bubble', async () => {
    useChatStore.setState({
      messages: [
        { id: 'u1', role: 'user', content: '介绍一下你自己', timestamp: new Date() },
        {
          id: 'a1',
          role: 'assistant',
          content: '您好，我是灵枢。',
          timestamp: new Date(),
          dbId: 'db-a1',
        },
      ],
      isLoading: false,
      streamingId: null,
      sessionId: 'session-123',
    });
    mockedApiFetch.mockResolvedValueOnce(sseResponse([
      { content: '我是灵枢，', done: false },
      { content: '可以帮你处理对话和本机任务。', done: true, assistant_message_id: 'db-a1' },
    ]));

    await useChatStore.getState().regenerateAssistantMessage('a1', 'too_formal');

    expect(mockedApiFetch).toHaveBeenCalledWith(
      '/api/v1/chat/messages/db-a1/regenerate',
      expect.objectContaining({
        method: 'POST',
        body: JSON.stringify({ style_tag: 'too_formal', client_platform: 'browser' }),
      }),
    );
    const state = useChatStore.getState();
    expect(state.isLoading).toBe(false);
    expect(state.streamingId).toBeNull();
    expect(state.messages[1]).toMatchObject({
      id: 'a1',
      content: '我是灵枢，可以帮你处理对话和本机任务。',
      dbId: 'db-a1',
      revision: 1,
    });
  });

  it('streams enhanced chat through the MoA endpoint and records member progress', async () => {
    mockedApiFetch.mockImplementation(async (input, init) => {
      if (input === '/api/v1/conversations' && init?.method === 'POST') {
        return {
          ok: true,
          status: 200,
          json: () => Promise.resolve({ id: 'session-123' }),
        } as Response;
      }
      if (input === '/api/v1/chat/enhance' && init?.method === 'POST') {
        return sseResponse([
          {
            type: 'start',
            members: [{ id: '0', label: 'DeepSeek · deepseek-chat', provider_id: 'deepseek' }],
            aggregator: 'OpenAI · gpt-4o',
          },
          { type: 'candidate', id: '0', status: 'done', draft: '候选答案' },
          { type: 'aggregate' },
          { type: 'content', content: '最终' },
          { type: 'content', content: '答案' },
          {
            type: 'done',
            assistant_message_id: 'assistant-db-id',
            user_message_id: 'user-db-id',
          },
        ]);
      }
      throw new Error(`Unexpected request: ${input}`);
    });
    useChatStore.setState({ enhanceEnabled: true, enhanceDefaultLoaded: true });

    await useChatStore.getState().sendMessage('解释 MoA');

    expect(mockedApiFetch).toHaveBeenCalledWith(
      '/api/v1/chat/enhance',
      expect.objectContaining({
        method: 'POST',
        body: JSON.stringify({
          message: '解释 MoA',
          session_id: 'session-123',
          client_platform: 'browser',
        }),
      }),
    );
    const assistant = useChatStore.getState().messages.find((message) => message.role === 'assistant');
    const user = useChatStore.getState().messages.find((message) => message.role === 'user');
    expect(useChatStore.getState().messages).toHaveLength(2);
    expect(user?.dbId).toBe('user-db-id');
    expect(assistant).toMatchObject({
      content: '最终答案',
      dbId: 'assistant-db-id',
      enhance: {
        phase: 'done',
        aggregatorLabel: 'OpenAI · gpt-4o',
        members: [
          {
            id: '0',
            label: 'DeepSeek · deepseek-chat',
            provider: 'deepseek',
            status: 'done',
            draft: '候选答案',
          },
        ],
      },
    });
  });

  it('sends the selected enhancement participants with the current chat turn', async () => {
    mockedApiFetch.mockImplementation(async (input, init) => {
      if (input === '/api/v1/conversations' && init?.method === 'POST') {
        return {
          ok: true,
          status: 200,
          json: () => Promise.resolve({ id: 'session-123' }),
        } as Response;
      }
      if (input === '/api/v1/chat/enhance' && init?.method === 'POST') {
        return sseResponse([
          {
            type: 'start',
            members: [{ id: '0', label: 'DeepSeek · deepseek-chat', provider_id: 'deepseek' }],
            aggregator: 'OpenAI · gpt-4o',
          },
          { type: 'content', content: 'ok' },
          { type: 'done', assistant_message_id: 'assistant-db-id' },
        ]);
      }
      throw new Error(`Unexpected request: ${input}`);
    });
    useChatStore.setState({ enhanceEnabled: true, enhanceDefaultLoaded: true });

    await useChatStore.getState().sendMessage('解释 MoA', {
      members: [{ provider_id: 'deepseek', model: 'deepseek-chat' }],
      aggregator: { provider_id: 'openai', model: 'gpt-4o' },
      per_model_timeout_ms: 45000,
      max_parallel: 2,
    });

    expect(mockedApiFetch).toHaveBeenCalledWith(
      '/api/v1/chat/enhance',
      expect.objectContaining({
        method: 'POST',
        body: JSON.stringify({
          message: '解释 MoA',
          session_id: 'session-123',
          client_platform: 'browser',
          enhance: {
            members: [{ provider_id: 'deepseek', model: 'deepseek-chat' }],
            aggregator: { provider_id: 'openai', model: 'gpt-4o' },
            per_model_timeout_ms: 45000,
            max_parallel: 2,
          },
        }),
      }),
    );
  });

  it('routes retry-like tool turns through normal chat even when enhancement is enabled', async () => {
    mockedApiFetch.mockImplementation(async (input, init) => {
      if (input === '/api/v1/conversations' && init?.method === 'POST') {
        return {
          ok: true,
          status: 200,
          json: () => Promise.resolve({ id: 'session-123' }),
        } as Response;
      }
      if (input === '/api/v1/chat' && init?.method === 'POST') {
        return sseResponse([
          { content: '这是重新搜索后的结果。', done: false },
          {
            done: true,
            assistant_message_id: 'assistant-db-id',
            user_message_id: 'user-db-id',
          },
        ]);
      }
      throw new Error(`Unexpected request: ${input}`);
    });
    useChatStore.setState({ enhanceEnabled: true, enhanceDefaultLoaded: true });

    await useChatStore.getState().sendMessage('重试');

    expect(mockedApiFetch).not.toHaveBeenCalledWith(
      '/api/v1/chat/enhance',
      expect.anything(),
    );
    expect(mockedApiFetch).toHaveBeenCalledWith(
      '/api/v1/chat',
      expect.objectContaining({
        method: 'POST',
        body: JSON.stringify({
          message: '重试',
          session_id: 'session-123',
          client_platform: 'browser',
        }),
      }),
    );
    const state = useChatStore.getState();
    expect(state.messages).toHaveLength(2);
    expect(state.messages[0]).toMatchObject({ role: 'user', content: '重试', dbId: 'user-db-id' });
    expect(state.messages[1]).toMatchObject({
      role: 'assistant',
      content: '这是重新搜索后的结果。',
      dbId: 'assistant-db-id',
    });
    expect(state.messages[1].enhance).toBeUndefined();
  });

  it('loads a recent conversation with persisted message ids', async () => {
    mockedApiFetch.mockResolvedValueOnce(new Response(JSON.stringify([
      {
        id: 'user-db-id',
        role: 'user',
        content: '之前的问题',
        model_id: null,
        created_at: '2026-07-10T10:00:00Z',
      },
      {
        id: 'assistant-db-id',
        role: 'assistant',
        content: '之前的回答',
        model_id: 'deepseek-v4-pro',
        created_at: '2026-07-10T10:00:01Z',
      },
    ]), { status: 200, headers: { 'Content-Type': 'application/json' } }));

    const loaded = await useChatStore.getState().loadSession('session-123');

    expect(loaded).toBe(true);
    expect(mockedApiFetch).toHaveBeenCalledWith('/api/v1/chat/sessions/session-123/messages');
    expect(useChatStore.getState()).toMatchObject({
      sessionId: 'session-123',
      isLoading: false,
    });
    expect(useChatStore.getState().messagesRevision).toBeGreaterThan(0);
    expect(useChatStore.getState().messages).toEqual([
      expect.objectContaining({ id: 'user-db-id', dbId: 'user-db-id', content: '之前的问题' }),
      expect.objectContaining({ id: 'assistant-db-id', dbId: 'assistant-db-id', content: '之前的回答' }),
    ]);
  });

  it('edits a persisted user message in place', async () => {
    useChatStore.setState({
      messages: [{
        id: 'user-local-id',
        dbId: 'user-db-id',
        role: 'user',
        content: '原消息',
        timestamp: new Date(),
      }],
    });
    mockedApiFetch.mockResolvedValueOnce(new Response(JSON.stringify({
      id: 'user-db-id',
      role: 'user',
      content: '修改后的消息',
      created_at: '2026-07-10T10:00:00Z',
    }), { status: 200, headers: { 'Content-Type': 'application/json' } }));

    await useChatStore.getState().editUserMessage('user-local-id', '  修改后的消息  ');

    expect(mockedApiFetch).toHaveBeenCalledWith(
      '/api/v1/chat/messages/user-db-id',
      expect.objectContaining({
        method: 'PATCH',
        body: JSON.stringify({ content: '修改后的消息' }),
      }),
    );
    expect(useChatStore.getState().messages[0]).toMatchObject({
      content: '修改后的消息',
      revision: 1,
    });
  });

  it('withdraws a persisted user message without deleting the conversation', async () => {
    useChatStore.setState({
      messages: [
        {
          id: 'user-local-id',
          dbId: 'user-db-id',
          role: 'user',
          content: '需要撤回',
          timestamp: new Date(),
        },
        {
          id: 'assistant-local-id',
          dbId: 'assistant-db-id',
          role: 'assistant',
          content: '对应回答仍保留',
          timestamp: new Date(),
        },
      ],
      sessionId: 'session-123',
    });
    mockedApiFetch.mockResolvedValueOnce(new Response(null, { status: 204 }));

    await useChatStore.getState().withdrawUserMessage('user-local-id');

    expect(mockedApiFetch).toHaveBeenCalledWith(
      '/api/v1/chat/messages/user-db-id',
      { method: 'DELETE' },
    );
    expect(useChatStore.getState().messages).toEqual([
      expect.objectContaining({ id: 'assistant-local-id' }),
    ]);
    expect(useChatStore.getState().sessionId).toBe('session-123');
    expect(useChatStore.getState().messagesRevision).toBeGreaterThan(0);
  });

  it('does not merge a withdrawn message back from an older window snapshot', () => {
    useChatStore.setState({
      messages: [{ id: 'a1', role: 'assistant', content: '保留回答', timestamp: new Date() }],
      messagesRevision: 2,
      sessionId: 'session-123',
    });
    const unsubscribe = installChatSessionSync();

    window.dispatchEvent(new CustomEvent(CHAT_SESSION_SYNC_EVENT, {
      detail: {
        sourceId: 'stale-window',
        messages: [
          { id: 'u1', role: 'user', content: '已撤回', timestamp: new Date().toISOString() },
          { id: 'a1', role: 'assistant', content: '保留回答', timestamp: new Date().toISOString() },
        ],
        messagesRevision: 1,
        sessionId: 'session-123',
      },
    }));

    expect(useChatStore.getState().messages).toHaveLength(1);
    expect(useChatStore.getState().messages[0].id).toBe('a1');
    unsubscribe();
  });
});
