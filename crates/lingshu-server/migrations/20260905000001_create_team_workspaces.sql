-- Team data uses a dedicated, non-login user namespace so the existing memory
-- engine retains its SQL/vector isolation. No member receives that user's JWT.
CREATE TABLE team_workspaces (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(100) NOT NULL,
    memory_user_id UUID NOT NULL UNIQUE REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE TABLE team_members (
    team_id UUID NOT NULL REFERENCES team_workspaces(id) ON DELETE CASCADE,
    cloud_user_id UUID NOT NULL,
    email VARCHAR(255) NOT NULL,
    role VARCHAR(16) NOT NULL CHECK (role IN ('owner','admin','editor','viewer')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY(team_id, cloud_user_id)
);
CREATE TABLE team_invites (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    team_id UUID NOT NULL REFERENCES team_workspaces(id) ON DELETE CASCADE,
    email VARCHAR(255) NOT NULL,
    role VARCHAR(16) NOT NULL CHECK (role IN ('admin','editor','viewer')),
    token_hash VARCHAR(64) NOT NULL UNIQUE,
    expires_at TIMESTAMPTZ NOT NULL DEFAULT NOW() + INTERVAL '7 days',
    used_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE TABLE team_agent_keys (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    team_id UUID NOT NULL REFERENCES team_workspaces(id) ON DELETE CASCADE,
    agent_key VARCHAR(64) NOT NULL,
    name VARCHAR(100) NOT NULL,
    token_hash VARCHAR(64) NOT NULL UNIQUE,
    permission VARCHAR(16) NOT NULL CHECK (permission IN ('read','write')),
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE TABLE team_audit_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    team_id UUID NOT NULL REFERENCES team_workspaces(id) ON DELETE CASCADE,
    actor TEXT NOT NULL,
    action VARCHAR(64) NOT NULL,
    target TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX team_members_identity ON team_members(cloud_user_id);
CREATE INDEX team_audit_timeline ON team_audit_events(team_id,created_at DESC);
