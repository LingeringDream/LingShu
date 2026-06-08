-- Add role_prompt column to users for custom AI persona/role-play configuration.
ALTER TABLE users ADD COLUMN role_prompt TEXT NOT NULL DEFAULT '';
