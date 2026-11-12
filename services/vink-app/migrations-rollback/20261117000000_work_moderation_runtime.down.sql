-- Pre-activation backout of #159 (work moderation runtime).
-- Refuses once any moderation case, grant, admin seat or automation audit exists,
-- or once any moderation record references a moderator grant.
BEGIN;
LOCK TABLE moderation_automation_state, moderation_automation_audit,
    moderation_case_notes, moderation_cases, moderation_grants,
    moderation_group_members, moderation_groups, moderation_admin_audit,
    moderation_admin_seats, work_revision_moderation, work_evaluation_jobs,
    work_publications IN ACCESS EXCLUSIVE MODE;

DO $$ BEGIN
    IF EXISTS (SELECT 1 FROM moderation_cases)
       OR EXISTS (SELECT 1 FROM moderation_grants)
       OR EXISTS (SELECT 1 FROM moderation_admin_seats)
       OR EXISTS (SELECT 1 FROM moderation_automation_audit)
       OR EXISTS (SELECT 1 FROM work_revision_moderation WHERE moderator_grant_id IS NOT NULL)
       OR EXISTS (SELECT 1 FROM work_publication_command_receipts)
    THEN
        RAISE EXCEPTION 'work moderation runtime state exists; preserve it and roll forward';
    END IF;
END $$;

-- 8. Automation mode
DROP TABLE moderation_automation_audit, moderation_automation_state;

-- 7. Cases and notes
DROP TABLE moderation_case_notes, moderation_cases;

-- 6. Revert work_revision_moderation
ALTER TABLE work_revision_moderation
    DROP CONSTRAINT work_revision_moderation_provenance_kind_check,
    DROP CONSTRAINT work_revision_moderation_approved_check,
    DROP CONSTRAINT work_revision_moderation_rejected_check;

ALTER TABLE work_revision_moderation
    DROP COLUMN moderator_grant_id,
    DROP COLUMN policy_version;

ALTER TABLE work_revision_moderation
    ADD CONSTRAINT work_revision_moderation_provenance_kind_check
        CHECK (provenance_kind IN ('ai_approved', 'ai_rejected', 'ai_needs_admin', 'admin', 'legacy_bootstrap')),
    ADD CONSTRAINT work_revision_moderation_approved_check CHECK (
        moderation <> 'approved' OR (
            decision_version > 1
            AND provenance_kind IN ('ai_approved', 'admin', 'legacy_bootstrap')
            AND (
                (provenance_kind = 'ai_approved' AND admin_actor IS NULL AND admin_verdict IS NULL AND admin_reason IS NULL AND policy_reason_code IS NULL AND needs_admin_cause IS NULL)
                OR
                (provenance_kind = 'admin' AND admin_verdict = 'approve' AND admin_actor IS NOT NULL AND admin_reason IS NOT NULL AND policy_reason_code IS NULL AND needs_admin_cause IS NULL)
                OR
                (provenance_kind = 'legacy_bootstrap' AND admin_actor IS NULL AND admin_verdict IS NULL AND admin_reason IS NULL AND policy_reason_code IS NULL AND needs_admin_cause IS NULL)
            )
        )
    ),
    ADD CONSTRAINT work_revision_moderation_rejected_check CHECK (
        moderation <> 'rejected' OR (
            decision_version > 1
            AND provenance_kind IN ('ai_rejected', 'admin')
            AND (
                (provenance_kind = 'ai_rejected' AND policy_reason_code IS NOT NULL AND admin_actor IS NULL AND admin_verdict IS NULL AND admin_reason IS NULL AND needs_admin_cause IS NULL)
                OR
                (provenance_kind = 'admin' AND admin_verdict = 'reject' AND admin_actor IS NOT NULL AND admin_reason IS NOT NULL AND policy_reason_code IS NULL AND needs_admin_cause IS NULL)
            )
        )
    );

-- 5 & 4 & 3. Grants, groups, admin seats
DROP TABLE moderation_grants, moderation_group_members, moderation_groups,
    moderation_admin_audit, moderation_admin_seats;

-- 2. Revert work_evaluation_jobs
DROP INDEX IF EXISTS work_evaluation_jobs_claim_idx;

ALTER TABLE work_evaluation_jobs
    DROP CONSTRAINT work_evaluation_jobs_state_check;

ALTER TABLE work_evaluation_jobs
    DROP COLUMN claim_generation,
    DROP COLUMN claimed_by,
    DROP COLUMN lease_expires_at,
    DROP COLUMN available_at,
    DROP COLUMN attempts,
    DROP COLUMN max_attempts,
    DROP COLUMN scenario,
    DROP COLUMN last_error;

ALTER TABLE work_evaluation_jobs
    ADD CONSTRAINT work_evaluation_jobs_state_check
        CHECK (state IN ('open', 'applied', 'failed'));

-- 1. Revert work_publications
ALTER TABLE work_publications
    DROP CONSTRAINT IF EXISTS work_publications_hidden_fkey,
    DROP CONSTRAINT IF EXISTS work_publications_review_withdrawn_fkey,
    DROP COLUMN hidden_revision_id,
    DROP COLUMN hidden_intent_generation,
    DROP COLUMN review_withdrawn_revision_id,
    DROP COLUMN review_withdrawn_intent_generation,
    DROP COLUMN ever_public,
    DROP COLUMN public_epoch;

DELETE FROM _sqlx_migrations WHERE version = 20261117000000;
COMMIT;
