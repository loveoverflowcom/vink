-- Stop writers. Keep both re-review guards installed if the v1 schema cannot
-- represent a live decision. The earlier down script restores the original CHECK.
BEGIN;
LOCK TABLE work_revision_moderation IN ACCESS EXCLUSIVE MODE;
DO $$
BEGIN
    IF EXISTS (SELECT FROM work_revision_moderation WHERE moderation = 'pending' AND decision_version > 1) THEN
        RAISE EXCEPTION 'resolve live policy re-reviews before backout'
            USING ERRCODE = '23514', CONSTRAINT = 'work_revision_moderation_pending_check';
    END IF;
END
$$;
DROP TRIGGER work_rereview_evidence ON work_revision_moderation;
DROP TRIGGER work_human_rereview_insert ON work_revision_moderation;
DROP FUNCTION check_work_rereview_evidence();
DELETE FROM _sqlx_migrations WHERE version = 20261121120100;
COMMIT;
