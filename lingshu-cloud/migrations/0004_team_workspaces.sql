PRAGMA foreign_keys = ON;

CREATE TABLE team_workspaces (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE team_members (
    team_id TEXT NOT NULL REFERENCES team_workspaces (id) ON DELETE CASCADE,
    cloud_user_id TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    email TEXT NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('owner', 'admin', 'editor', 'viewer')),
    created_at TEXT NOT NULL,
    PRIMARY KEY (team_id, cloud_user_id)
);

CREATE INDEX idx_team_members_user ON team_members (cloud_user_id, team_id);

CREATE TABLE team_invites (
    id TEXT PRIMARY KEY NOT NULL,
    team_id TEXT NOT NULL REFERENCES team_workspaces (id) ON DELETE CASCADE,
    email TEXT NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('admin', 'editor', 'viewer')),
    token_hash TEXT NOT NULL UNIQUE,
    expires_at TEXT NOT NULL,
    used_at TEXT,
    revoked_at TEXT,
    created_at TEXT NOT NULL
);

CREATE INDEX idx_team_invites_token ON team_invites (token_hash, expires_at);
CREATE INDEX idx_team_invites_team ON team_invites (team_id, created_at);

CREATE TABLE team_agent_keys (
    id TEXT PRIMARY KEY NOT NULL,
    team_id TEXT NOT NULL REFERENCES team_workspaces (id) ON DELETE CASCADE,
    agent_key TEXT NOT NULL,
    name TEXT NOT NULL,
    token_hash TEXT NOT NULL UNIQUE,
    permission TEXT NOT NULL CHECK (permission IN ('read', 'write')),
    expires_at TEXT NOT NULL,
    revoked_at TEXT,
    created_at TEXT NOT NULL
);

CREATE INDEX idx_team_agent_keys_team ON team_agent_keys (team_id, created_at);

CREATE TABLE team_audit_events (
    id TEXT PRIMARY KEY NOT NULL,
    team_id TEXT NOT NULL REFERENCES team_workspaces (id) ON DELETE CASCADE,
    actor TEXT NOT NULL,
    action TEXT NOT NULL,
    target TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX idx_team_audit_team ON team_audit_events (team_id, created_at);

CREATE TABLE team_memories (
    id TEXT PRIMARY KEY NOT NULL,
    team_id TEXT NOT NULL REFERENCES team_workspaces (id) ON DELETE CASCADE,
    content TEXT NOT NULL,
    memory_layer TEXT NOT NULL,
    memory_scope TEXT NOT NULL CHECK (memory_scope IN ('private', 'shared', 'global')),
    owner_agent_id TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX idx_team_memories_team ON team_memories (team_id, created_at);

CREATE TABLE team_memory_grants (
    memory_id TEXT NOT NULL REFERENCES team_memories (id) ON DELETE CASCADE,
    grantee_agent_id TEXT NOT NULL,
    can_read INTEGER NOT NULL DEFAULT 1 CHECK (can_read IN (0, 1)),
    PRIMARY KEY (memory_id, grantee_agent_id)
);
