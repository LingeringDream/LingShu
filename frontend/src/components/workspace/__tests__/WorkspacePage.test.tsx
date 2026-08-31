import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { WorkspacePage } from '../WorkspacePage';
import { apiFetch } from '../../../lib/api';
import { exportCalendarEvent } from '../../../lib/ics';
import { useProjectStore } from '../../../stores/projectStore';

const { platformState } = vi.hoisted(() => ({
  platformState: { calendarProvider: 'none' as 'none' | 'eventkit' | 'ics' },
}));

vi.mock('../../../lib/api', () => ({
  apiFetch: vi.fn(),
}));

vi.mock('../../../lib/tauri', () => ({
  getPlatformCapabilities: () => Promise.resolve({
    platform: platformState.calendarProvider === 'eventkit' ? 'macos' : 'windows',
    isDesktop: true,
    calendarProvider: platformState.calendarProvider,
    screenReader: false,
    applicationAutomation: false,
    urlOpener: false,
    pathOpener: false,
    fileReader: false,
    pdfReader: false,
    desktopPet: true,
  }),
}));

vi.mock('../../../lib/eventkit', () => ({
  syncEventToAppleCalendar: vi.fn(),
  deleteAppleCalendarEvent: vi.fn(),
  updateAppleCalendarEvent: vi.fn(),
}));

vi.mock('../../../lib/ics', () => ({
  exportCalendarEvent: vi.fn(),
}));

const mockedApiFetch = vi.mocked(apiFetch);
const mockedExportCalendarEvent = vi.mocked(exportCalendarEvent);

function jsonResponse(body: unknown, ok = true, status = 200): Response {
  return {
    ok,
    status,
    json: () => Promise.resolve(body),
  } as Response;
}

