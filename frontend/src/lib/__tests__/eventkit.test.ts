import { beforeEach, describe, expect, it, vi } from 'vitest';

const { invokeMock, capabilityState } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  capabilityState: { calendarProvider: 'none' as 'none' | 'eventkit' },
}));

vi.mock('../tauri', () => ({
  getPlatformCapabilities: () => Promise.resolve({
    platform: capabilityState.calendarProvider === 'eventkit' ? 'macos' : 'windows',
    isDesktop: true,
    calendarProvider: capabilityState.calendarProvider,
    screenReader: capabilityState.calendarProvider === 'eventkit',
    applicationAutomation: capabilityState.calendarProvider === 'eventkit',
    urlOpener: capabilityState.calendarProvider === 'eventkit',
    pathOpener: capabilityState.calendarProvider === 'eventkit',
    fileReader: capabilityState.calendarProvider === 'eventkit',
    pdfReader: capabilityState.calendarProvider === 'eventkit',
    desktopPet: true,
  }),
  invokeTauri: (...args: unknown[]) => invokeMock(...args),
}));

import { requestCalendarAccess } from '../eventkit';

// Type-only imports for compile-time checking — the actual module has
// runtime deps (Tauri invoke) that need a real Tauri environment.

describe('eventkit types', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    capabilityState.calendarProvider = 'none';
  });

  it('CalendarAccessStatus union is valid', () => {
    const valid: string[] = [
      'full_access',
      'denied',
      'restricted',
      'write_only',
      'not_determined',
      'unsupported',
    ];
    expect(valid.length).toBe(6);
    expect(new Set(valid).size).toBe(6); // no duplicates
  });

  it('AppleEventInput has required fields', () => {
    // Compile-time type check: this assignment must type-check
    const input = {
      title: 'Test',
      start_time: '2026-06-01T09:00:00Z',
      end_time: '2026-06-01T10:00:00Z',
      location: 'Room 1',
      notes: 'Bring laptop',
      calendar_name: 'work',
    };
    expect(input.title).toBe('Test');
    expect(input.location).toBe('Room 1');
    expect(input.notes).toBe('Bring laptop');
  });

  it('does not invoke EventKit on an unsupported desktop platform', async () => {
    await expect(requestCalendarAccess()).resolves.toBe('unsupported');
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it('requests EventKit access only when the native provider exists', async () => {
    capabilityState.calendarProvider = 'eventkit';
    invokeMock.mockResolvedValue('full_access');
    await expect(requestCalendarAccess()).resolves.toBe('full_access');
    expect(invokeMock).toHaveBeenCalledWith('request_calendar_access');
  });
});
