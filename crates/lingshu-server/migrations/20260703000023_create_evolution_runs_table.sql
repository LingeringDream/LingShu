-- Append-only ledger for self-evolution decisions.
-- Personality snapshots remain the source of active trait values; this table
-- records why an evolution run applied, skipped, failed, or required review.

CREATE TABLE evolution_runs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    scope VARCHAR(40) NOT NULL
        CHECK (scope IN ('personality')),
    status VARCHAR(20) NOT NULL
        CHECK (status IN ('applied', 'skipped', 'failed', 'review_required')),
    evidence_summary JSONB NOT NULL DEFAULT '{}',
    proposed_changes JSONB NOT NULL DEFAULT '[]',
    applied_changes JSONB NOT NULL DEFAULT '[]',
    skipped_reason TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_evolution_runs_user_scope_created
    ON evolution_runs(user_id, scope, created_at DESC);

CREATE INDEX idx_evolution_runs_status
    ON evolution_runs(user_id, status, created_at DESC);
