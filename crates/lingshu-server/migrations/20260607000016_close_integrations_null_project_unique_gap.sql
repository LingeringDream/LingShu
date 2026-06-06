-- PostgreSQL UNIQUE constraints treat NULLs as distinct values, so the
-- existing UNIQUE(user_id, project_id, platform) on the integrations table
-- does not prevent duplicate rows when project_id IS NULL.
--
-- This partial index closes that gap: at most one row per (user_id, platform)
-- can have a NULL project_id, enforced by the DB rather than relying solely on
-- the application-level IS NOT DISTINCT FROM pre-check in create_integration.
CREATE UNIQUE INDEX integrations_user_platform_no_project
    ON integrations (user_id, platform)
    WHERE project_id IS NULL;
