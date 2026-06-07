-- Fix thought_queue status vocabulary mismatch between the application layer
-- and the DB CHECK constraint.
--
-- Old CHECK:  pending | shown | confirmed | dismissed | expired
-- New CHECK:  pending | shown | accepted | dismissed | snoozed | expired
--
-- "confirmed" was the old name for what is now "accepted".
-- Historical "confirmed" rows are migrated to "accepted".
-- "snoozed" is a new status for suggestions the user wants to defer.

-- 1. Add shown_at column (set for rows already in shown status)
ALTER TABLE thought_queue
    ADD COLUMN shown_at TIMESTAMPTZ;

UPDATE thought_queue
SET shown_at = updated_at
WHERE status = 'shown';

-- 2. Migrate confirmed → accepted before altering the constraint
UPDATE thought_queue
SET status = 'accepted'
WHERE status = 'confirmed';

-- 3. Replace the CHECK constraint
ALTER TABLE thought_queue
    DROP CONSTRAINT thought_queue_status_check;

ALTER TABLE thought_queue
    ADD CONSTRAINT thought_queue_status_check
    CHECK (status IN ('pending', 'shown', 'accepted', 'dismissed', 'snoozed', 'expired'));
