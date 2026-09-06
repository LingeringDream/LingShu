PRAGMA foreign_keys = ON;

ALTER TABLE team_invites ADD COLUMN notified_at TEXT;

CREATE INDEX IF NOT EXISTS idx_team_invites_pending
    ON team_invites (email, used_at, revoked_at, expires_at);