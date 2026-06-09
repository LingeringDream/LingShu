import { describe, it, expect, beforeEach } from 'vitest';
import { useMemoryStore } from '../memoryStore';

describe('memoryStore', () => {
  beforeEach(() => {
    useMemoryStore.setState({
      memories: [],
      loading: false,
      error: null,
      searchQuery: '',
      typeFilter: '',
    });
  });

  it('starts with empty memories', () => {
    const state = useMemoryStore.getState();
    expect(state.memories).toEqual([]);
    expect(state.loading).toBe(false);
    expect(state.searchQuery).toBe('');
    expect(state.typeFilter).toBe('');
  });

  it('setSearchQuery updates search query', () => {
    useMemoryStore.getState().setSearchQuery('test query');
    expect(useMemoryStore.getState().searchQuery).toBe('test query');
  });

  it('setTypeFilter updates type filter', () => {
    useMemoryStore.getState().setTypeFilter('preference');
    expect(useMemoryStore.getState().typeFilter).toBe('preference');

    useMemoryStore.getState().setTypeFilter('');
    expect(useMemoryStore.getState().typeFilter).toBe('');
  });
});
