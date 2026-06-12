/* global CustomEvent */
import { describe, it, expect, beforeEach } from 'vitest';
import {
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
});
