import { describe, expect, it } from 'vitest';
import {
  collectDueCalendarReminders,
  prepareCalendarEventsForDisplay,
  type CalendarPresentationEvent,
} from '../calendarPresentation';

function event(overrides: Partial<CalendarPresentationEvent>): CalendarPresentationEvent {
  return {
    id: 'event-1',
    title: '近现代史纲要',
    start_time: '2026-06-16T13:30:00.000Z',
    end_time: '2026-06-16T14:30:00.000Z',
    status: 'confirmed',
    ...overrides,
  };
}

describe('calendarPresentation', () => {
  it('hides confirmed events after their start time has passed', () => {
    const now = new Date('2026-06-16T13:31:00.000Z');
    const visible = prepareCalendarEventsForDisplay([
      event({ id: 'past-confirmed', status: 'confirmed', start_time: '2026-06-16T13:30:00.000Z' }),
      event({ id: 'future-confirmed', status: 'confirmed', start_time: '2026-06-16T13:45:00.000Z' }),
    ], now);

    expect(visible.map((item) => item.id)).toEqual(['future-confirmed']);
  });

  it('keeps overdue pending events visible and labels them as overdue unconfirmed', () => {
    const now = new Date('2026-06-16T13:31:00.000Z');
    const visible = prepareCalendarEventsForDisplay([
      event({
        id: 'past-pending',
        status: 'pending_confirmation',
        start_time: '2026-06-16T13:30:00.000Z',
      }),
    ], now);

    expect(visible).toHaveLength(1);
    expect(visible[0]).toMatchObject({
      id: 'past-pending',
      displayStatus: '已过期未确认',
      urgency: 'overdue_pending',
    });
  });

  it('collects one reminder for events that are coming soon and have not been reminded', () => {
    const now = new Date('2026-06-16T13:20:00.000Z');
    const due = collectDueCalendarReminders([
      event({ id: 'soon', title: '中国近现代史纲要', start_time: '2026-06-16T13:30:00.000Z' }),
      event({ id: 'later', title: '晚课', start_time: '2026-06-16T15:30:00.000Z' }),
      event({ id: 'already', title: '已提醒', start_time: '2026-06-16T13:25:00.000Z' }),
    ], now, new Set(['already']));

    expect(due).toEqual([
      {
        id: 'soon',
        title: '中国近现代史纲要',
        minutesUntilStart: 10,
        message: '10 分钟后：中国近现代史纲要',
      },
    ]);
  });

  it('shows the next occurrence for a recurring event whose first start time has passed', () => {
    const now = new Date('2026-06-16T13:20:00.000Z');
    const visible = prepareCalendarEventsForDisplay([
      event({
        id: 'daily-standup',
        title: '每日站会',
        start_time: '2026-06-15T13:30:00.000Z',
        end_time: '2026-06-15T13:45:00.000Z',
        recurrence: { frequency: 'daily', interval: 1 },
      }),
    ], now);

    expect(visible).toHaveLength(1);
    expect(visible[0].startsAt.toISOString()).toBe('2026-06-16T13:30:00.000Z');
    expect(visible[0].endsAt.toISOString()).toBe('2026-06-16T13:45:00.000Z');
    expect(visible[0].displayStatus).toBe('已确认');
  });

  it('reminds for the upcoming occurrence of a recurring event', () => {
    const now = new Date('2026-06-16T13:20:00.000Z');
    const due = collectDueCalendarReminders([
      event({
        id: 'daily-review',
        title: '每日复盘',
        start_time: '2026-06-15T13:30:00.000Z',
        end_time: '2026-06-15T13:45:00.000Z',
        recurrence: { frequency: 'daily', interval: 1 },
      }),
    ], now);

    expect(due).toEqual([
      {
        id: 'daily-review:2026-06-16T13:30:00.000Z',
        title: '每日复盘',
        minutesUntilStart: 10,
        message: '10 分钟后：每日复盘',
      },
    ]);
  });
});
