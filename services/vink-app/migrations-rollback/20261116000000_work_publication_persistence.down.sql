-- Only pre-activation backout of #158. Once any author publication command was accepted (every
-- capture, request and withdrawal writes a receipt) or any evaluation ticket or non-legacy
-- moderation fact exists, the canonical publication state is the only record of the author's
-- audience intent: preserve it and roll forward. On unused schema the bootstrap rows are derived
-- from `publications`, so dropping them loses nothing. `publications.generation` stays >= 2, which
-- the previous schema accepts (CHECK generation >= 1) and no previous reader interprets.
BEGIN;
LOCK TABLE publications, work_publications, work_revision_moderation, work_evaluation_jobs,
    work_publication_command_receipts IN ACCESS EXCLUSIVE MODE;
DO $$ BEGIN
    IF EXISTS (SELECT 1 FROM work_publication_command_receipts)
       OR EXISTS (SELECT 1 FROM work_evaluation_jobs)
       OR EXISTS (SELECT 1 FROM work_revision_moderation
                   WHERE provenance_kind IS DISTINCT FROM 'legacy_bootstrap')
       OR EXISTS (SELECT 1 FROM revisions WHERE metadata_snapshot_id IS NOT NULL
                   AND NOT EXISTS (SELECT 1 FROM publications p
                                    WHERE p.work_id = revisions.work_id
                                      AND p.revision_id = revisions.id
                                      AND p.metadata_snapshot_id = revisions.metadata_snapshot_id))
    THEN
        RAISE EXCEPTION 'author publication state exists; preserve it and roll forward';
    END IF;
END $$;
DROP TABLE work_publication_command_receipts, work_evaluation_jobs, work_revision_moderation,
    work_publications;
ALTER TABLE revisions DROP CONSTRAINT revisions_metadata_snapshot_fkey,
    DROP COLUMN metadata_snapshot_id;
DELETE FROM _sqlx_migrations WHERE version = 20261116000000;
COMMIT;
