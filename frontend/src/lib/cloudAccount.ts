import { isTauri } from './tauri';

// Cloud account operations use the fixed issuer. Team requests below explicitly
// send only a short-lived access token to this installation's team API.
export const CLOUD_ACCOUNT_URL = 'https://account.lingeringdream.com';
const DEVICE_ID_KEY = 'lingshu_cloud_device_id';

function savedDeviceId(): string | undefined {
  try {
    const value = window.localStorage.getItem(DEVICE_ID_KEY);
    return value && /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(value) ? value : undefined;
  } catch { return undefined; }
}

export interface CloudUser {
  id: string;
  email: string;
  display_name: string;
  email_verified_at: string;
  created_at: string;
}

export interface CloudDevice { id: string; name: string; platform: string }
export interface CloudSubscription {
  plan_code: string;
  plan_name: string;
  status: string;
  current_period_end: string | null;
  entitlement_active: boolean;
}
export interface ProductEntitlements {
  effective_plan: string;
  access_source?: 'free' | 'subscription' | 'team_membership';
  features: Array<{ code: string; name: string; plans: string[]; included: boolean; available: boolean; enabled: boolean }>;
}
export interface CloudAccount {
  entitlements?: ProductEntitlements;
  user: CloudUser;
  device: CloudDevice;
  subscription: CloudSubscription | null;
  subscriptionError?: string;
}
interface Tokens {
  access_token: string;
  refresh_token: string;
  expires_in: number;
  user: CloudUser;
  device: CloudDevice;
}
export interface CredentialStore {
  load(): Promise<string | null>;
  save(value: string): Promise<void>;
  clear(): Promise<void>;
}

// Browser development keeps tokens in this tab's memory only. The installed app
// persists only the refresh credential in Windows Credential Manager / Keychain.
let browserCredential: string | null = null;
export const cloudCredentialStore: CredentialStore = {
  async load() {
    if (!isTauri()) return browserCredential;
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke<string | null>('cloud_credential_load');
  },
  async save(value) {
    if (!isTauri()) { browserCredential = value; return; }
    const { invoke } = await import('@tauri-apps/api/core');
    await invoke('cloud_credential_save', { value });
  },
  async clear() {
    if (!isTauri()) { browserCredential = null; return; }
    const { invoke } = await import('@tauri-apps/api/core');
    await invoke('cloud_credential_clear');
  },
};

export class CloudAccountError extends Error {
  constructor(message: string, public status = 0, public code = '') { super(message); }
}

const messages: Record<string, string> = {
  INVALID_CREDENTIALS: '邮箱或密码不正确；新账号请先完成邮件验证。',
  LOGIN_RATE_LIMITED: '登录尝试过于频繁，请稍后再试。',
  INVALID_PASSWORD: '请输入有效密码。',
  INVALID_EMAIL: '请输入有效邮箱。',
  INVALID_AUTHORIZATION: '网页登录验证已过期或失效，请重新打开网页登录。',
};

export function cloudErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : typeof error === 'string' ? error : '云账户暂时不可用，请稍后重试。';
}

export class CloudAccountClient {
  private tokens: Tokens | null = null;
  private expiresAt = 0;
  private queue: Promise<unknown> = Promise.resolve();

  constructor(private vault: CredentialStore, private fetcher: typeof fetch = (...args) => fetch(...args)) {}

  // Serialise the entire operation, including vault writes. Rotating a refresh
  // token concurrently would trigger the server's replay protection.
  private serial<T>(operation: () => Promise<T>): Promise<T> {
    const next = this.queue.then(operation, operation);
    this.queue = next.catch(() => undefined);
    return next;
  }

