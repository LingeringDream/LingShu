import type { CalendarRecurrenceRule } from './calendarPresentation';
import { getPlatformCapabilities, invokeTauri } from './tauri';

export interface CalendarExportEvent {
  id: string;
  title: string;
  start_time: string;
  end_time: string;
  location?: string | null;
  description?: string | null;
  recurrence?: CalendarRecurrenceRule | null;
}

export interface CalendarExportResult {
  method: 'browser' | 'native' | 'unsupported';
  path?: string;
}

function escapeIcsText(value: string): string {
  return value
    .replace(/\\/g, '\\\\')
    .replace(/\r?\n/g, '\\n')
    .replace(/;/g, '\\;')
    .replace(/,/g, '\\,');
}

function toIcsUtc(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) throw new Error(`无效的日历时间：${iso}`);
  return date.toISOString().replace(/[-:]/g, '').replace(/\.\d{3}Z$/, 'Z');
}

function recurrenceLine(recurrence?: CalendarRecurrenceRule | null): string | null {
  const frequency = recurrence?.frequency;
  if (!frequency || frequency === 'none') return null;
  const parts = [
    `FREQ=${frequency.toUpperCase()}`,
    `INTERVAL=${Math.max(1, Math.floor(recurrence.interval || 1))}`,
  ];
  if (recurrence.until) parts.push(`UNTIL=${toIcsUtc(recurrence.until)}`);
  else if (recurrence.count && recurrence.count > 0) parts.push(`COUNT=${Math.floor(recurrence.count)}`);
  return `RRULE:${parts.join(';')}`;
}

export function buildCalendarIcs(event: CalendarExportEvent, now = new Date()): string {
  if (Number.isNaN(now.getTime())) throw new Error('无效的导出时间');
  const lines = [
    'BEGIN:VCALENDAR',
    'VERSION:2.0',
    'PRODID:-//LingShu//Cross-platform Calendar//ZH-CN',
    'CALSCALE:GREGORIAN',
    'METHOD:PUBLISH',
    'BEGIN:VEVENT',
    `UID:${escapeIcsText(event.id)}@lingshu.local`,
    `DTSTAMP:${toIcsUtc(now.toISOString())}`,
    `DTSTART:${toIcsUtc(event.start_time)}`,
    `DTEND:${toIcsUtc(event.end_time)}`,
    `SUMMARY:${escapeIcsText(event.title)}`,
  ];
  if (event.location) lines.push(`LOCATION:${escapeIcsText(event.location)}`);
  if (event.description) lines.push(`DESCRIPTION:${escapeIcsText(event.description)}`);
  const recurrence = recurrenceLine(event.recurrence);
  if (recurrence) lines.push(recurrence);
  lines.push('END:VEVENT', 'END:VCALENDAR');
  return `${lines.join('\r\n')}\r\n`;
}

function calendarFileName(event: CalendarExportEvent): string {
  const date = new Date(event.start_time);
  const datePart = Number.isNaN(date.getTime())
    ? 'calendar'
    : date.toISOString().slice(0, 10);
  const title = event.title
    .trim()
    .replace(/[^\p{L}\p{N}\-_ ]/gu, '_')
    .replace(/\s+/g, '-')
    .slice(0, 48) || 'lingshu';
  return `${datePart}-${title}.ics`;
}

function downloadInBrowser(fileName: string, content: string): void {
  const blob = new Blob([content], { type: 'text/calendar;charset=utf-8' });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement('a');
  anchor.href = url;
  anchor.download = fileName;
  anchor.style.display = 'none';
  document.body.appendChild(anchor);
  anchor.click();
  anchor.remove();
  window.setTimeout(() => URL.revokeObjectURL(url), 0);
}

export async function exportCalendarEvent(event: CalendarExportEvent): Promise<CalendarExportResult> {
  const capabilities = await getPlatformCapabilities();
  if (capabilities.calendarProvider !== 'ics') return { method: 'unsupported' };

  const fileName = calendarFileName(event);
  const content = buildCalendarIcs(event);
  if (capabilities.isDesktop) {
    const path = await invokeTauri<string>('export_calendar_file', { fileName, content });
    return path ? { method: 'native', path } : { method: 'unsupported' };
  }

  downloadInBrowser(fileName, content);
  return { method: 'browser', path: fileName };
}
