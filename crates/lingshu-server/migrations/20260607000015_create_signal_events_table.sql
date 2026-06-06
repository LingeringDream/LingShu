-- Append-only signal events log for SoulLedger calibration.
-- Every user-facing interaction worth measuring records one row here.
-- The telemetry module guarantees fire-and-forget semantics:
-- insert failures are logged but never propagated to callers.
--
-- Event types are enforced by the telemetry::SignalEventType enum
-- (the DB column is a plain VARCHAR — the enum is the Rust-side whitelist).

CREATE TABLE signal_events (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    event_type VARCHAR(50) NOT NULL,
    entity_type VARCHAR(30),       -- e.g. 'memory', 'thought', 'reply', 'personality'
    entity_id  UUID,               -- associated entity (nullable)
    metadata   JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_signal_events_user_type
    ON signal_events(user_id, event_type, created_at DESC);

CREATE INDEX idx_signal_events_entity
    ON signal_events(entity_type, entity_id)
    WHERE entity_id IS NOT NULL;
