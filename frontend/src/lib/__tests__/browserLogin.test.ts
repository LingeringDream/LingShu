/* global Crypto */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { createLoginProof, waitForBrowserLogin, type BrowserLoginBridge } from '../browserLogin';

const { webcrypto, createHash } = await vi.importActual<{
  webcrypto: Crypto;
  createHash(algorithm: string): { update(value: string): { digest(encoding: string): string } };
}>('node:crypto');

vi.mock('../tauri', () => ({ isTauri: () => true, getPlatformCapabilities: async () => ({ platform: 'windows' }) }));
afterEach(() => { vi.unstubAllGlobals(); vi.useRealTimers(); });

function bridge(): BrowserLoginBridge {
  return { start: vi.fn(async () => 'http://127.0.0.1:54321/callback'), open: vi.fn(async () => undefined), poll: vi.fn(async () => 'code'), cancel: vi.fn(async () => undefined) };
}

describe('desktop browser authorization', () => {
  it('uses random state and an S256 proof whose verifier stays out of the URL', async () => {
    vi.stubGlobal('crypto', webcrypto);
    const proof = await createLoginProof();
    expect(proof.state).toMatch(/^[a-f0-9]{64}$/);
    expect(proof.challenge).toBe(createHash('sha256').update(proof.verifier).digest('base64url'));
    expect((await createLoginProof()).state).not.toBe(proof.state);
    const native = bridge();
    const grant = await waitForBrowserLogin('工作电脑', new AbortController().signal, native);
    const url = new window.URL(vi.mocked(native.open).mock.calls[0][0]);
    expect(url.origin).toBe('https://account.lingeringdream.com');
    expect(url.searchParams.get('device_name')).toBe('工作电脑');
    expect(url.searchParams.get('code_challenge')).toBe(createHash('sha256').update(grant.verifier).digest('base64url'));
    expect(url.href).not.toContain(grant.verifier);
    expect(grant.redirectUri).toBe('http://127.0.0.1:54321/callback');
    expect(native.cancel).toHaveBeenCalled();
  });

  it('closes the callback listener when opening the browser fails', async () => {
    vi.stubGlobal('crypto', webcrypto);
    const native = bridge();
    native.open = vi.fn().mockRejectedValue(new Error('cannot open'));
    await expect(waitForBrowserLogin('电脑', new AbortController().signal, native)).rejects.toThrow('cannot open');
    expect(native.cancel).toHaveBeenCalled();
  });

  it('cancels pending authorization and releases the listener', async () => {
    vi.stubGlobal('crypto', webcrypto);
    const native = bridge();
    const controller = new AbortController();
    native.poll = vi.fn(async () => { controller.abort(); return null; });
    await expect(waitForBrowserLogin('电脑', controller.signal, native)).rejects.toThrow('已取消');
    expect(native.cancel).toHaveBeenCalled();
  });
});
