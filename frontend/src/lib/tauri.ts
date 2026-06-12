// ── Tauri Bridge (graceful degradation for browser) ────────────────────
//
// When running inside Tauri (`tauri dev` / .app bundle), Tauri APIs are
// available. In a browser (yarn dev), they gracefully degrade to no-ops.
//
// Phase A: window label detection for pet vs main window
// Phase B: invoke() for EventKit commands (create_calendar_event, etc.)

let cachedIsTauri: boolean | null = null;

/** True when running inside a Tauri 2 webview. */
export function isTauri(): boolean {
  if (cachedIsTauri !== null) return cachedIsTauri;

  // Tauri 2 injects `window.__TAURI_INTERNALS__` at runtime.
  // Also check the global `window.__TAURI__` (available with withGlobalTauri).
  cachedIsTauri =
    typeof window !== 'undefined' &&
    ('__TAURI_INTERNALS__' in window || '__TAURI__' in window);

  return cachedIsTauri;
}

/** Get the current webview window label ("main" or "pet"). */
export async function getWindowLabel(): Promise<string> {
  if (!isTauri()) return 'main'; // browser defaults to main

  try {
    const { getCurrentWindow } = await import(
      '@tauri-apps/api/window'
    );
    return getCurrentWindow().label;
  } catch {
    return 'main';
  }
}

/**
 * Invoke a Tauri command (Rust side). Returns null when not in Tauri.
 * Phase B will use this for EventKit bridge calls.
 */
export async function invokeTauri<T>(
  cmd: string,
  args?: Record<string, unknown>
): Promise<T | null> {
  if (!isTauri()) return null;

  try {
    const { invoke } = await import(
      '@tauri-apps/api/core'
    );
    return await invoke<T>(cmd, args);
  } catch (err) {
    console.error(`[tauri] invoke "${cmd}" failed:`, err);
    throw err;
  }
}

/** Cross-window event: tell the main window to switch to a given section. */
export const MAIN_NAVIGATE_EVENT = 'lingshu://navigate-section';

/**
 * Show / focus the main window from the pet window. When `section` is given,
 * also emit a navigate event so the main window switches to that tab (e.g.
 * 'chat' so a long reply is actually visible instead of landing on 'home').
 * No-op in browser mode.
 */
export async function showMainWindow(section?: string): Promise<void> {
  if (!isTauri()) return;

  try {
    const { getAllWebviewWindows } = await import(
      '@tauri-apps/api/webviewWindow'
    );
    const windows = await getAllWebviewWindows();
    const main = windows.find((w) => w.label === 'main');
    if (main) {
      await main.show();
      await main.setFocus();
      if (section) {
        const { emitTo } = await import('@tauri-apps/api/event');
        await emitTo('main', MAIN_NAVIGATE_EVENT, section);
      }
    }
  } catch (err) {
    console.error('[tauri] showMainWindow failed:', err);
  }
}
