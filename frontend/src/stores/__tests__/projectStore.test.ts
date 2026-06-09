import { describe, it, expect, beforeEach } from 'vitest';
import { useProjectStore } from '../projectStore';

describe('projectStore', () => {
  beforeEach(() => {
    useProjectStore.setState({
      projects: [],
      loading: false,
      error: null,
    });
  });

  it('starts with empty projects', () => {
    const state = useProjectStore.getState();
    expect(state.projects).toEqual([]);
    expect(state.loading).toBe(false);
    expect(state.error).toBeNull();
  });

  it('supports CRUD operations shape', () => {
    const state = useProjectStore.getState();
    // Verify the store has all expected methods
    expect(typeof state.fetchProjects).toBe('function');
    expect(typeof state.createProject).toBe('function');
    expect(typeof state.updateProject).toBe('function');
    expect(typeof state.deleteProject).toBe('function');
  });
});
