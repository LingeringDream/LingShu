// ── EventKit Frontend Bridge (Phase B) ─────────────────────────────────
//
// Invokes Tauri EventKit commands from the frontend.
// Gracefully degrades to no-ops when running in a browser (non-Tauri).
//
// Flow for creating an Apple Calendar event:
//   1. User confirms event: POST /api/v1/calendar/events/{id}/confirm
//   2. invoke create_calendar_event  →  get appleEventId
//   3. POST /api/v1/calendar/events/{id}/apple-event  →  store appleEventId

import { isTauri, invokeTauri } from './tauri';
import { apiFetch } from './api';

/** Authorization status returned by EventKit. */
export type CalendarAccessStatus =
  | 'full_access'
  | 'denied'
  | 'restricted'
  | 'write_only'
  | 'not_determined'
  | 'unsupported';

/** Request calendar permissions from the user. */
export async function requestCalendarAccess(): Promise<CalendarAccessStatus> {
  if (!isTauri()) return 'unsupported';
  const result = await invokeTauri<string>('request_calendar_access');
  return (result as CalendarAccessStatus) ?? 'unsupported';
}

/** Event fields needed for EventKit creation. */
export interface AppleEventInput {
  title: string;
  start_time: string; // ISO 8601
  end_time: string;   // ISO 8601
  location?: string;
  notes?: string;
  calendar_name?: string;
}

/**
 * Create an event in Apple Calendar and store the resulting
 * eventIdentifier back to the backend.
 *
 * Returns the EventKit eventIdentifier on success.
 */
export async function syncEventToAppleCalendar(
  backendEventId: string,
  event: AppleEventInput
): Promise<string | null> {
  if (!isTauri()) {
    console.log('[eventkit] Running in browser — skipping Apple Calendar sync');
    return null;
  }

  try {
    const access = await requestCalendarAccess();
    if (access === 'denied' || access === 'restricted' || access === 'unsupported') {
      console.error(`[eventkit] Calendar access unavailable: ${access}`);
      return null;
    }

    // 1. Create in Apple Calendar via EventKit
    const appleEventId = await invokeTauri<string>('create_calendar_event', {
      event,
    });

    if (!appleEventId) {
      console.error('[eventkit] create_calendar_event returned null');
      return null;
    }

    console.log(`[eventkit] Created Apple Calendar event: ${appleEventId}`);

    // 2. Store apple_event_id back to backend
    const resp = await apiFetch(
      `/api/v1/calendar/events/${backendEventId}/apple-event`,
      {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ apple_event_id: appleEventId }),
      }
    );

    if (!resp.ok) {
      console.error(
        `[eventkit] Failed to save apple_event_id: HTTP ${resp.status}`
      );
      return appleEventId; // Event was created, but backend sync failed
    }

    console.log(`[eventkit] Stored apple_event_id for event ${backendEventId}`);
    return appleEventId;
  } catch (err) {
    console.error('[eventkit] syncEventToAppleCalendar failed:', err);
    return null;
  }
}

/**
 * Update an existing Apple Calendar event.
 */
export async function updateAppleCalendarEvent(
  eventIdentifier: string,
  updates: {
    title?: string;
    start_time?: string;
    end_time?: string;
    location?: string;
    notes?: string;
  }
): Promise<boolean> {
  if (!isTauri()) return false;

  try {
    await invokeTauri('update_calendar_event', {
      updates: { event_identifier: eventIdentifier, ...updates },
    });
    return true;
  } catch (err) {
    console.error('[eventkit] updateAppleCalendarEvent failed:', err);
    return false;
  }
}

/**
 * Delete an Apple Calendar event.
 */
export async function deleteAppleCalendarEvent(
  eventIdentifier: string
): Promise<boolean> {
  if (!isTauri()) return false;

  try {
    await invokeTauri('delete_calendar_event', {
      eventIdentifier,
    });
    return true;
  } catch (err) {
    console.error('[eventkit] deleteAppleCalendarEvent failed:', err);
    return false;
  }
}
