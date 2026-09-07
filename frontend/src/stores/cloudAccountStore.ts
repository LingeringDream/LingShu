import { create } from 'zustand';
import { cloudAccountClient, CloudAccountError, cloudErrorMessage, type CloudAccount } from '../lib/cloudAccount';
import { waitForBrowserLogin } from '../lib/browserLogin';
import { stopProAutoSync } from '../lib/proSync';

interface CloudAccountState {
  account: CloudAccount | null;
  busy: boolean;
  initialized: boolean;
  error: string | null;
  browserPending: boolean;
  loginWithBrowser(deviceName: string): Promise<void>;
  cancelBrowserLogin(): void;
  initialize(): Promise<void>;
  reload(): Promise<void>;
  login(email: string, password: string, deviceName: string, platform: string): Promise<void>;
  logout(): Promise<void>;
}

export const useCloudAccountStore = create<CloudAccountState>((set, get) => {
  let browserController: AbortController | null = null;
  async function run(operation: () => Promise<CloudAccount | null>) {
    if (get().busy) return;
    set({ busy: true, error: null });
    try { set({ account: await operation() }); }
    catch (error) {
      set({ error: cloudErrorMessage(error), ...(error instanceof CloudAccountError && error.status === 401 ? { account: null } : {}) });
    } finally { set({ busy: false, initialized: true }); }
  }
  return {
    account: null, busy: false, initialized: false, error: null, browserPending: false,
    async loginWithBrowser(deviceName) {
      if (get().busy) return;
      const controller = new AbortController();
      browserController = controller;
      set({ browserPending: true });
      await run(async () => {
        try {
          const grant = await waitForBrowserLogin(deviceName, controller.signal);
          browserController = null;
          set({ browserPending: false });
          return await cloudAccountClient.completeBrowserLogin(grant.code, grant.verifier, grant.redirectUri);
        } finally { browserController = null; set({ browserPending: false }); }
      });
    },
    cancelBrowserLogin() { browserController?.abort(); },
    async initialize() { if (!get().initialized) await get().reload(); },
    reload: () => run(() => cloudAccountClient.load()),
    login: (email, password, name, platform) => run(() => cloudAccountClient.login(email, password, name, platform)),
    logout: () => run(async () => { await cloudAccountClient.logout(); stopProAutoSync(); return null; }),
  };
});
