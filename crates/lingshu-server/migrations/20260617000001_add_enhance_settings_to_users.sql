-- Persist per-user multi-model enhancement (Mixture-of-Agents) settings.
-- When enabled, a chat turn is answered by several member models in parallel;
-- their drafts are synthesised by a single aggregator model into one reply.
-- Stored as JSONB matching the EnhanceSettings struct; '{}' = disabled.
ALTER TABLE users ADD COLUMN enhance_settings JSONB NOT NULL DEFAULT '{}'::jsonb;
