/* global CustomEvent */
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import {
  CHAT_SESSION_SYNC_DEBOUNCE_MS,
  CHAT_SESSION_SYNC_EVENT,
  installChatSessionSync,
  useChatStore,
} from '../chatStore';

describe('chatStore', () => {
  beforeEach(() => {
    // Reset the store state between tests
    useChatStore.setState({
      messages: [],
      isLoading: false,
      streamingId: null,
      sessionId: null,
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
});
