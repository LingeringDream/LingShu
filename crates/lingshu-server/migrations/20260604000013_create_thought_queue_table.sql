-- SoulLedger L7: Thought Queue — proactive suggestions the assistant forms.
-- Each thought must include reason, confidence, source memories, and confirmation flag.
CREATE TABLE thought_queue (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,

    title TEXT NOT NULL,
    detail TEXT,

    -- Why this thought was generated
    reason TEXT,

    -- 0.0-1.0 confidence score
    confidence REAL NOT NULL DEFAULT 0.5
        CHECK (confidence >= 0 AND confidence <= 1),

    -- Memories that support this suggestion
    source_memory_ids UUID[] NOT NULL DEFAULT '{}',

    -- Whether user confirmation is required before acting
    requires_confirmation BOOLEAN NOT NULL DEFAULT true,

    -- Lifecycle: pending → shown → confirmed/dismissed; or auto-expired
    status VARCHAR(20) NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'shown', 'confirmed', 'dismissed', 'expired')),

    -- When to surface this thought (NULL = immediate)
    scheduled_at TIMESTAMPTZ,

    -- When the user acted on it
    resolved_at TIMESTAMPTZ,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_thought_queue_user ON thought_queue(user_id);
CREATE INDEX idx_thought_queue_status ON thought_queue(user_id, status);
CREATE INDEX idx_thought_queue_confidence ON thought_queue(confidence DESC) WHERE status = 'pending';
