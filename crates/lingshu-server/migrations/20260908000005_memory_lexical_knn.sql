-- pg_trgm is already enabled by the initial extension migration. GiST supports
-- distance-ordered top-k retrieval; GIN cannot avoid sorting every common hit.
-- Keep the existing GIN terms index for write-time duplicate candidate lookup.
-- On existing installations index creation runs during migration and can block
-- writes until complete; schedule upgrades of large databases accordingly.
CREATE INDEX memories_lexical_knn_idx ON memories
USING gist (lower(content) gist_trgm_ops(siglen=64))
WHERE deleted_at IS NULL AND NOT derivation_stale;
