import { create } from 'zustand';
import { apiFetch } from '../lib/api';

export interface Memory {
  id: string;
  memory_type: string;
  content: string;
  importance: number;
  metadata: Record<string, unknown>;
  created_at: string;
  updated_at: string;
}

interface MemoryState {
  memories: Memory[];
  loading: boolean;
  error: string | null;
  searchQuery: string;
  typeFilter: string;
  fetchMemories: () => Promise<void>;
  searchMemories: (q: string) => Promise<void>;
  createMemory: (data: { memory_type: string; content: string; importance: number }) => Promise<void>;
  updateMemory: (id: string, data: Partial<Pick<Memory, 'content' | 'memory_type' | 'importance'>>) => Promise<void>;
  deleteMemory: (id: string) => Promise<void>;
  setSearchQuery: (q: string) => void;
  setTypeFilter: (t: string) => void;
}

export const useMemoryStore = create<MemoryState>((set) => ({
  memories: [],
  loading: false,
  error: null,
  searchQuery: '',
  typeFilter: '',

  fetchMemories: async () => {
    set({ loading: true, error: null });
    try {
      const params = new URLSearchParams({ limit: '100' });
      const resp = await apiFetch(`/api/v1/memories?${params}`);
      if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
      const data: Memory[] = await resp.json();
      set({ memories: data, loading: false });
    } catch (e) {
      set({ error: e instanceof Error ? e.message : '加载失败', loading: false });
    }
  },

  searchMemories: async (q: string) => {
    if (!q.trim()) {
      // fall back to list
      const store = useMemoryStore.getState();
      store.fetchMemories();
      return;
    }
    set({ loading: true, error: null, searchQuery: q });
    try {
      const params = new URLSearchParams({ q, limit: '50' });
      const resp = await apiFetch(`/api/v1/memories/search?${params}`);
      if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
      const data: Memory[] = await resp.json();
      set({ memories: data, loading: false });
    } catch (e) {
      set({ error: e instanceof Error ? e.message : '搜索失败', loading: false });
    }
  },

  createMemory: async (data) => {
    const resp = await apiFetch('/api/v1/memories', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(data),
    });
    if (!resp.ok) {
      const err = await resp.json();
      throw new Error(err?.error?.message ?? `HTTP ${resp.status}`);
    }
    // Refresh list
    const store = useMemoryStore.getState();
    store.fetchMemories();
  },

  updateMemory: async (id, data) => {
    const resp = await apiFetch(`/api/v1/memories/${id}`, {
      method: 'PATCH',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(data),
    });
    if (!resp.ok) {
      const err = await resp.json();
      throw new Error(err?.error?.message ?? `HTTP ${resp.status}`);
    }
    const store = useMemoryStore.getState();
    store.fetchMemories();
  },

  deleteMemory: async (id) => {
    const resp = await apiFetch(`/api/v1/memories/${id}`, { method: 'DELETE' });
    if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
    const store = useMemoryStore.getState();
    store.fetchMemories();
  },

  setSearchQuery: (q) => set({ searchQuery: q }),
  setTypeFilter: (t) => set({ typeFilter: t }),
}));
