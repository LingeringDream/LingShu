import { describe, expect, it, vi } from 'vitest';
import {
  DESKTOP_BACKEND_STARTUP_TIMEOUT_MS,
  WEB_BACKEND_STARTUP_TIMEOUT_MS,
  retryLocalSession,
} from '../localSessionBoot';

describe('retryLocalSession', () => {
  it('keeps retrying until the desktop backend becomes ready', async () => {
    const request = vi
      .fn<() => Promise<string>>()
      .mockRejectedValueOnce(new TypeError('Failed to fetch'))
      .mockRejectedValueOnce(new TypeError('Failed to fetch'))
      .mockResolvedValue('ready');
    const sleep = vi.fn(async () => undefined);

    await expect(
      retryLocalSession({ request, timeoutMs: DESKTOP_BACKEND_STARTUP_TIMEOUT_MS, sleep }),
    ).resolves.toBe('ready');
    expect(request).toHaveBeenCalledTimes(3);
    expect(sleep).toHaveBeenCalledTimes(2);
  });

  it('returns the last startup error after the deadline', async () => {
    const lastError = new Error('backend unavailable');
    const request = vi.fn<() => Promise<never>>().mockRejectedValue(lastError);
    const now = vi.fn().mockReturnValueOnce(1_000).mockReturnValue(31_000);

    await expect(
      retryLocalSession({
        request,
        timeoutMs: WEB_BACKEND_STARTUP_TIMEOUT_MS,
        now,
        sleep: async () => undefined,
      }),
    ).rejects.toBe(lastError);
    expect(request).toHaveBeenCalledOnce();
  });
});
