// ── L2 Automation Frontend Bridge (Phase 4) ────────────────────────────
//
// Invokes the Tauri automation commands (open app / URL / file). Gracefully
// degrades to no-ops in a browser (non-Tauri).
//
// SECURITY: the backend (lingshu-server) has already enforced the L2 permission
// + whitelist before emitting an action on the chat SSE stream, so this layer
// only forwards approved actions to the OS.

import { isTauri, invokeTauri } from './tauri';

/** Open a macOS application by name. */
export async function openApplication(name: string): Promise<boolean> {
  if (!isTauri()) return false;
  try {
    await invokeTauri('open_application', { name });
    return true;
  } catch (err) {
    console.error('[automation] openApplication failed:', err);
    return false;
  }
}

/** Open a URL in the default browser (http/https only — re-checked natively). */
export async function openUrl(url: string): Promise<boolean> {
  if (!isTauri()) return false;
  try {
    await invokeTauri('open_url', { url });
    return true;
  } catch (err) {
    console.error('[automation] openUrl failed:', err);
    return false;
  }
}

/** Open a local file or folder with its default application. */
export async function openPath(path: string): Promise<boolean> {
  if (!isTauri()) return false;
  try {
    await invokeTauri('open_path', { path });
    return true;
  } catch (err) {
    console.error('[automation] openPath failed:', err);
    return false;
  }
}

/** An automation action emitted on the chat SSE stream by the backend. */
export interface AutomationAction {
  kind: 'open_app' | 'open_url' | 'open_file' | string;
  target: string;
}

/**
 * Read the frontmost window's on-screen text. Runs in the Tauri MAIN-app
 * process (`com.lingshu.desktop`) — the only process whose macOS Accessibility
 * grant the user can actually authorize; the backend sidecar is a different
 * TCC subject and cannot. Throws on permission failure (the Rust command
 * triggers the system prompt and returns a guidance message) so the caller can
 * surface the one-click grant flow.
 */
export async function readScreen(): Promise<string> {
  if (!isTauri()) {
    throw new Error('屏幕识别仅在桌面应用中可用');
  }
  const text = await invokeTauri<string>('read_screen');
  return text ?? '';
}

/** Dispatch one automation action to its matching Tauri command. */
export async function runAutomationAction(action: AutomationAction): Promise<void> {
  switch (action.kind) {
    case 'open_app':
      await openApplication(action.target);
      break;
    case 'open_url':
      await openUrl(action.target);
      break;
    case 'open_file':
      await openPath(action.target);
      break;
    default:
      console.warn('[automation] unknown action kind:', action.kind);
  }
}
