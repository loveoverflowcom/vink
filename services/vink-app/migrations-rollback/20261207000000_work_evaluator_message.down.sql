-- Only safe before the first evaluator message is stored. Roll forward afterwards.
BEGIN;
LOCK TABLE work_revision_decision_history IN ACCESS EXCLUSIVE MODE;
DO $$ BEGIN
    IF EXISTS (SELECT FROM work_revision_decision_history WHERE evaluator_message IS NOT NULL) THEN
        RAISE EXCEPTION 'private evaluator messages exist; preserve history and roll forward';
    END IF;
END $$;
ALTER TABLE work_revision_decision_history DROP COLUMN evaluator_message;
DELETE FROM _sqlx_migrations WHERE version = 20261207000000;
COMMIT;
