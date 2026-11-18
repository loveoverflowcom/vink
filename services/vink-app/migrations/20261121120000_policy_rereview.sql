-- PUB-06 / ADM-06. A fresh submission is v1. Reopening a non-human approval
-- (at least v2) consumes one further version and binds a strictly newer policy.
ALTER TABLE work_revision_moderation
    DROP CONSTRAINT work_revision_moderation_pending_check,
    ADD CONSTRAINT work_revision_moderation_pending_check CHECK (
        CASE WHEN moderation = 'pending' THEN
            (decision_version = 1 OR (decision_version >= 3 AND policy_version > 1))
            AND provenance_kind IS NULL AND policy_reason_code IS NULL
            AND needs_admin_cause IS NULL AND admin_actor IS NULL
            AND admin_verdict IS NULL AND admin_reason IS NULL AND moderator_grant_id IS NULL
        -- The verdict-specific checks use IN/=, which yield UNKNOWN for SQL NULL.
        ELSE provenance_kind IS NOT NULL
            AND (provenance_kind NOT IN ('admin', 'moderator') OR admin_verdict IS NOT NULL)
        END
    );

-- A row CHECK alone would also admit a manufactured v3 or erase a human decision.
-- AFTER sees the actual INSERT or UPDATE taken by the decision writer's upsert.
CREATE FUNCTION check_work_pending_transition() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.moderation <> 'pending' THEN
        -- Fence the decision that closes the newly representable pending state.
        IF TG_OP = 'INSERT' OR OLD.moderation <> 'pending' OR OLD.decision_version = 1
           OR ((NEW.work_id, NEW.revision_id) = (OLD.work_id, OLD.revision_id)
               AND NEW.decision_version - 1 = OLD.decision_version
               AND NEW.policy_version >= OLD.policy_version
               AND EXISTS (SELECT FROM work_publications p WHERE p.work_id = NEW.work_id
                           AND p.policy_version = NEW.policy_version)) THEN
            RETURN NULL;
        END IF;
    ELSIF TG_OP = 'INSERT' THEN
        IF NEW.decision_version = 1 THEN RETURN NULL; END IF;
    ELSIF (NEW.work_id, NEW.revision_id) = (OLD.work_id, OLD.revision_id) THEN
        -- A resubmission can requeue an existing decision, even after another policy bump.
        IF OLD.moderation = 'pending' AND NEW.decision_version = OLD.decision_version
           AND NEW.policy_version = OLD.policy_version THEN
            RETURN NULL;
        END IF;
        IF OLD.moderation = 'approved' AND OLD.provenance_kind IN ('ai_approved', 'legacy_bootstrap')
           AND NEW.decision_version - 1 = OLD.decision_version
           AND NEW.policy_version > OLD.policy_version
           AND EXISTS (SELECT FROM work_publications p WHERE p.work_id = NEW.work_id
                       AND p.policy_version = NEW.policy_version
                       AND p.intent_revision_id = NEW.revision_id
                       AND p.pointer_revision_id IS DISTINCT FROM NEW.revision_id) THEN
            RETURN NULL;
        END IF;
    END IF;
    RAISE EXCEPTION 'invalid pending work moderation transition'
        USING ERRCODE = '23514', CONSTRAINT = 'work_revision_moderation_pending_transition';
END
$$;

CREATE TRIGGER work_revision_moderation_pending_transition
AFTER INSERT OR UPDATE ON work_revision_moderation
FOR EACH ROW
EXECUTE FUNCTION check_work_pending_transition();
