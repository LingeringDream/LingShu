-- Pending derivations are retained for review, never silently reactivated by sync.
ALTER TABLE memories ADD COLUMN derivation_stale boolean NOT NULL DEFAULT false;

CREATE FUNCTION invalidate_memory_descendants(source_id uuid) RETURNS void
LANGUAGE sql AS $$
    WITH RECURSIVE descendants(id) AS (
        SELECT id FROM memories WHERE source_memory_ids @> ARRAY[source_id]
        UNION
        SELECT m.id FROM memories m JOIN descendants d
          ON m.source_memory_ids @> ARRAY[d.id]
    )
    UPDATE memories SET derivation_stale=true,
        metadata=metadata || '{"derivation_review_required":true}'::jsonb,
        updated_at=now()
    WHERE id IN (SELECT id FROM descendants) AND tier='derived' AND NOT derivation_stale;
$$;

CREATE FUNCTION memory_source_changed() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    PERFORM invalidate_memory_descendants(OLD.id);
    RETURN NULL;
END
$$;
CREATE TRIGGER memory_source_deleted AFTER DELETE ON memories
FOR EACH ROW EXECUTE FUNCTION memory_source_changed();
CREATE TRIGGER memory_source_updated AFTER UPDATE ON memories
FOR EACH ROW WHEN (
    ROW(OLD.content,OLD.deleted_at,OLD.user_id,OLD.owner_agent_id,OLD.memory_scope,
        OLD.context_scope,OLD.conflict_status,OLD.retention_status,OLD.valid_from,
        OLD.valid_to,OLD.fact_key,OLD.fact_value,OLD.source_trust,OLD.tier,OLD.source_memory_ids)
    IS DISTINCT FROM
    ROW(NEW.content,NEW.deleted_at,NEW.user_id,NEW.owner_agent_id,NEW.memory_scope,
        NEW.context_scope,NEW.conflict_status,NEW.retention_status,NEW.valid_from,
        NEW.valid_to,NEW.fact_key,NEW.fact_value,NEW.source_trust,NEW.tier,NEW.source_memory_ids))
EXECUTE FUNCTION memory_source_changed();

CREATE FUNCTION memory_grant_changed() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP <> 'INSERT' THEN
        PERFORM invalidate_memory_descendants(OLD.memory_id);
    END IF;
    IF TG_OP <> 'DELETE' THEN
        PERFORM invalidate_memory_descendants(NEW.memory_id);
    END IF;
    RETURN NULL;
END
$$;
CREATE TRIGGER memory_grant_derivation AFTER INSERT OR UPDATE OR DELETE ON memory_access_grants
FOR EACH ROW EXECUTE FUNCTION memory_grant_changed();

CREATE FUNCTION guard_memory_derivation() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    source memories%ROWTYPE;
    found_count integer := 0;
BEGIN
    NEW.derivation_stale := NEW.derivation_stale
        OR NEW.metadata @> '{"derivation_review_required":true}'::jsonb;
    IF TG_OP='UPDATE' THEN
        -- No existing writer is a provenance review endpoint. A replacement
        -- must be a new derivation built against current source snapshots.
        NEW.derivation_stale := OLD.derivation_stale OR NEW.derivation_stale;
        IF NEW.tier='derived' AND ROW(OLD.tier,OLD.source_memory_ids,OLD.content)
            IS DISTINCT FROM ROW(NEW.tier,NEW.source_memory_ids,NEW.content) THEN
            NEW.derivation_stale := true;
        END IF;
    ELSIF NEW.tier='derived' THEN
        -- Stable locks serialize creation with source edits/deletes.
        FOR source IN SELECT * FROM memories WHERE id=ANY(NEW.source_memory_ids)
            ORDER BY id FOR SHARE LOOP
            found_count := found_count+1;
            IF source.user_id<>NEW.user_id OR source.deleted_at IS NOT NULL
                OR source.derivation_stale OR source.conflict_status<>'active'
                OR source.retention_status<>'active' OR source.valid_from>now()
                OR (source.valid_to IS NOT NULL AND source.valid_to<=now())
                OR source.owner_agent_id<>NEW.owner_agent_id
                OR source.memory_scope<>NEW.memory_scope
                OR source.context_scope<>NEW.context_scope
                OR source.memory_scope='shared' THEN
                NEW.derivation_stale := true;
            END IF;
            NEW.valid_from := greatest(NEW.valid_from, source.valid_from);
            NEW.valid_to := least(NEW.valid_to, source.valid_to);
            NEW.source_trust := least(NEW.source_trust, source.source_trust);
        END LOOP;
        IF found_count=0 OR found_count<>cardinality(NEW.source_memory_ids) THEN
            NEW.derivation_stale := true;
        END IF;
    END IF;
    IF NEW.derivation_stale THEN
        NEW.metadata := NEW.metadata || '{"derivation_review_required":true}'::jsonb;
    END IF;
    RETURN NEW;
END
$$;
CREATE TRIGGER memory_derivation_guard BEFORE INSERT OR UPDATE ON memories
FOR EACH ROW EXECUTE FUNCTION guard_memory_derivation();

-- Old summaries have no source revision snapshots: their freshness cannot be
-- proved retrospectively. Preserve them but require replacement/review.
UPDATE memories SET derivation_stale=true,
    metadata=metadata || '{"derivation_review_required":true}'::jsonb, updated_at=now()
WHERE tier='derived';
