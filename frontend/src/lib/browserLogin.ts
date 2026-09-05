/* global TextEncoder, btoa, URL */
import { isTauri, getPlatformCapabilities } from './tauri';
import { CLOUD_ACCOUNT_URL } from './cloudAccount';

export interface BrowserLoginGrant { code: string; verifier: string; redirectUri: string }
export interface BrowserLoginBridge {
  start(state: string): Promise<string>;
  open(url: string): Promise<void>;
  poll(state: string): Promise<string | null>;
  cancel(state: string): Promise<void>;
}

const nativeBridge: BrowserLoginBridge = {
  async start(state) {
    if (!isTauri()) throw new Error('请在灵枢桌面软件中使用网页登录。');
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke<string>('cloud_browser_login_start', { state });
  },
  async open(url) { const { openUrl } = await import('@tauri-apps/plugin-opener'); await openUrl(url); },
  async poll(state) { const { invoke } = await import('@tauri-apps/api/core'); return invoke<string | null>('cloud_browser_login_poll', { state }); },
  async cancel(state) { const { invoke } = await import('@tauri-apps/api/core'); await invoke('cloud_browser_login_cancel', { state }); },
};

function randomHex(): string {
  return Array.from(crypto.getRandomValues(new Uint8Array(32)), byte => byte.toString(16).padStart(2, '0')).join('');
}

export async function createLoginProof(): Promise<{ state: string; verifier: string; challenge: string }> {
  const verifier = randomHex();
  const digest = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(verifier));
  const challenge = btoa(String.fromCharCode(...new Uint8Array(digest))).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
  return { state: randomHex(), verifier, challenge };
}

function delay(signal: AbortSignal): Promise<void> {
  return new Promise((resolve, reject) => {
    const abort = () => { clearTimeout(timer); reject(new Error('已取消网页登录。')); };
    const timer = setTimeout(() => { signal.removeEventListener('abort', abort); resolve(); }, 1000);
    if (signal.aborted) abort();
    else signal.addEventListener('abort', abort, { once: true });
  });
}

export async function waitForBrowserLogin(deviceName: string, signal: AbortSignal, bridge: BrowserLoginBridge = nativeBridge): Promise<BrowserLoginGrant> {
  const proof = await createLoginProof();
  const platform = await getPlatformCapabilities();
  let started = false;
  try {
    if (signal.aborted) throw new Error('已取消网页登录。');
    const redirectUri = await bridge.start(proof.state);
    started = true;
    const url = new URL(`${CLOUD_ACCOUNT_URL}/account`);
    url.search = new URLSearchParams({ desktop_login: '1', redirect_uri: redirectUri, state: proof.state,
      code_challenge: proof.challenge, code_challenge_method: 'S256', device_name: deviceName.trim() || '我的灵枢电脑', platform: platform.platform }).toString();
    let deviceId: string | null = null;
    try { deviceId = window.localStorage.getItem('lingshu_cloud_device_id'); } catch { /* optional metadata */ }
    if (deviceId && /^[0-9a-f-]{36}$/i.test(deviceId)) url.searchParams.set('device_id', deviceId);
    if (signal.aborted) throw new Error('已取消网页登录。');
    await bridge.open(url.href);
    const expiresAt = Date.now() + 300_000;
    while (Date.now() < expiresAt) {
      if (signal.aborted) throw new Error('已取消网页登录。');
      const code = await bridge.poll(proof.state);
      if (signal.aborted) throw new Error('已取消网页登录。');
      if (code) return { code, verifier: proof.verifier, redirectUri };
      await delay(signal);
    }
    throw new Error('网页登录已超时，请重试。');
  } finally {
    if (started) await bridge.cancel(proof.state).catch(() => undefined);
  }
}
