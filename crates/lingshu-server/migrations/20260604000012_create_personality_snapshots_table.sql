-- SoulLedger L5: Personality snapshots track evolution of the 7 trait parameters.
-- Each snapshot records what changed, why, and which memories influenced the change.
CREATE TABLE personality_snapshots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,

    -- The 7 personality traits stored as JSON for flexibility
    -- { directness, warmth, proactivity, risk_tolerance, verbosity, formality, humor }
    -- Each value is 0.0-1.0 with Identity Core defaults at 0.5
    trait_values JSONB NOT NULL,

    -- What caused this snapshot
    change_reason TEXT,
    -- e.g. "auto-evolution", "manual-edit", "rollback", "identity-core-reset"

    -- Memories that influenced this personality change
    source_memory_ids UUID[] NOT NULL DEFAULT '{}',

    -- Only one snapshot is active at a time per user
    is_active BOOLEAN NOT NULL DEFAULT false,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_personality_snapshots_user ON personality_snapshots(user_id);
CREATE INDEX idx_personality_snapshots_active ON personality_snapshots(user_id) WHERE is_active = true;
