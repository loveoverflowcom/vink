-- Provenance of each comment's admission (#120 DISC-02/03; docs/architecture/discussion-admission.md).
-- Every existing comment was admitted under premoderation, which the backfill records truthfully.
-- No default remains: the writer names both values on INSERT, and they never change afterwards.
--
-- Lock budget: ADD COLUMN with a constant default is catalog-only (PostgreSQL 11+); the CHECK
-- constraints and the trigger take ACCESS EXCLUSIVE on `comments` while existing rows are checked,
-- which the pilot's table size keeps short. Backout: drop the trigger, its function, the two
-- constraints and the two columns; nothing else depends on them.

ALTER TABLE comments
    ADD COLUMN admission_policy text NOT NULL DEFAULT 'premoderation.v1',
    ADD COLUMN admission_reason text NOT NULL DEFAULT 'policy_premoderation';
ALTER TABLE comments
    ALTER COLUMN admission_policy DROP DEFAULT,
    ALTER COLUMN admission_reason DROP DEFAULT;

-- Known (policy, reason) pairs of `domain::discussion::admission`: premoderation has one reason;
-- the candidate reports its first failing guard, or `trusted_and_clear`.
ALTER TABLE comments ADD CONSTRAINT comments_admission_known CHECK (
    (admission_policy = 'premoderation.v1' AND admission_reason = 'policy_premoderation')
    OR (admission_policy = 'trusted-cohort.v1' AND admission_reason IN (
        'thread_reviewed', 'content_flagged', 'trust_unknown', 'not_trusted', 'trust_revoked',
        'trust_stale', 'content_unassessed', 'content_mismatch', 'content_stale',
        'trusted_and_clear'))
);

-- Pilot: premoderation is the only effective policy (COM-02, D-08). Dropping this constraint
-- belongs to an accepted D-08 delta and its reviewed code change, never to a workaround.
ALTER TABLE comments ADD CONSTRAINT comments_admission_premoderation_only CHECK (
    admission_policy = 'premoderation.v1'
);

-- A row is born exactly as its admission decided: present, at the first version, approved only
-- for `trusted_and_clear`. No import or direct INSERT can create a public comment any other way.
-- Provenance is immutable; review, removal and version keep their own guard (comments_guard_update).
CREATE FUNCTION guard_comment_admission() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        IF NEW.removal IS NOT NULL
            OR NEW.moderation_version <> 1
            -- Parenthesized: PL/pgSQL ends an IF condition at the first top-level THEN.
            OR NEW.review <> (CASE WHEN NEW.admission_reason = 'trusted_and_clear'
                                   THEN 'approved' ELSE 'pending' END)
        THEN
            RAISE EXCEPTION 'comment % initial state disagrees with its admission', NEW.id
                USING ERRCODE = 'integrity_constraint_violation';
        END IF;
    ELSIF NEW.admission_policy IS DISTINCT FROM OLD.admission_policy
        OR NEW.admission_reason IS DISTINCT FROM OLD.admission_reason
    THEN
        RAISE EXCEPTION 'comment % admission provenance is immutable', OLD.id
            USING ERRCODE = 'integrity_constraint_violation';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER comments_guard_admission
    BEFORE INSERT OR UPDATE ON comments
    FOR EACH ROW EXECUTE FUNCTION guard_comment_admission();

COMMENT ON COLUMN comments.admission_policy IS
    'Versioned admission policy that chose the initial review (premoderation.v1 in the pilot). Audit only; never shown to readers.';
COMMENT ON COLUMN comments.admission_reason IS
    'Reason code of that admission (policy_premoderation in the pilot). Audit only; never shown to readers.';
