-- Persist L0–L4 permission tiers so they survive server restarts.
ALTER TABLE users ADD COLUMN IF NOT EXISTS permissions JSONB;
