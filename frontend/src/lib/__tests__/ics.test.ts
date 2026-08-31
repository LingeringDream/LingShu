import { beforeEach, describe, expect, it, vi } from 'vitest';

const { invokeMock, capabilityState } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  capabilityState: { isDesktop: false },
}));

vi.mock('../tauri', () => ({
  getPlatformCapabilities: () => Promise.resolve({
    platform: capabilityState.isDesktop ? 'windows' : 'browser',
    isDesktop: capabilityState.isDesktop,
    calendarProvider: 'ics',
    screenReader: false,
    applicationAutomation: false,
    urlOpener: false,
    pathOpener: false,
    fileReader: false,
    pdfReader: false,
    desktopPet: capabilityState.isDesktop,
  }),
  invokeTauri: (...args: unknown[]) => invokeMock(...args),
}));

import { buildCalendarIcs, exportCalendarEvent, type CalendarExportEvent } from '../ics';

const event: CalendarExportEvent = {
  id: 'school-final',
  title: '校赛答辩, 第一组',
  start_time: '2026-09-08T01:00:00.000Z',
  end_time: '2026-09-08T02:30:00.000Z',
  location: '报告厅; A区',
  description: '展示灵枢\n跨平台版本',
  recurrence: { frequency: 'weekly', interval: 2, count: 3 },
};

describe('portable ICS export', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    capabilityState.isDesktop = false;
  });

  it('builds a standards-shaped UTC calendar with escaped text and recurrence', () => {
    const ics = buildCalendarIcs(event, new Date('2026-08-31T00:00:00.000Z'));
    expect(ics).toContain('BEGIN:VCALENDAR\r\nVERSION:2.0\r\n');
    expect(ics).toContain('DTSTART:20260908T010000Z');
    expect(ics).toContain('SUMMARY:校赛答辩\\, 第一组');
    expect(ics).toContain('LOCATION:报告厅\\; A区');
    expect(ics).toContain('DESCRIPTION:展示灵枢\\n跨平台版本');
    expect(ics).toContain('RRULE:FREQ=WEEKLY;INTERVAL=2;COUNT=3');
    expect(ics.endsWith('END:VCALENDAR\r\n')).toBe(true);
  });

  it('uses a browser download outside the desktop shell', async () => {
    const createObjectURL = vi.fn(() => 'blob:calendar');
    const revokeObjectURL = vi.fn();
    vi.stubGlobal('URL', { createObjectURL, revokeObjectURL });
    const click = vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => {});

    await expect(exportCalendarEvent(event)).resolves.toMatchObject({ method: 'browser' });
    expect(createObjectURL).toHaveBeenCalledOnce();
    expect(click).toHaveBeenCalledOnce();
    expect(invokeMock).not.toHaveBeenCalled();

    click.mockRestore();
    vi.unstubAllGlobals();
  });

  it('writes through the constrained native command on desktop platforms', async () => {
    capabilityState.isDesktop = true;
    invokeMock.mockResolvedValue('C:\\Users\\student\\Downloads\\答辩.ics');

    await expect(exportCalendarEvent(event)).resolves.toEqual({
      method: 'native',
      path: 'C:\\Users\\student\\Downloads\\答辩.ics',
    });
    expect(invokeMock).toHaveBeenCalledWith(
      'export_calendar_file',
      expect.objectContaining({
        fileName: expect.stringMatching(/\.ics$/),
        content: expect.stringContaining('BEGIN:VCALENDAR'),
      }),
    );
  });
});
