-- Stop writers first. The original validated CHECK refuses a live re-review
-- (pending version > 1). On refusal roll forward; never erase or renumber decisions.
BEGIN;
ALTER TABLE work_revision_moderation
    DROP CONSTRAINT work_revision_moderation_pending_check,
    ADD CONSTRAINT work_revision_moderation_pending_check CHECK (
        moderation <> 'pending' OR (
            decision_version = 1
            AND provenance_kind IS NULL
            AND policy_reason_code IS NULL
            AND needs_admin_cause IS NULL
            AND admin_actor IS NULL
            AND admin_verdict IS NULL
            AND admin_reason IS NULL
        )
    );
DROP TRIGGER work_revision_moderation_pending_transition ON work_revision_moderation;
DROP FUNCTION check_work_pending_transition();
DELETE FROM _sqlx_migrations WHERE version = 20261121120000;
COMMIT;
