-- SoulLedger: memory tiering and provenance tracking.
--
-- tier = 'raw'     → original episodic memory (default for all existing rows)
-- tier = 'derived' → synthesized by the offline consolidation engine from
--                     multiple raw source memories
--
-- source_memory_ids links derived memories back to their raw sources for
-- provenance auditing and forgetting-sweep protection.

ALTER TABLE memories
    ADD COLUMN source_memory_ids UUID[] NOT NULL DEFAULT '{}',
    ADD COLUMN tier VARCHAR(16) NOT NULL DEFAULT 'raw',
    ADD CONSTRAINT memories_tier_check CHECK (tier IN ('raw', 'derived'));

-- GIN index for efficient reverse lookups: "which derived memories reference
-- this raw source?"
CREATE INDEX idx_memories_source_ids ON memories USING GIN(source_memory_ids);
