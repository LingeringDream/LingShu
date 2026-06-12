import { describe, it, expect, vi, beforeEach } from 'vitest';

// Hoisted so the vi.mock factory below can reference them safely.
const { invokeMock, env } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  env: { tauri: true },
}));

vi.mock('../tauri', () => ({
  isTauri: () => env.tauri,
  invokeTauri: (...args: unknown[]) => invokeMock(...args),
}));

import { runAutomationAction, openApplication } from '../automation';

describe('automation dispatcher', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(undefined);
    env.tauri = true;
  });

  it('routes open_app → open_application', async () => {
    await runAutomationAction({ kind: 'open_app', target: 'Calculator' });
    expect(invokeMock).toHaveBeenCalledWith('open_application', { name: 'Calculator' });
  });

  it('routes open_url → open_url', async () => {
    await runAutomationAction({ kind: 'open_url', target: 'https://github.com' });
    expect(invokeMock).toHaveBeenCalledWith('open_url', { url: 'https://github.com' });
  });

  it('routes open_file → open_path', async () => {
    await runAutomationAction({ kind: 'open_file', target: '/tmp/x.txt' });
    expect(invokeMock).toHaveBeenCalledWith('open_path', { path: '/tmp/x.txt' });
  });

  it('ignores unknown kinds without invoking', async () => {
    await runAutomationAction({ kind: 'open_evil', target: 'x' });
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it('no-ops in a browser (non-Tauri)', async () => {
    env.tauri = false;
    const ok = await openApplication('Calculator');
    expect(ok).toBe(false);
    expect(invokeMock).not.toHaveBeenCalled();
  });
});
