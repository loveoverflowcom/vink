-- Pre-activation only. Apply before the #159 backout; never discard an appeal or its conclusion.
BEGIN;
LOCK TABLE work_appeals, work_revision_decision_history IN ACCESS EXCLUSIVE MODE;
DO $$ BEGIN
    IF EXISTS (SELECT 1 FROM work_appeals)
       OR EXISTS (SELECT 1 FROM work_revision_decision_history WHERE appeal_id IS NOT NULL)
    THEN RAISE EXCEPTION 'work appeals exist; preserve history and roll forward'; END IF;
END $$;
ALTER TABLE work_revision_decision_history DROP COLUMN appeal_id;
DROP TABLE work_appeals;
DELETE FROM _sqlx_migrations WHERE version = 20261118000000;
COMMIT;
