import { create } from 'zustand';
import { apiFetch } from '../lib/api';

export interface Project {
  id: string;
  name: string;
  description: string | null;
  status: string;
  health_score: number | null;
  created_at: string;
}

interface ProjectState {
  projects: Project[];
  loading: boolean;
  error: string | null;
  fetchProjects: () => Promise<void>;
  createProject: (data: { name: string; description?: string }) => Promise<Project | null>;
  updateProject: (id: string, data: { name: string; description?: string }) => Promise<void>;
  deleteProject: (id: string) => Promise<void>;
}

function getErrorMessage(error: unknown, fallback: string): string {
  return error instanceof Error ? error.message : fallback;
}

export const useProjectStore = create<ProjectState>((set, get) => ({
  projects: [],
  loading: false,
  error: null,

  fetchProjects: async () => {
    set({ loading: true, error: null });
    try {
      const res = await apiFetch('/api/v1/projects');
      if (!res.ok) throw new Error('Failed to fetch projects');
      const projects: Project[] = await res.json();
      set({ projects, loading: false });
    } catch (err) {
      set({ error: getErrorMessage(err, 'Failed to load projects'), loading: false });
    }
  },

  createProject: async (data) => {
    set({ loading: true, error: null });
    try {
      const res = await apiFetch('/api/v1/projects', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(data),
      });
      if (!res.ok) throw new Error('Failed to create project');
      const project: Project = await res.json();
      set({ projects: [...get().projects, project], loading: false });
      return project;
    } catch (err) {
      set({ error: getErrorMessage(err, 'Failed to create project'), loading: false });
      return null;
    }
  },

  updateProject: async (id, data) => {
    set({ loading: true, error: null });
    try {
      const res = await apiFetch(`/api/v1/projects/${id}`, {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(data),
      });
      if (!res.ok) throw new Error('Failed to update project');
      const updated: Project = await res.json();
      set({
        projects: get().projects.map((p) => (p.id === id ? updated : p)),
        loading: false,
      });
    } catch (err) {
      set({ error: getErrorMessage(err, 'Failed to update project'), loading: false });
    }
  },

  deleteProject: async (id) => {
    set({ loading: true, error: null });
    try {
      const res = await apiFetch(`/api/v1/projects/${id}`, { method: 'DELETE' });
      if (!res.ok && res.status !== 204) throw new Error('Failed to delete project');
      set({
        projects: get().projects.filter((p) => p.id !== id),
        loading: false,
      });
    } catch (err) {
      set({ error: getErrorMessage(err, 'Failed to delete project'), loading: false });
    }
  },
}));
