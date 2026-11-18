-- The decision writer upserts the current row before appending its history. Check
-- both versions at commit so missing or contradictory evidence aborts the command.
CREATE FUNCTION check_work_rereview_evidence() RETURNS trigger
LANGUAGE plpgsql AS $$
DECLARE
    decision work_revision_moderation;
    source_case uuid;
BEGIN
    IF TG_OP = 'INSERT' THEN
        RAISE EXCEPTION 'human re-review requires a prior human approval'
            USING ERRCODE = '23514', CONSTRAINT = 'work_human_rereview_transition';
    END IF;

    IF OLD.provenance_kind = 'policy_rereview' AND NEW.provenance_kind = 'policy_rereview' THEN
        -- Requeueing the same human decision consumes no version.
        IF (NEW.work_id, NEW.revision_id, NEW.decision_version, NEW.policy_version)
            <> (OLD.work_id, OLD.revision_id, OLD.decision_version, OLD.policy_version) THEN
            RAISE EXCEPTION 'human re-review identity changed on resubmission'
                USING ERRCODE = '23514', CONSTRAINT = 'work_human_rereview_transition';
        END IF;
    ELSIF NEW.provenance_kind = 'policy_rereview' THEN
        IF (OLD.moderation = 'approved' AND OLD.provenance_kind IN ('admin', 'moderator')
            AND (NEW.work_id, NEW.revision_id) = (OLD.work_id, OLD.revision_id)
            AND NEW.decision_version - 1 = OLD.decision_version
            AND NEW.policy_version > OLD.policy_version
            AND EXISTS (SELECT FROM work_publications p WHERE p.work_id = NEW.work_id
                        AND p.policy_version = NEW.policy_version
                        AND p.intent_revision_id = NEW.revision_id
                        AND p.pointer_revision_id IS DISTINCT FROM NEW.revision_id FOR SHARE)) IS NOT TRUE THEN
            RAISE EXCEPTION 'invalid human re-review predecessor'
                USING ERRCODE = '23514', CONSTRAINT = 'work_human_rereview_transition';
        END IF;
    ELSIF OLD.provenance_kind = 'policy_rereview' THEN
        IF (NEW.moderation IN ('approved', 'rejected') AND NEW.provenance_kind IN ('admin', 'moderator')
            AND (NEW.work_id, NEW.revision_id) = (OLD.work_id, OLD.revision_id)
            AND NEW.decision_version - 1 = OLD.decision_version
            AND NEW.policy_version >= OLD.policy_version
            AND EXISTS (SELECT FROM work_publications p WHERE p.work_id = NEW.work_id
                        AND p.policy_version = NEW.policy_version
                        AND p.audience = 'public' AND p.lifecycle = 'active' FOR SHARE)) IS NOT TRUE THEN
            RAISE EXCEPTION 'invalid human re-review conclusion'
                USING ERRCODE = '23514', CONSTRAINT = 'work_human_rereview_transition';
        END IF;
    END IF;

    FOREACH decision IN ARRAY ARRAY[OLD::work_revision_moderation, NEW::work_revision_moderation] LOOP
        SELECT h.case_id INTO source_case FROM work_revision_decision_history h
         WHERE (h.work_id, h.revision_id, h.decision_version) =
               (decision.work_id, decision.revision_id, decision.decision_version)
           AND (h.moderation, h.policy_version, h.provenance_kind, h.policy_reason_code,
                h.needs_admin_cause, h.actor_id, h.moderator_grant_id, h.reason)
               IS NOT DISTINCT FROM
               (decision.moderation, decision.policy_version, decision.provenance_kind, decision.policy_reason_code,
                decision.needs_admin_cause, decision.admin_actor, decision.moderator_grant_id, decision.admin_reason)
           FOR SHARE;
        IF NOT FOUND THEN
            RAISE EXCEPTION 're-review has missing or conflicting decision history'
                USING ERRCODE = '23514', CONSTRAINT = 'work_rereview_history_match';
        END IF;
    END LOOP;

    -- The case writer revalidates standing, seat/grant and recusal. Bind its
    -- durable conclusion to this verdict, including the moderator's precise grant.
    IF OLD.provenance_kind = 'policy_rereview' AND NEW.provenance_kind IN ('admin', 'moderator')
       AND NOT EXISTS (
           SELECT FROM moderation_cases c
             JOIN revisions r ON (r.work_id, r.id) = (c.work_id, c.revision_id)
            WHERE c.id = source_case AND c.subject_kind = 'work_revision' AND c.state = 'concluded'
              AND (c.work_id, c.revision_id, c.decision_version) =
                  (OLD.work_id, OLD.revision_id, OLD.decision_version)
              AND c.content_digest = r.content_hash
              AND c.concluded_by = NEW.admin_actor AND c.concluded_verdict = NEW.admin_verdict
              AND c.concluded_basis_kind = CASE NEW.provenance_kind WHEN 'admin' THEN 'admin' ELSE 'grant' END
              AND c.concluded_basis_grant_id IS NOT DISTINCT FROM NEW.moderator_grant_id FOR SHARE
       ) THEN
        RAISE EXCEPTION 'human re-review verdict has no matching concluded case'
            USING ERRCODE = '23514', CONSTRAINT = 'work_human_rereview_case';
    END IF;
    RETURN NULL;
END
$$;

CREATE TRIGGER work_human_rereview_insert
AFTER INSERT ON work_revision_moderation FOR EACH ROW
WHEN (NEW.provenance_kind = 'policy_rereview')
EXECUTE FUNCTION check_work_rereview_evidence();

CREATE CONSTRAINT TRIGGER work_rereview_evidence
AFTER UPDATE ON work_revision_moderation DEFERRABLE INITIALLY DEFERRED FOR EACH ROW
WHEN ((OLD.moderation = 'pending' AND OLD.decision_version > 1)
   OR (NEW.moderation = 'pending' AND NEW.decision_version > 1)
   OR OLD.provenance_kind = 'policy_rereview' OR NEW.provenance_kind = 'policy_rereview')
EXECUTE FUNCTION check_work_rereview_evidence();
