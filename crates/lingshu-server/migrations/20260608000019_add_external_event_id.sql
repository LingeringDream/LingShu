-- Add external_event_id (EventKit eventIdentifier) and synced_to_eventkit
-- flag so the frontend can indicate a confirmed event was written to the
-- system calendar via the Tauri EventKit bridge.
--
-- The existing apple_event_id column is kept for backwards compatibility
-- with events synced before this migration.

ALTER TABLE calendar_events ADD COLUMN external_event_id TEXT;
ALTER TABLE calendar_events ADD COLUMN synced_to_eventkit BOOLEAN NOT NULL DEFAULT false;

COMMENT ON COLUMN calendar_events.external_event_id IS 'EventKit eventIdentifier from create_calendar_event Tauri command';
COMMENT ON COLUMN calendar_events.synced_to_eventkit IS 'true when the event was successfully written to the system calendar';
