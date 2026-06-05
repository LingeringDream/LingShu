CREATE TABLE memories (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    project_id UUID REFERENCES projects(id) ON DELETE SET NULL,
    memory_type VARCHAR(30) NOT NULL,
    content TEXT NOT NULL,
    importance REAL NOT NULL DEFAULT 0.5,
    access_count INT NOT NULL DEFAULT 0,
    last_accessed_at TIMESTAMPTZ,
    vector_id VARCHAR(255),
    metadata JSONB NOT NULL DEFAULT '{}',
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_memories_user ON memories(user_id) WHERE deleted_at IS NULL;
CREATE INDEX idx_memories_project ON memories(project_id) WHERE deleted_at IS NULL;
CREATE INDEX idx_memories_type ON memories(memory_type) WHERE deleted_at IS NULL;
CREATE INDEX idx_memories_importance ON memories(importance DESC) WHERE deleted_at IS NULL;
