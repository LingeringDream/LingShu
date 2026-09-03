export const DESKTOP_BACKEND_STARTUP_TIMEOUT_MS = 120_000;
export const WEB_BACKEND_STARTUP_TIMEOUT_MS = 30_000;

interface RetryLocalSessionOptions<T> {
  request: () => Promise<T>;
  timeoutMs: number;
  now?: () => number;
  sleep?: (delayMs: number) => Promise<void>;
}

/**
 * Retry the local-session handshake while the bundled backend is starting.
 *
 * The Windows desktop build may spend noticeably longer on its first launch
 * because it has to extract and initialise the embedded PostgreSQL runtime.
 */
export async function retryLocalSession<T>({
  request,
  timeoutMs,
  now = Date.now,
  sleep = (delayMs) => new Promise((resolve) => setTimeout(resolve, delayMs)),
}: RetryLocalSessionOptions<T>): Promise<T> {
  const deadlineMs = now() + timeoutMs;
  let lastError: unknown;

  for (let attempt = 0; ; attempt++) {
    try {
      return await request();
    } catch (error) {
      lastError = error;
      if (now() >= deadlineMs) {
        throw lastError;
      }
      await sleep(Math.min(500 + attempt * 250, 2_000));
    }
  }
}
