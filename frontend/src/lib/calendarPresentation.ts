export interface CalendarPresentationEvent {
  id: string;
  title: string;
  start_time: string;
  end_time: string;
  status: string;
  location?: string;
  recurrence?: CalendarRecurrenceRule | null;
}

export type CalendarRecurrenceFrequency = 'none' | 'daily' | 'weekly' | 'monthly' | 'yearly';

export interface CalendarRecurrenceRule {
  frequency?: CalendarRecurrenceFrequency;
  interval?: number;
  until?: string | null;
  count?: number | null;
}

export type CalendarUrgency = 'active' | 'muted' | 'soon' | 'overdue_pending';

export interface DisplayCalendarEvent extends CalendarPresentationEvent {
  displayStatus: string;
  urgency: CalendarUrgency;
  startsAt: Date;
  endsAt: Date;
  occurrenceId: string;
}

export interface CalendarReminder {
  id: string;
  title: string;
  minutesUntilStart: number;
  message: string;
}

export const CALENDAR_REMINDER_WINDOW_MINUTES = 15;

function isPendingConfirmation(event: CalendarPresentationEvent): boolean {
  return event.status === 'pending_confirmation';
}

function parseDate(raw: string): Date | null {
  const parsed = new Date(raw);
  return Number.isNaN(parsed.getTime()) ? null : parsed;
}

function isValidFrequency(value: string | undefined): value is CalendarRecurrenceFrequency {
  return value === 'none' || value === 'daily' || value === 'weekly' || value === 'monthly' || value === 'yearly';
}

function recurrenceFrequency(event: CalendarPresentationEvent): CalendarRecurrenceFrequency {
  const frequency = event.recurrence?.frequency;
  return isValidFrequency(frequency) ? frequency : 'none';
}

function recurrenceInterval(event: CalendarPresentationEvent): number {
  const interval = event.recurrence?.interval;
  return typeof interval === 'number' && Number.isFinite(interval) && interval >= 1
    ? Math.floor(interval)
    : 1;
}

export function isRecurringEvent(event: CalendarPresentationEvent): boolean {
  return recurrenceFrequency(event) !== 'none';
}

export function formatRecurrenceLabel(recurrence?: CalendarRecurrenceRule | null): string {
  const frequency = isValidFrequency(recurrence?.frequency) ? recurrence.frequency : 'none';
  const interval = typeof recurrence?.interval === 'number' && recurrence.interval > 1
    ? Math.floor(recurrence.interval)
    : 1;
  if (frequency === 'none') return '一次';
  const base = {
    daily: '每天',
    weekly: '每周',
    monthly: '每月',
    yearly: '每年',
  }[frequency];
  if (interval <= 1) return base;
  return `每 ${interval} ${{
    daily: '天',
    weekly: '周',
    monthly: '月',
    yearly: '年',
  }[frequency]}`;
}

function daysInUtcMonth(year: number, month: number): number {
  return new Date(Date.UTC(year, month + 1, 0)).getUTCDate();
}

function addMonthsUtc(date: Date, months: number, preferredDay: number): Date {
  const firstOfMonth = new Date(Date.UTC(
    date.getUTCFullYear(),
    date.getUTCMonth() + months,
    1,
    date.getUTCHours(),
    date.getUTCMinutes(),
    date.getUTCSeconds(),
    date.getUTCMilliseconds(),
  ));
  firstOfMonth.setUTCDate(Math.min(preferredDay, daysInUtcMonth(
    firstOfMonth.getUTCFullYear(),
    firstOfMonth.getUTCMonth(),
  )));
  return firstOfMonth;
}

function addYearsUtc(date: Date, years: number, preferredDay: number): Date {
  const next = new Date(Date.UTC(
    date.getUTCFullYear() + years,
    date.getUTCMonth(),
    1,
    date.getUTCHours(),
    date.getUTCMinutes(),
    date.getUTCSeconds(),
    date.getUTCMilliseconds(),
  ));
  next.setUTCDate(Math.min(preferredDay, daysInUtcMonth(next.getUTCFullYear(), next.getUTCMonth())));
  return next;
}

function addOccurrence(
  candidate: Date,
  event: CalendarPresentationEvent,
  preferredDay: number,
): Date {
  const interval = recurrenceInterval(event);
  switch (recurrenceFrequency(event)) {
    case 'daily':
      return new Date(candidate.getTime() + interval * 24 * 60 * 60 * 1000);
    case 'weekly':
      return new Date(candidate.getTime() + interval * 7 * 24 * 60 * 60 * 1000);
    case 'monthly':
      return addMonthsUtc(candidate, interval, preferredDay);
    case 'yearly':
      return addYearsUtc(candidate, interval, preferredDay);
    case 'none':
    default:
      return candidate;
  }
}

