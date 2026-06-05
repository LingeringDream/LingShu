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
    localSessionRequest = fetch('/api/v1/auth/local-session', { method: 'POST' })
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

export async function apiFetch(input: string, init: ApiFetchInit = {}) {
  const headers = { ...(init.headers ?? {}) };
  const session = await ensureLocalSession();
  headers.Authorization = `Bearer ${session.token}`;

  const resp = await fetch(input, { ...init, headers });
  if (resp.status !== 401) {
    return resp;
  }

  const refreshed = await ensureLocalSession(true);
  return fetch(input, {
    ...init,
    headers: { ...(init.headers ?? {}), Authorization: `Bearer ${refreshed.token}` },
  });
}
