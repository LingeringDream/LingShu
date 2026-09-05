import { beforeEach, describe, expect, it, vi } from 'vitest';
import { CLOUD_ACCOUNT_URL, CloudAccountClient, cloudCredentialStore, type CredentialStore } from '../cloudAccount';

vi.mock('../tauri', () => ({ isTauri: () => false }));

const identity = {
  user: { id: 'user-1', email: 'alice@example.com', display_name: 'Alice', email_verified_at: '2026-09-05', created_at: '2026-09-05' },
  device: { id: '312683fe-b09c-4300-a679-c90b5fd00f47', name: '工作电脑', platform: 'windows' },
};
const tokens = (suffix = '1', expires = 900) => ({ ...identity, access_token: `access-${suffix}`, refresh_token: `refresh-${suffix}`, expires_in: expires });
const ok = (data: unknown, status = 200) => new Response(JSON.stringify(data), { status });
const rejected = (code = 'SESSION_REVOKED', status = 401) => ok({ error: { code } }, status);
const subscription = { plan_code: 'free', plan_name: '免费版', status: 'active', current_period_end: null, entitlement_active: true };

describe('cloud account lifecycle', () => {
  let stored: string | null;
  let vault: CredentialStore;
  let fetcher: ReturnType<typeof vi.fn<typeof fetch>>;
  let client: CloudAccountClient;
  beforeEach(() => {
    stored = null;
    vault = {
      load: vi.fn(async () => stored),
      save: vi.fn(async (value: string) => { stored = value; }),
      clear: vi.fn(async () => { stored = null; }),
    };
    fetcher = vi.fn<typeof fetch>();
    client = new CloudAccountClient(vault, fetcher);
    window.localStorage.clear();
  });
  const overview = () => [ok(identity), ok({ subscription })];

  it('does not contact the network when no cloud login exists', async () => {
    expect(await client.load()).toBeNull();
    expect(fetcher).not.toHaveBeenCalled();
  });

  it('uses the website identity and keeps local auth completely separate', async () => {
    window.localStorage.setItem('lingshu_auth_token', 'local-jwt');
    fetcher.mockResolvedValueOnce(ok(tokens()));
    for (const response of overview()) fetcher.mockResolvedValueOnce(response);
    expect((await client.login(' alice@example.com ', 'secret', '工作电脑', 'windows')).user).toEqual(identity.user);
    expect(stored).toBe('refresh-1');
    const [url, init] = fetcher.mock.calls[0];
    expect(url).toBe(`${CLOUD_ACCOUNT_URL}/api/v1/auth/login`);
    expect(JSON.parse(init!.body as string)).toMatchObject({ email: 'alice@example.com', password: 'secret', device_name: '工作电脑', platform: 'windows' });
    expect(init).toMatchObject({ credentials: 'omit', redirect: 'error' });
    expect(fetcher.mock.calls[1][1]?.headers).toEqual({ Authorization: 'Bearer access-1' });
    expect(window.localStorage.getItem('lingshu_auth_token')).toBe('local-jwt');
    expect(window.localStorage.length).toBe(2);
    expect(window.localStorage.getItem('lingshu_cloud_device_id')).toBe(identity.device.id);
    expect(JSON.stringify(window.localStorage)).not.toMatch(/refresh-1|access-1|secret/);
  });

  it('serializes concurrent restoration and persists rotation before reading the account', async () => {
    stored = 'refresh-old';
    fetcher.mockResolvedValueOnce(ok(tokens('new')));
    for (const response of [...overview(), ...overview()]) fetcher.mockResolvedValueOnce(response);
    const [first, second] = await Promise.all([client.load(), client.load()]);
    expect(first?.user.id).toBe(second?.user.id);
    expect(stored).toBe('refresh-new');
    expect(fetcher.mock.calls.filter(([url]) => String(url).endsWith('/auth/refresh'))).toHaveLength(1);
    expect(vault.save).toHaveBeenCalledWith('refresh-new');
  });

  it('reuses the non-secret device ID on a later login', async () => {
    window.localStorage.setItem('lingshu_cloud_device_id', identity.device.id);
    fetcher.mockResolvedValueOnce(ok(tokens()));
    for (const response of overview()) fetcher.mockResolvedValueOnce(response);
    await client.login('alice@example.com', 'secret', '电脑', 'windows');
    expect(JSON.parse(fetcher.mock.calls[0][1]!.body as string).device_id).toBe(identity.device.id);
  });

  it('renews an access token rejected by the server and retries with the replacement', async () => {
    stored = 'refresh-old';
    fetcher.mockResolvedValueOnce(ok(tokens('1'))).mockResolvedValueOnce(rejected()).mockResolvedValueOnce(ok(tokens('2')));
    for (const response of overview()) fetcher.mockResolvedValueOnce(response);
    expect((await client.load())?.subscription).toEqual(subscription);
    expect(fetcher.mock.calls[3][1]?.headers).toEqual({ Authorization: 'Bearer access-2' });
    expect(stored).toBe('refresh-2');
  });

  it('clears revoked credentials instead of looping on refresh', async () => {
    stored = 'revoked';
    fetcher.mockResolvedValueOnce(rejected());
    await expect(client.load()).rejects.toMatchObject({ status: 401 });
    expect(stored).toBeNull();
    expect(await client.load()).toBeNull();
    expect(fetcher).toHaveBeenCalledTimes(1);
  });

  it('keeps the credential on temporary network failure so restoration can be retried', async () => {
    stored = 'refresh-offline';
    fetcher.mockRejectedValueOnce(new TypeError('offline'));
    await expect(client.load()).rejects.toThrow('无法连接');
    expect(stored).toBe('refresh-offline');
    expect(vault.clear).not.toHaveBeenCalled();
  });

  it('revokes a newly created session when secure storage fails', async () => {
    vault.save = vi.fn().mockRejectedValue(new Error('vault locked'));
    fetcher.mockResolvedValueOnce(ok(tokens())).mockResolvedValueOnce(new Response(null, { status: 204 }));
    await expect(client.login('alice@example.com', 'secret', '电脑', 'windows')).rejects.toThrow('安全保存');
    expect(fetcher.mock.calls[1][0]).toBe(`${CLOUD_ACCOUNT_URL}/api/v1/auth/logout`);
    expect(stored).toBeNull();
  });

  it('does not discard a valid login just because billing is down', async () => {
    fetcher.mockResolvedValueOnce(ok(tokens())).mockResolvedValueOnce(ok(identity)).mockResolvedValueOnce(rejected('INTERNAL_ERROR', 500));
    const result = await client.login('alice@example.com', 'secret', '电脑', 'windows');
    expect(result.user).toEqual(identity.user);
    expect(result.subscription).toBeNull();
    expect(result.subscriptionError).toBeTruthy();
  });

  it('retains an offline logout for retry, then revokes before clearing the vault', async () => {
    stored = 'refresh-1';
    fetcher.mockRejectedValueOnce(new Error('offline')).mockResolvedValueOnce(new Response(null, { status: 204 }));
    await expect(client.logout()).rejects.toThrow('无法连接');
    expect(stored).toBe('refresh-1');
    await client.logout();
    expect(stored).toBeNull();
    expect(await client.load()).toBeNull();
  });

  it('reports invalid credentials without changing local authentication', async () => {
    fetcher.mockResolvedValueOnce(rejected('INVALID_CREDENTIALS'));
    await expect(client.login('alice@example.com', 'wrong', '电脑', 'windows')).rejects.toThrow('邮箱或密码不正确');
    expect(vault.save).not.toHaveBeenCalled();
  });

  it('browser storage never writes credentials to localStorage or sessionStorage', async () => {
    await cloudCredentialStore.save('sensitive-refresh-token');
    expect(await cloudCredentialStore.load()).toBe('sensitive-refresh-token');
    expect(window.localStorage.length).toBe(0);
    expect(window.sessionStorage.length).toBe(0);
    await cloudCredentialStore.clear();
    expect(await cloudCredentialStore.load()).toBeNull();
  });

  it('exchanges the browser code with its proof and saves a separate desktop session', async () => {
    fetcher.mockResolvedValueOnce(ok(tokens('browser')));
    for (const response of overview()) fetcher.mockResolvedValueOnce(response);
    expect((await client.completeBrowserLogin('one-use-code', 'private-verifier', 'http://127.0.0.1:54321/callback')).user).toEqual(identity.user);
    expect(JSON.parse(fetcher.mock.calls[0][1]!.body as string)).toEqual({ code: 'one-use-code', code_verifier: 'private-verifier', redirect_uri: 'http://127.0.0.1:54321/callback' });
    expect(stored).toBe('refresh-browser');
  });

  it('revokes exchanged tokens when the vault cannot even be read', async () => {
    vault.load = vi.fn().mockRejectedValue(new Error('vault unavailable'));
    fetcher.mockResolvedValueOnce(ok(tokens('browser'))).mockResolvedValueOnce(new Response(null, { status: 204 }));
    await expect(client.completeBrowserLogin('code', 'proof', 'http://127.0.0.1:54321/callback')).rejects.toThrow('vault unavailable');
    expect(fetcher.mock.calls[1][0]).toContain('/auth/logout');
  });
});
