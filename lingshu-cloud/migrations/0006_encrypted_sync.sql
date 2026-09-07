-- The server stores ciphertext only. Keys never leave the client.
CREATE TABLE sync_vaults (
  user_id TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
  key_id TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE TABLE sync_records (
  user_id TEXT NOT NULL REFERENCES sync_vaults(user_id) ON DELETE CASCADE,
  id TEXT NOT NULL,
  revision INTEGER NOT NULL CHECK (revision > 0),
  envelope TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (user_id, id)
);
CREATE TABLE sync_history (
  user_id TEXT NOT NULL REFERENCES sync_vaults(user_id) ON DELETE CASCADE,
  id TEXT NOT NULL,
  revision INTEGER NOT NULL,
  envelope TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (user_id, id, revision)
);
CREATE TRIGGER sync_preserve_history AFTER UPDATE ON sync_records BEGIN
  INSERT INTO sync_history VALUES (OLD.user_id, OLD.id, OLD.revision, OLD.envelope, OLD.updated_at);
  DELETE FROM sync_history WHERE user_id = NEW.user_id AND id = NEW.id AND revision < NEW.revision - 10;
END;
