PRAGMA foreign_keys = ON;

CREATE TABLE user_sync_state (
    user_id TEXT PRIMARY KEY NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    revision INTEGER NOT NULL DEFAULT 0,
    object_key TEXT NOT NULL,
    checksum TEXT NOT NULL,
    byte_size INTEGER NOT NULL,
    item_count INTEGER NOT NULL DEFAULT 0,
    key_version INTEGER NOT NULL DEFAULT 1,
    algorithm TEXT NOT NULL CHECK (algorithm = 'AES-256-GCM'),
    updated_by_device_id TEXT NOT NULL REFERENCES devices (id),
    updated_at TEXT NOT NULL
);

CREATE TABLE user_sync_history (
    user_id TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    revision INTEGER NOT NULL,
    object_key TEXT NOT NULL,
    checksum TEXT NOT NULL,
    byte_size INTEGER NOT NULL,
    item_count INTEGER NOT NULL DEFAULT 0,
    key_version INTEGER NOT NULL DEFAULT 1,
    algorithm TEXT NOT NULL CHECK (algorithm = 'AES-256-GCM'),
    device_id TEXT NOT NULL REFERENCES devices (id),
    created_at TEXT NOT NULL,
    PRIMARY KEY (user_id, revision)
);

CREATE INDEX idx_user_sync_history_recent
    ON user_sync_history (user_id, revision DESC);
