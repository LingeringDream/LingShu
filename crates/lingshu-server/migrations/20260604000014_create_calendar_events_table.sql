-- Calendar events — local store until Apple Calendar / Swift sidecar is integrated in Phase 3.
-- Once the macOS bridge is built, events are synced to EventKit and linked via apple_event_id.
CREATE TABLE calendar_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,

    title TEXT NOT NULL,
    description TEXT,
    location TEXT,

    start_time TIMESTAMPTZ NOT NULL,
    end_time TIMESTAMPTZ NOT NULL,

    -- JSON array of attendee names/emails
    attendees JSONB NOT NULL DEFAULT '[]',

    -- NULL = local-only; set once synced to Apple Calendar
    apple_event_id TEXT,

    -- confirmed | pending_confirmation | cancelled
    status VARCHAR(20) NOT NULL DEFAULT 'pending_confirmation'
        CHECK (status IN ('pending_confirmation', 'confirmed', 'cancelled')),

    -- Which calendar this belongs to (e.g. "default", "work", "personal")
    calendar_name VARCHAR(50) NOT NULL DEFAULT 'default',

    -- LLM confidence in the parse (0-1)
    parse_confidence REAL,

    -- Raw user input that produced this event
    source_input TEXT,

    -- Conversation that spawned this event
    conversation_id UUID REFERENCES conversations(id) ON DELETE SET NULL,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_calendar_events_user ON calendar_events(user_id);
CREATE INDEX idx_calendar_events_time ON calendar_events(user_id, start_time);
CREATE INDEX idx_calendar_events_status ON calendar_events(user_id, status);
