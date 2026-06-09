-- Persist per-user LLM settings (provider, model, api key, base url, params)
-- so they survive backend restarts. Previously these lived only in memory plus
-- an optional Redis cache, so every restart (or any run without Redis) dropped
-- the user's model configuration and forced them to re-enter it.
--
-- Stored as JSONB matching the `LlmSettings` struct. Defaults to '{}', which is
-- intentionally NOT a complete LlmSettings — the loader treats a value that
-- fails to deserialize as "not configured yet" and falls back to config defaults.
ALTER TABLE users ADD COLUMN llm_settings JSONB NOT NULL DEFAULT '{}'::jsonb;
