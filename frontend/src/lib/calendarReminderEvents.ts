import { isTauri } from './tauri';

export const CALENDAR_REMINDER_EVENT = 'lingshu:calendar-reminder';

export interface CalendarReminderPayload {
  id: string;
  title: string;
  message: string;
  minutesUntilStart: number;
}

export async function publishCalendarReminder(payload: CalendarReminderPayload): Promise<void> {
  window.dispatchEvent(new CustomEvent(CALENDAR_REMINDER_EVENT, { detail: payload }));

  if (!isTauri()) return;
  try {
    const { emitTo } = await import('@tauri-apps/api/event');
    await emitTo('pet', CALENDAR_REMINDER_EVENT, payload);
  } catch (error) {
    console.error('[calendar] failed to emit reminder to pet window:', error);
  }
}

export function subscribeToCalendarReminders(
  handler: (payload: CalendarReminderPayload) => void,
): () => void {
  let disposed = false;
  let unlistenTauri: (() => void) | null = null;

  const handleCustomEvent = (event: Event) => {
    handler((event as CustomEvent<CalendarReminderPayload>).detail);
  };

  window.addEventListener(CALENDAR_REMINDER_EVENT, handleCustomEvent);

  if (isTauri()) {
    import('@tauri-apps/api/event')
      .then(({ listen }) => listen<CalendarReminderPayload>(CALENDAR_REMINDER_EVENT, (event) => {
        handler(event.payload);
      }))
      .then((unlisten) => {
        if (disposed) {
          unlisten();
        } else {
          unlistenTauri = unlisten;
        }
      })
      .catch((error) => {
        console.error('[calendar] failed to listen for reminders:', error);
      });
  }

  return () => {
    disposed = true;
    window.removeEventListener(CALENDAR_REMINDER_EVENT, handleCustomEvent);
    if (unlistenTauri) unlistenTauri();
  };
}