  private async request<T>(path: string, body?: unknown, token?: string, method?: string): Promise<T> {
    let response: Response;
    try {
      response = await this.fetcher(`${CLOUD_ACCOUNT_URL}/api/v1${path}`, {
        method: method || (body === undefined ? 'GET' : 'POST'),
        credentials: 'omit',
        cache: 'no-store',
        redirect: 'error',
        headers: {
          ...(body === undefined ? {} : { 'Content-Type': 'application/json' }),
          ...(token ? { Authorization: `Bearer ${token}` } : {}),
        },
        body: body === undefined ? undefined : JSON.stringify(body),
        signal: AbortSignal.timeout(15_000),
      });
    } catch {
      throw new CloudAccountError('无法连接云账户服务，请检查网络后重试。本地功能仍可使用。');
    }
    if (!response.ok) {
      const data = await response.json().catch(() => null);
      const code = data?.error?.code || '';
      const retry = Number(response.headers.get('Retry-After'));
      throw new CloudAccountError(
        messages[code]
          ? messages[code] + (response.status === 429 && retry > 0 ? `（约 ${Math.ceil(retry / 60)} 分钟）` : '')
          : response.status === 401 ? '云账户会话已过期或被撤销，请重新登录。' : '云账户请求失败，请稍后重试。',
        response.status, code,
      );
    }
    return response.status === 204 ? undefined as T : response.json();
  }

  private async accept(tokens: Tokens): Promise<void> {
    try {
      await this.vault.save(tokens.refresh_token);
    } catch (error) {
      // Never report a durable login when secure storage failed. Revoke the new
      // family best-effort, including after a refresh rotation.
      this.tokens = null;
      await this.request('/auth/logout', { refresh_token: tokens.refresh_token }).catch(() => undefined);
      throw new CloudAccountError(`无法安全保存登录状态：${cloudErrorMessage(error)}`, 401, 'SECURE_STORAGE_FAILED');
    }
    this.tokens = tokens;
    this.expiresAt = Date.now() + tokens.expires_in * 1000;
    // A device ID is non-secret metadata. The server verifies its owner and
    // revoked state before reuse, including when another account logs in.
    try { window.localStorage.setItem(DEVICE_ID_KEY, tokens.device.id); } catch { /* optional */ }
  }

  private async clear(): Promise<void> {
    this.tokens = null;
    this.expiresAt = 0;
    await this.vault.clear();
  }

  private async refresh(): Promise<boolean> {
    const refreshToken = this.tokens?.refresh_token || await this.vault.load();
    if (!refreshToken) return false;
    try {
      await this.accept(await this.request<Tokens>('/auth/refresh', { refresh_token: refreshToken }));
      return true;
    } catch (error) {
      if (error instanceof CloudAccountError && error.status === 401) await this.clear();
      throw error;
    }
  }

  private async authorized<T>(path: string): Promise<T> {
    try {
      return await this.request<T>(path, undefined, this.tokens!.access_token);
    } catch (error) {
      if (!(error instanceof CloudAccountError) || error.status !== 401) throw error;
      await this.refresh();
      try {
        return await this.request<T>(path, undefined, this.tokens!.access_token);
      } catch (retryError) {
        if (retryError instanceof CloudAccountError && retryError.status === 401) await this.clear();
        throw retryError;
      }
    }
  }

  private async authorizedRequest<T>(path: string, method: 'GET' | 'PUT', body?: unknown): Promise<T> {
    if ((!this.tokens || Date.now() >= this.expiresAt - 60_000) && !await this.refresh()) {
      throw new CloudAccountError('请先登录云账户。', 401);
    }
    const send = () => this.request<T>(path, body, this.tokens!.access_token, method);
    try { return await send(); }
    catch (error) {
      if (!(error instanceof CloudAccountError) || error.status !== 401) throw error;
      await this.refresh();
      return send();
    }
  }

  readEncryptedSync<T>(): Promise<T> {
    return this.serial(() => this.authorizedRequest<T>('/sync', 'GET'));
  }

  writeEncryptedSync<T>(body: unknown): Promise<T> {
    return this.serial(() => this.authorizedRequest<T>('/sync', 'PUT', body));
  }

  readSyncHistory<T>(): Promise<T> {
    return this.serial(() => this.authorizedRequest<T>('/sync/history', 'GET'));
  }

  private async overview(): Promise<CloudAccount> {
    const account = await this.authorized<{ user: CloudUser; device: CloudDevice }>('/account');
    try {
      const billing = await this.authorized<{ subscription: CloudSubscription; entitlements?: ProductEntitlements }>('/account/subscription');
      return { ...account, subscription: billing.subscription, entitlements: billing.entitlements };
    } catch (error) {
      if (error instanceof CloudAccountError && error.status === 401) throw error;
      return { ...account, subscription: null, subscriptionError: cloudErrorMessage(error) };
    }
  }