function nextOccurrence(
  event: CalendarPresentationEvent,
  now: Date,
): { startsAt: Date; endsAt: Date; occurrenceId: string } | null {
  const start = parseDate(event.start_time);
  if (!start) return null;
  const end = parseDate(event.end_time) ?? start;
  const durationMs = Math.max(0, end.getTime() - start.getTime());

  if (!isRecurringEvent(event) || isPendingConfirmation(event)) {
    return {
      startsAt: start,
      endsAt: new Date(start.getTime() + durationMs),
      occurrenceId: event.id,
    };
  }

  const until = event.recurrence?.until ? parseDate(event.recurrence.until) : null;
  const count = typeof event.recurrence?.count === 'number' && event.recurrence.count >= 1
    ? Math.floor(event.recurrence.count)
    : null;
  const preferredDay = start.getUTCDate();
  let candidate = start;
  let occurrenceNumber = 1;

  for (let guard = 0; guard < 10_000 && candidate <= now; guard += 1) {
    if (count !== null && occurrenceNumber >= count) return null;
    candidate = addOccurrence(candidate, event, preferredDay);
    occurrenceNumber += 1;
  }

  if (until && candidate > until) return null;
  return {
    startsAt: candidate,
    endsAt: new Date(candidate.getTime() + durationMs),
    occurrenceId: `${event.id}:${candidate.toISOString()}`,
  };
}

function displayStatusFor(event: CalendarPresentationEvent, startsAt: Date, now: Date): string {
  if (isPendingConfirmation(event) && startsAt <= now) return '已过期未确认';
  if (isPendingConfirmation(event)) return '待确认';
  if (event.status === 'confirmed') return '已确认';
  return event.status;
}

function urgencyFor(event: CalendarPresentationEvent, startsAt: Date, now: Date): CalendarUrgency {
  if (isPendingConfirmation(event) && startsAt <= now) return 'overdue_pending';
  if (isPendingConfirmation(event)) return 'muted';
  const minutesUntilStart = (startsAt.getTime() - now.getTime()) / 60_000;
  if (minutesUntilStart >= 0 && minutesUntilStart <= CALENDAR_REMINDER_WINDOW_MINUTES) return 'soon';
  return event.status === 'confirmed' ? 'active' : 'muted';
}

export function prepareCalendarEventsForDisplay<T extends CalendarPresentationEvent>(
  events: T[],
  now = new Date(),
): Array<T & DisplayCalendarEvent> {
  return events
    .map((event) => {
      const occurrence = nextOccurrence(event, now);
      if (!occurrence) return null;
      const { startsAt, endsAt, occurrenceId } = occurrence;
      if (!isPendingConfirmation(event) && startsAt <= now) return null;
      return {
        ...event,
        startsAt,
        endsAt,
        occurrenceId,
        displayStatus: displayStatusFor(event, startsAt, now),
        urgency: urgencyFor(event, startsAt, now),
      };
    })
    .filter((event): event is T & DisplayCalendarEvent => event !== null)
    .sort((a, b) => {
      if (a.urgency === 'overdue_pending' && b.urgency !== 'overdue_pending') return -1;
      if (a.urgency !== 'overdue_pending' && b.urgency === 'overdue_pending') return 1;
      return a.startsAt.getTime() - b.startsAt.getTime();
    });
}

export function collectDueCalendarReminders(
  events: CalendarPresentationEvent[],
  now = new Date(),
  remindedIds = new Set<string>(),
  windowMinutes = CALENDAR_REMINDER_WINDOW_MINUTES,
): CalendarReminder[] {
  return events
    .map((event) => {
      const occurrence = nextOccurrence(event, now);
      if (!occurrence || remindedIds.has(occurrence.occurrenceId)) return null;
      const { startsAt, occurrenceId } = occurrence;
      const minutesUntilStart = Math.ceil((startsAt.getTime() - now.getTime()) / 60_000);
      if (minutesUntilStart < 0 || minutesUntilStart > windowMinutes) return null;
      return {
        id: occurrenceId,
        title: event.title,
        minutesUntilStart,
        message: minutesUntilStart <= 0
          ? `现在开始：${event.title}`
          : `${minutesUntilStart} 分钟后：${event.title}`,
      };
    })
    .filter((reminder): reminder is CalendarReminder => reminder !== null)
    .sort((a, b) => a.minutesUntilStart - b.minutesUntilStart);
}