function localInputValue(date: Date): string {
  const pad = (n: number) => String(n).padStart(2, '0');
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}T${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

function utcFromLocalInput(value: string): string {
  return new Date(value).toISOString();
}

/// The workspace only lists events whose next occurrence is still ahead
/// (`prepareCalendarEventsForDisplay` drops confirmed events once they start),
/// so fixtures must be anchored to the clock rather than hardcoded — a literal
/// date silently stops rendering the moment it passes, and the test then fails
/// on a missing 编辑 button instead of on whatever it meant to assert.
function upcomingEventTimes(durationMinutes: number): { start: string; end: string } {
  const start = new Date();
  start.setDate(start.getDate() + 7);
  start.setHours(10, 0, 0, 0);
  return {
    start: start.toISOString(),
    end: new Date(start.getTime() + durationMinutes * 60_000).toISOString(),
  };
}

describe('WorkspacePage', () => {
  beforeEach(() => {
    mockedApiFetch.mockReset();
    mockedExportCalendarEvent.mockReset();
    mockedExportCalendarEvent.mockResolvedValue({ method: 'browser', path: 'event.ics' });
    platformState.calendarProvider = 'none';
    useProjectStore.setState({
      projects: [],
      loading: false,
      error: null,
    });
  });

  it('saves the displayed occurrence time when editing a recurring calendar event', async () => {
    const now = new Date();
    now.setSeconds(0, 0);
    const originalStart = new Date(now.getTime() - 2 * 24 * 60 * 60 * 1000 + 60 * 60 * 1000);
    const originalEnd = new Date(originalStart.getTime() + 30 * 60 * 1000);
    const displayedStart = new Date(originalStart.getTime() + 2 * 24 * 60 * 60 * 1000);
    const displayedEnd = new Date(originalEnd.getTime() + 2 * 24 * 60 * 60 * 1000);
    const displayedStartInput = localInputValue(displayedStart);
    const displayedEndInput = localInputValue(displayedEnd);

    const event = {
      id: 'daily-review',
      title: '每日复盘',
      description: null,
      location: '书房',
      start_time: originalStart.toISOString(),
      end_time: originalEnd.toISOString(),
      status: 'confirmed',
      calendar_name: 'default',
      attendees: [],
      recurrence: { frequency: 'daily', interval: 1 },
      parse_confidence: null,
      apple_event_id: null,
      external_event_id: null,
    };

    mockedApiFetch.mockImplementation(async (input, init) => {
      if (input === '/api/v1/calendar/events?limit=20' && !init?.method) {
        return jsonResponse([event]);
      }
      if (input === '/api/v1/projects' && !init?.method) {
        return jsonResponse([]);
      }
      if (input === '/api/v1/calendar/events/daily-review' && init?.method === 'PATCH') {
        return jsonResponse({
          ...event,
          ...JSON.parse(String(init.body)),
        });
      }
      throw new Error(`Unexpected request: ${input}`);
    });

    render(<WorkspacePage />);

    fireEvent.click(await screen.findByRole('button', { name: '编辑' }));

    expect(screen.getByLabelText('开始日期')).toHaveValue(displayedStartInput.slice(0, 10));
    expect(screen.getByLabelText('开始时间')).toHaveValue(displayedStartInput.slice(11, 16));
    expect(screen.getByLabelText('结束日期')).toHaveValue(displayedEndInput.slice(0, 10));
    expect(screen.getByLabelText('结束时间')).toHaveValue(displayedEndInput.slice(11, 16));

    fireEvent.click(screen.getByRole('button', { name: '保存' }));

    await waitFor(() => {
      expect(mockedApiFetch).toHaveBeenCalledWith(
        '/api/v1/calendar/events/daily-review',
        expect.objectContaining({ method: 'PATCH' }),
      );
    });

    const updateCall = mockedApiFetch.mock.calls.find(
      ([input, init]) => input === '/api/v1/calendar/events/daily-review' && init?.method === 'PATCH',
    );
    expect(updateCall).toBeDefined();
    const body = JSON.parse(String(updateCall?.[1]?.body));
    expect(body.start_time).toBe(utcFromLocalInput(displayedStartInput));
    expect(body.end_time).toBe(utcFromLocalInput(displayedEndInput));
  });

  it('lets users edit a precise start time while preserving the event duration', async () => {
    const times = upcomingEventTimes(45);
    const event = {
      id: 'math-review',
      title: '数学复盘',
      description: null,
      location: '书房',
      start_time: times.start,
      end_time: times.end,
      status: 'confirmed',
      calendar_name: 'default',
      attendees: [],
      recurrence: { frequency: 'none', interval: 1 },
      parse_confidence: null,
      apple_event_id: null,
      external_event_id: null,
    };

    mockedApiFetch.mockImplementation(async (input, init) => {
      if (input === '/api/v1/calendar/events?limit=20' && !init?.method) {
        return jsonResponse([event]);
      }
      if (input === '/api/v1/projects' && !init?.method) {
        return jsonResponse([]);
      }
      if (input === '/api/v1/calendar/events/math-review' && init?.method === 'PATCH') {
        return jsonResponse({
          ...event,
          ...JSON.parse(String(init.body)),
        });
      }
      throw new Error(`Unexpected request: ${input}`);
    });

    render(<WorkspacePage />);

    fireEvent.click(await screen.findByRole('button', { name: '编辑' }));
    fireEvent.input(screen.getByLabelText('开始时间'), { target: { value: '11:15' } });
    fireEvent.change(screen.getByLabelText('开始时间'), { target: { value: '11:15' } });
    expect(screen.getByLabelText('结束时间')).toHaveValue('12:00');
    fireEvent.click(screen.getByRole('button', { name: '保存' }));

    await waitFor(() => {
      expect(mockedApiFetch).toHaveBeenCalledWith(
        '/api/v1/calendar/events/math-review',
        expect.objectContaining({ method: 'PATCH' }),
      );
    });

    const updateCall = mockedApiFetch.mock.calls.find(
      ([input, init]) => input === '/api/v1/calendar/events/math-review' && init?.method === 'PATCH',
    );
    expect(updateCall).toBeDefined();
    const body = JSON.parse(String(updateCall?.[1]?.body));
    const date = localInputValue(new Date(event.start_time)).slice(0, 10);
    expect(body.start_time).toBe(utcFromLocalInput(`${date}T11:15`));
    expect(body.end_time).toBe(utcFromLocalInput(`${date}T12:00`));
  });

  it('reads the visible time input values on save even if native input state is stale', async () => {
    const times = upcomingEventTimes(45);
    const event = {
      id: 'native-time',
      title: '原生时间输入',
      description: null,
      location: '书房',
      start_time: times.start,
      end_time: times.end,
      status: 'confirmed',
      calendar_name: 'default',
      attendees: [],
      recurrence: { frequency: 'none', interval: 1 },
      parse_confidence: null,
      apple_event_id: null,
      external_event_id: null,
    };

    mockedApiFetch.mockImplementation(async (input, init) => {
      if (input === '/api/v1/calendar/events?limit=20' && !init?.method) {
        return jsonResponse([event]);
      }
      if (input === '/api/v1/projects' && !init?.method) {
        return jsonResponse([]);
      }
      if (input === '/api/v1/calendar/events/native-time' && init?.method === 'PATCH') {
        return jsonResponse({
          ...event,
          ...JSON.parse(String(init.body)),
        });
      }
      throw new Error(`Unexpected request: ${input}`);
    });

    render(<WorkspacePage />);

    fireEvent.click(await screen.findByRole('button', { name: '编辑' }));
    const startTimeInput = screen.getByLabelText('开始时间') as HTMLInputElement;
    startTimeInput.value = '11:15';
    fireEvent.click(screen.getByRole('button', { name: '保存' }));

    await waitFor(() => {
      expect(mockedApiFetch).toHaveBeenCalledWith(
        '/api/v1/calendar/events/native-time',
        expect.objectContaining({ method: 'PATCH' }),
      );
    });

    const updateCall = mockedApiFetch.mock.calls.find(
      ([input, init]) => input === '/api/v1/calendar/events/native-time' && init?.method === 'PATCH',
    );
    expect(updateCall).toBeDefined();
    const body = JSON.parse(String(updateCall?.[1]?.body));
    const date = localInputValue(new Date(event.start_time)).slice(0, 10);
    expect(body.start_time).toBe(utcFromLocalInput(`${date}T11:15`));
    expect(body.end_time).toBe(utcFromLocalInput(`${date}T12:00`));
  });

  it('hides Apple Calendar sync when the desktop platform has no EventKit provider', async () => {
    const times = upcomingEventTimes(30);
    const event = {
      id: 'windows-event',
      title: '跨平台评审',
      description: null,
      location: '线上',
      start_time: times.start,
      end_time: times.end,
      status: 'confirmed',
      calendar_name: 'default',
      attendees: [],
      recurrence: { frequency: 'none', interval: 1 },
      parse_confidence: null,
      apple_event_id: null,
      external_event_id: null,
    };

    mockedApiFetch.mockImplementation(async (input, init) => {
      if (input === '/api/v1/calendar/events?limit=20' && !init?.method) return jsonResponse([event]);
      if (input === '/api/v1/projects' && !init?.method) return jsonResponse([]);
      throw new Error(`Unexpected request: ${input}`);
    });

    render(<WorkspacePage />);

    expect(await screen.findByText('跨平台评审')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '同步到系统日历' })).toBeNull();
  });

  it('shows native calendar sync when EventKit is available', async () => {
    platformState.calendarProvider = 'eventkit';
    const times = upcomingEventTimes(30);
    const event = {
      id: 'mac-event',
      title: 'macOS评审',
      description: null,
      location: '线上',
      start_time: times.start,
      end_time: times.end,
      status: 'confirmed',
      calendar_name: 'default',
      attendees: [],
      recurrence: { frequency: 'none', interval: 1 },
      parse_confidence: null,
      apple_event_id: null,
      external_event_id: null,
    };

    mockedApiFetch.mockImplementation(async (input, init) => {
      if (input === '/api/v1/calendar/events?limit=20' && !init?.method) return jsonResponse([event]);
      if (input === '/api/v1/projects' && !init?.method) return jsonResponse([]);
      throw new Error(`Unexpected request: ${input}`);
    });

    render(<WorkspacePage />);

    expect(await screen.findByRole('button', { name: '同步到系统日历' })).toBeInTheDocument();
  });

  it('exports an ICS file when the platform uses the portable calendar provider', async () => {
    platformState.calendarProvider = 'ics';
    const times = upcomingEventTimes(30);
    const event = {
      id: 'portable-event',
      title: '跨平台答辩',
      description: '校赛演示',
      location: '报告厅',
      start_time: times.start,
      end_time: times.end,
      status: 'confirmed',
      calendar_name: 'default',
      attendees: [],
      recurrence: { frequency: 'none', interval: 1 },
      parse_confidence: null,
      apple_event_id: null,
      external_event_id: null,
    };

    mockedApiFetch.mockImplementation(async (input, init) => {
      if (input === '/api/v1/calendar/events?limit=20' && !init?.method) return jsonResponse([event]);
      if (input === '/api/v1/projects' && !init?.method) return jsonResponse([]);
      throw new Error(`Unexpected request: ${input}`);
    });

    render(<WorkspacePage />);
    fireEvent.click(await screen.findByRole('button', { name: '导出日历文件' }));

    await waitFor(() => expect(mockedExportCalendarEvent).toHaveBeenCalledWith({
      id: event.id,
      title: event.title,
      start_time: event.start_time,
      end_time: event.end_time,
      location: event.location,
      description: event.description,
      recurrence: event.recurrence,
    }));
    expect(await screen.findByRole('button', { name: '已下载日历文件' })).toBeInTheDocument();
  });
});
