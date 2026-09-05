/* global localStorage */
import { beforeEach, describe, expect, it } from 'vitest';
import { AVATAR_CONTROL_STORAGE_KEY } from '../../components/avatar/avatarControls';
import {
  CALENDAR_REMINDER_EVENT,
  publishCalendarReminder,
  type CalendarReminderPayload,
} from '../calendarReminderEvents';

describe('calendarReminderEvents', () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it('publishes a runtime reminder without persisting avatar control settings', async () => {
    const payload: CalendarReminderPayload = {
      id: 'event-1',
      title: '中国近现代史纲要',
      message: '10 分钟后：中国近现代史纲要',
      minutesUntilStart: 10,
    };
    const received = new Promise((resolve) => {
      window.addEventListener(
        CALENDAR_REMINDER_EVENT,
        (event) => resolve((event as CustomEvent).detail),
        { once: true },
      );
    });

    await publishCalendarReminder(payload);

    await expect(received).resolves.toEqual(payload);
    expect(localStorage.getItem(AVATAR_CONTROL_STORAGE_KEY)).toBeNull();
  });
});
