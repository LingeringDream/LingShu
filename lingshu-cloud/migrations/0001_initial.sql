PRAGMA foreign_keys = ON;

CREATE TABLE pending_registrations (
    id TEXT PRIMARY KEY NOT NULL,
    email TEXT NOT NULL UNIQUE COLLATE NOCASE,
    display_name TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    verification_token_hash TEXT NOT NULL UNIQUE,
    token_expires_at TEXT NOT NULL,
    consumed_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX idx_pending_registrations_token
    ON pending_registrations (verification_token_hash, token_expires_at);

CREATE TABLE users (
    id TEXT PRIMARY KEY NOT NULL,
    email TEXT NOT NULL UNIQUE COLLATE NOCASE,
    display_name TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    email_verified_at TEXT NOT NULL,
    last_login_at TEXT,
    deleted_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX idx_users_email_active
    ON users (email)
    WHERE deleted_at IS NULL;

CREATE TABLE devices (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    platform TEXT NOT NULL,
    created_at TEXT NOT NULL,
    last_seen_at TEXT NOT NULL,
    revoked_at TEXT
);

CREATE INDEX idx_devices_user_active
    ON devices (user_id, last_seen_at)
    WHERE revoked_at IS NULL;

CREATE TABLE sessions (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    device_id TEXT NOT NULL REFERENCES devices (id) ON DELETE CASCADE,
    family_id TEXT NOT NULL,
    refresh_token_hash TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL,
    last_used_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    revoked_at TEXT,
    replaced_by TEXT REFERENCES sessions (id)
);

CREATE INDEX idx_sessions_user_active
    ON sessions (user_id, expires_at)
    WHERE revoked_at IS NULL;

CREATE INDEX idx_sessions_device_active
    ON sessions (device_id, expires_at)
    WHERE revoked_at IS NULL;

CREATE INDEX idx_sessions_family
    ON sessions (family_id);

CREATE TABLE login_attempts (
    key_type TEXT NOT NULL,
    key_hash TEXT NOT NULL,
    window_started_at TEXT NOT NULL,
    attempts INTEGER NOT NULL DEFAULT 0,
    blocked_until TEXT,
    PRIMARY KEY (key_type, key_hash)
);
