-- Close the conflict-governance loop: disputed facts can be reviewed by an
-- agent, and the resulting reusable experience remains linked to the event.

ALTER TABLE memory_conflict_events
    ADD COLUMN review_status VARCHAR(16) NOT NULL DEFAULT 'not_required',
    ADD COLUMN reviewer_agent_id VARCHAR(64),
    ADD COLUMN review_note TEXT,
    ADD COLUMN reviewed_at TIMESTAMPTZ,
    ADD COLUMN experience_memory_id UUID REFERENCES memories(id) ON DELETE SET NULL,
    ADD CONSTRAINT memory_conflict_review_status_check
        CHECK (review_status IN ('not_required', 'pending', 'resolved'));

UPDATE memory_conflict_events
SET review_status = 'pending'
WHERE resolution = 'disputed';

CREATE INDEX idx_memory_conflicts_pending_review
    ON memory_conflict_events(user_id, created_at DESC)
    WHERE review_status = 'pending';
