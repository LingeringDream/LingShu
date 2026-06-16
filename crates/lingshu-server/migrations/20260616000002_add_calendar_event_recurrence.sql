-- Store recurrence metadata for local calendar events.
-- The default keeps existing rows and older clients as one-time events.
ALTER TABLE calendar_events
    ADD COLUMN recurrence JSONB NOT NULL DEFAULT '{"frequency":"none","interval":1}';

COMMENT ON COLUMN calendar_events.recurrence IS
    'Local recurrence rule: frequency none/daily/weekly/monthly/yearly, interval, optional until/count';
