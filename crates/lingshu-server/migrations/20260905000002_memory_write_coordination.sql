-- Business scope is independent of the visibility/ACL scope.
ALTER TABLE memories ADD COLUMN context_scope TEXT NOT NULL DEFAULT 'default';
CREATE INDEX idx_memories_context_fact
    ON memories(user_id, context_scope, fact_key) WHERE deleted_at IS NULL;

-- Persist the original request so retries cannot silently change its meaning.
CREATE TABLE memory_write_requests (
    user_id UUID NOT NULL REFERENCES users(id),
    agent_id TEXT NOT NULL,
    request_id TEXT NOT NULL,
    payload JSONB NOT NULL,
    memory_id UUID NOT NULL REFERENCES memories(id),
    PRIMARY KEY(user_id, agent_id, request_id)
);
