const TOKEN_KEY = 'lingshu_auth_token';

interface LocalSessionResponse {
  user_id: string;
  token: string;
  display_name: string;
}

let localSessionRequest: Promise<LocalSessionResponse> | null = null;

export function getAuthToken(): string | null {
  return window.localStorage.getItem(TOKEN_KEY);
}

export function setAuthToken(token: string) {
  window.localStorage.setItem(TOKEN_KEY, token);
}

export async function ensureLocalSession(force = false): Promise<LocalSessionResponse> {
  const token = getAuthToken();
  if (token && !force) {
    return { user_id: '', token, display_name: '本地用户' };
  }

  if (!localSessionRequest) {
    localSessionRequest = fetch(apiBaseUrl() + '/api/v1/auth/local-session', { method: 'POST' })
      .then(async (resp) => {
        if (!resp.ok) {
          const err = await resp.json().catch(() => null);
          throw new Error(err?.error?.message ?? `HTTP ${resp.status}`);
        }
        const data: LocalSessionResponse = await resp.json();
        setAuthToken(data.token);
        return data;
      })
      .finally(() => {
        localSessionRequest = null;
      });
  }

  return localSessionRequest;
}

interface ApiFetchInit {
  method?: string;
  headers?: Record<string, string>;
  body?: string;
}

/** Payload shape for POST /api/v1/signals. */
export interface SignalPayload {
  event_type: string;
  entity_type?: string;
  entity_id?: string;
  metadata?: Record<string, unknown>;
}

/** Fire-and-forget signal ingestion. Never throws. */
export async function postSignal(payload: SignalPayload): Promise<void> {
  try {
    await apiFetch('/api/v1/signals', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    });
  } catch {
    // best-effort only
  }
}

function apiBaseUrl(): string {
  // In Tauri (built .app), there's no Vite proxy — use the backend directly.
  // In Vite dev, relative paths are proxied by the dev server.
  if (typeof window !== 'undefined' && '__TAURI__' in window) {
    return 'http://127.0.0.1:8080';
  }
  return '';
}

export async function apiFetch(input: string, init: ApiFetchInit = {}) {
  const headers = { ...(init.headers ?? {}) };
  const session = await ensureLocalSession();
  headers.Authorization = `Bearer ${session.token}`;

  const url = apiBaseUrl() + input;
  const resp = await fetch(url, { ...init, headers });
  if (resp.status !== 401) {
    return resp;
  }

  const refreshed = await ensureLocalSession(true);
  // Must reuse the same absolute base as the first attempt; a relative `input`
  // would resolve against the bundled app's `tauri://localhost` origin instead
  // of the backend.
  return fetch(url, {
    ...init,
    headers: { ...(init.headers ?? {}), Authorization: `Bearer ${refreshed.token}` },
  });
}
