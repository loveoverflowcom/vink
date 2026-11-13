-- Pre-activation backout of comment moderation governance provenance.
-- Refuses once any governance-grant or admin comment decision exists.
BEGIN;
LOCK TABLE comment_moderation_decisions IN ACCESS EXCLUSIVE MODE;
DO $$ BEGIN
    IF EXISTS (
        SELECT 1 FROM comment_moderation_decisions
         WHERE authority_kind <> 'legacy_grant'
            OR grant_id IS NULL
            OR governance_grant_id IS NOT NULL
    )
    THEN
        RAISE EXCEPTION 'comment moderation governance decisions exist; preserve history and roll forward';
    END IF;
END $$;
DROP INDEX IF EXISTS comment_moderation_decisions_governance_grant_idx;
ALTER TABLE comment_moderation_decisions
    DROP CONSTRAINT comment_moderation_decisions_provenance_required,
    DROP COLUMN governance_grant_id,
    DROP COLUMN authority_kind;
ALTER TABLE comment_moderation_decisions
    ADD CONSTRAINT comment_moderation_decisions_grant_required CHECK (grant_id IS NOT NULL) NOT VALID;
DELETE FROM _sqlx_migrations WHERE version = 20261119000000;
COMMIT;
