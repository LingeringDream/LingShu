import { describe, it, expect } from 'vitest';
import { getPlatformCapabilities, isTauri } from '../tauri';

describe('isTauri', () => {
  it('returns false in browser (non-Tauri env)', () => {
    // In vitest/jsdom, window.__TAURI__ is undefined
    expect(isTauri()).toBe(false);
  });

  it('returns conservative browser capabilities outside Tauri', async () => {
    const capabilities = await getPlatformCapabilities();
    expect(capabilities).toMatchObject({
      platform: 'browser',
      isDesktop: false,
      calendarProvider: 'ics',
      screenReader: false,
      applicationAutomation: false,
      desktopPet: false,
    });
  });
});
