-- Evolutionary memory protocol: three-layer storage, multi-agent gateway,
-- temporal conflict governance and explainable adaptive retrieval.

ALTER TABLE memories
    ADD COLUMN memory_layer VARCHAR(24) NOT NULL DEFAULT 'episode',
    ADD COLUMN owner_agent_id VARCHAR(64) NOT NULL DEFAULT 'lingshu',
    ADD COLUMN memory_scope VARCHAR(16) NOT NULL DEFAULT 'private',
    ADD COLUMN scene_tags TEXT[] NOT NULL DEFAULT '{}',
    ADD COLUMN episode_id UUID,
    ADD COLUMN occurred_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    ADD COLUMN valid_from TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    ADD COLUMN valid_to TIMESTAMPTZ,
    ADD COLUMN source_type VARCHAR(32) NOT NULL DEFAULT 'conversation',
    ADD COLUMN source_trust REAL NOT NULL DEFAULT 0.7,
    ADD COLUMN fact_key VARCHAR(255),
    ADD COLUMN fact_value TEXT,
    ADD COLUMN conflict_status VARCHAR(16) NOT NULL DEFAULT 'active',
    ADD COLUMN superseded_by UUID REFERENCES memories(id) ON DELETE SET NULL,
    ADD COLUMN retention_status VARCHAR(16) NOT NULL DEFAULT 'active',
    ADD CONSTRAINT memories_layer_check
        CHECK (memory_layer IN ('episode', 'fact', 'experience')),
    ADD CONSTRAINT memories_scope_check
        CHECK (memory_scope IN ('private', 'shared', 'global')),
    ADD CONSTRAINT memories_source_trust_check
        CHECK (source_trust >= 0 AND source_trust <= 1),
    ADD CONSTRAINT memories_conflict_status_check
        CHECK (conflict_status IN ('active', 'superseded', 'disputed')),
    ADD CONSTRAINT memories_retention_status_check
        CHECK (retention_status IN ('active', 'stale', 'filtered'));

-- Existing consolidated memories are reusable experiences; original rows keep
-- their historical episodic meaning. New extraction writes explicitly choose
-- the structured fact layer.
UPDATE memories SET memory_layer = 'experience' WHERE tier = 'derived';

CREATE INDEX idx_memories_gateway
    ON memories(user_id, owner_agent_id, memory_scope, memory_layer)
    WHERE deleted_at IS NULL;
CREATE INDEX idx_memories_temporal
    ON memories(user_id, occurred_at DESC, valid_from DESC)
    WHERE deleted_at IS NULL;
CREATE INDEX idx_memories_fact_key
    ON memories(user_id, fact_key, valid_from DESC)
    WHERE deleted_at IS NULL AND fact_key IS NOT NULL;
CREATE INDEX idx_memories_scene_tags ON memories USING GIN(scene_tags);

CREATE TABLE memory_agents (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    agent_key VARCHAR(64) NOT NULL,
    display_name VARCHAR(120) NOT NULL,
    role_description TEXT NOT NULL DEFAULT '',
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(user_id, agent_key)
);

CREATE TABLE memory_access_grants (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    memory_id UUID NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
    grantee_agent_id VARCHAR(64) NOT NULL,
    can_read BOOLEAN NOT NULL DEFAULT TRUE,
    can_write BOOLEAN NOT NULL DEFAULT FALSE,
    can_share BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(memory_id, grantee_agent_id)
);
CREATE INDEX idx_memory_grants_agent
    ON memory_access_grants(grantee_agent_id, memory_id) WHERE can_read = TRUE;

CREATE TABLE memory_conflict_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    fact_key VARCHAR(255) NOT NULL,
    incumbent_memory_id UUID NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
    challenger_memory_id UUID NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
    resolution VARCHAR(24) NOT NULL,
    reason TEXT NOT NULL,
    score_delta REAL NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT memory_conflict_resolution_check
        CHECK (resolution IN ('challenger_wins', 'incumbent_wins', 'disputed'))
);
CREATE INDEX idx_memory_conflicts_user
    ON memory_conflict_events(user_id, created_at DESC);

CREATE TABLE memory_retrieval_runs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    agent_id VARCHAR(64) NOT NULL,
    query TEXT NOT NULL,
    mode VARCHAR(16) NOT NULL,
    requested_k INT NOT NULL,
    returned_count INT NOT NULL,
    filtered_count INT NOT NULL DEFAULT 0,
    latency_ms REAL NOT NULL,
    estimated_tokens INT NOT NULL DEFAULT 0,
    weights JSONB NOT NULL DEFAULT '{}',
    result_ids UUID[] NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT memory_retrieval_mode_check CHECK (mode IN ('static', 'adaptive'))
);
CREATE INDEX idx_memory_retrieval_runs_user
    ON memory_retrieval_runs(user_id, created_at DESC);
