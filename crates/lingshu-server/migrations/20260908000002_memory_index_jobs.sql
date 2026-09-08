-- A durable, coalescing outbox. Triggers cover every writer, including sync,
-- manual edits and soft/hard deletion, in the same transaction as the memory.
CREATE TABLE memory_index_jobs (
    memory_id uuid PRIMARY KEY,
    user_id uuid NOT NULL,
    generation bigint NOT NULL DEFAULT 1,
    attempts integer NOT NULL DEFAULT 0,
    available_at timestamptz NOT NULL DEFAULT now(),
    lease_id uuid,
    locked_until timestamptz,
    last_error text
);
CREATE INDEX memory_index_jobs_available_idx ON memory_index_jobs(available_at);

CREATE FUNCTION enqueue_memory_index() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        INSERT INTO memory_index_jobs(memory_id, user_id) VALUES(OLD.id, OLD.user_id)
        ON CONFLICT(memory_id) DO UPDATE SET generation=memory_index_jobs.generation+1,
            attempts=0, available_at=now(), last_error=NULL;
        RETURN OLD;
    END IF;
    INSERT INTO memory_index_jobs(memory_id, user_id) VALUES(NEW.id, NEW.user_id)
    ON CONFLICT(memory_id) DO UPDATE SET generation=memory_index_jobs.generation+1,
        user_id=EXCLUDED.user_id, attempts=0, available_at=now(), last_error=NULL;
    RETURN NEW;
END
$$;
CREATE TRIGGER memory_index_insert AFTER INSERT ON memories
FOR EACH ROW EXECUTE FUNCTION enqueue_memory_index();
CREATE TRIGGER memory_index_update AFTER UPDATE OF content, deleted_at, user_id ON memories
FOR EACH ROW WHEN (OLD.content IS DISTINCT FROM NEW.content
    OR OLD.deleted_at IS DISTINCT FROM NEW.deleted_at OR OLD.user_id IS DISTINCT FROM NEW.user_id)
EXECUTE FUNCTION enqueue_memory_index();
CREATE TRIGGER memory_index_delete AFTER DELETE ON memories
FOR EACH ROW EXECUTE FUNCTION enqueue_memory_index();

-- Reconcile existing records once, including previously soft-deleted points.
INSERT INTO memory_index_jobs(memory_id,user_id) SELECT id,user_id FROM memories;