  load(): Promise<CloudAccount | null> {
    return this.serial(async () => {
      if ((!this.tokens || Date.now() >= this.expiresAt - 60_000) && !await this.refresh()) return null;
      return this.overview();
    });
  }

  teamRequest(path: string, init: { method?: string; body?: string } = {}): Promise<Response> {
    if (!/^\/api\/v1\/teams(?:\/|$)/.test(path) || path.includes('..') || /[?#\\]/.test(path)) {
      return Promise.reject(new CloudAccountError('无效的团队接口路径'));
    }
    return this.serial(async () => {
      if ((!this.tokens || Date.now() >= this.expiresAt - 60_000) && !await this.refresh()) {
        throw new CloudAccountError('请先在「云账户」登录，再进入团队工作区。', 401);
      }
      const send = () => this.fetcher(`${isTauri() ? 'http://127.0.0.1:8080' : ''}${path}`, {
        ...init, credentials: 'omit', redirect: 'error', cache: 'no-store',
        headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${this.tokens!.access_token}` },
        signal: AbortSignal.timeout(30_000),
      });
      const response = await send();
      if (response.status !== 401) return response;
      if (!await this.refresh()) throw new CloudAccountError('请重新登录云账户。', 401);
      return send();
    });
  }

  login(email: string, password: string, deviceName: string, platform: string): Promise<CloudAccount> {
    return this.serial(async () => {
      // Cleanly close an earlier offline/hidden session before switching users.
      const previous = this.tokens?.refresh_token || await this.vault.load();
      if (previous) {
        await this.request('/auth/logout', { refresh_token: previous });
        await this.clear();
      }
      await this.accept(await this.request<Tokens>('/auth/login', {
        email: email.trim(), password, device_name: deviceName.trim() || '灵枢桌面端', platform,
        device_id: savedDeviceId(),
      }));
      // Login itself contains the verified identity. Billing being unavailable
      // must not make the UI claim the login failed.
      try { return await this.overview(); }
      catch (error) {
        if (!this.tokens || (error instanceof CloudAccountError && error.status === 401)) throw error;
        return { user: this.tokens.user, device: this.tokens.device, subscription: null, subscriptionError: cloudErrorMessage(error) };
      }
    });
  }

  logout(): Promise<void> {
    return this.serial(async () => {
      const refreshToken = this.tokens?.refresh_token || await this.vault.load();
      // On network failure retain credentials and show a retry, so logout never
      // silently leaves a valid cloud session behind.
      if (refreshToken) await this.request('/auth/logout', { refresh_token: refreshToken });
      await this.clear();
    });
  }

  completeBrowserLogin(code: string, verifier: string, redirectUri: string): Promise<CloudAccount> {
    return this.serial(async () => {
      const next = await this.request<Tokens>('/auth/desktop/exchange', { code, code_verifier: verifier, redirect_uri: redirectUri });
      try {
        const previous = this.tokens?.refresh_token || await this.vault.load();
        if (previous) await this.request('/auth/logout', { refresh_token: previous });
        await this.accept(next);
      } catch (error) {
        await this.request('/auth/logout', { refresh_token: next.refresh_token }).catch(() => undefined);
        throw error;
      }
      try { return await this.overview(); }
      catch (error) {
        if (!this.tokens || (error instanceof CloudAccountError && error.status === 401)) throw error;
        return { user: next.user, device: next.device, subscription: null, subscriptionError: cloudErrorMessage(error) };
      }
    });
  }
}

export const cloudAccountClient = new CloudAccountClient(cloudCredentialStore);

export async function openCloudAccount(path: '/account' | '/billing' | '/devices'): Promise<void> {
  const url = `${CLOUD_ACCOUNT_URL}${path}`;
  if (isTauri()) {
    const { openUrl } = await import('@tauri-apps/plugin-opener');
    await openUrl(url);
  } else {
    window.open(url, '_blank', 'noopener,noreferrer');
  }
}
