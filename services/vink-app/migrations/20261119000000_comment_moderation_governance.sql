-- Comment moderation governance provenance (ADM-01/ADM-03).
-- Extends comment_moderation_decisions so decisions recorded under admin seats or
-- moderation_grants carry explicit audit provenance alongside inherited legacy grants.

ALTER TABLE comment_moderation_decisions
    DROP CONSTRAINT comment_moderation_decisions_grant_required;

ALTER TABLE comment_moderation_decisions
    ADD COLUMN authority_kind text NOT NULL DEFAULT 'legacy_grant'
        CHECK (authority_kind IN ('legacy_grant', 'governance_grant', 'admin')),
    ADD COLUMN governance_grant_id uuid REFERENCES moderation_grants (id);

ALTER TABLE comment_moderation_decisions
    ADD CONSTRAINT comment_moderation_decisions_provenance_required CHECK (
        (authority_kind = 'legacy_grant' AND grant_id IS NOT NULL AND governance_grant_id IS NULL)
        OR (authority_kind = 'governance_grant' AND grant_id IS NULL AND governance_grant_id IS NOT NULL)
        OR (authority_kind = 'admin' AND grant_id IS NULL AND governance_grant_id IS NULL)
    );

CREATE INDEX comment_moderation_decisions_governance_grant_idx
    ON comment_moderation_decisions (governance_grant_id)
    WHERE governance_grant_id IS NOT NULL;

-- Check provenance on insert. Later revocations or seat/membership changes must leave history intact.
CREATE FUNCTION check_comment_decision_authority() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF (CASE NEW.authority_kind
        WHEN 'legacy_grant' THEN NEW.grant_id IS NOT NULL AND NOT EXISTS (
            SELECT 1 FROM discussion_moderator_grants g
             WHERE g.id = NEW.grant_id AND g.principal_id = NEW.moderator_id
               AND g.revoked_at IS NULL
        )
        WHEN 'governance_grant' THEN NEW.governance_grant_id IS NOT NULL AND NOT EXISTS (
            SELECT 1
              FROM moderation_grants g
              JOIN comments c ON c.id = NEW.comment_id
              JOIN discussion_threads t ON t.id = c.thread_id
             WHERE g.id = NEW.governance_grant_id AND g.capability = 'comments' AND g.origin = 'admin'
               AND NOT g.revoked AND (g.expires_at IS NULL OR NEW.decided_at < g.expires_at)
               AND (g.scope_kind = 'global' OR g.scope_work_id = t.work_id)
               AND ((g.grantee_kind = 'principal' AND g.grantee_id = NEW.moderator_id)
                    OR (g.grantee_kind = 'group' AND EXISTS (
                        SELECT 1 FROM moderation_group_members m
                         WHERE m.group_id = g.grantee_id AND m.principal_id = NEW.moderator_id
                    )))
        )
        WHEN 'admin' THEN NOT EXISTS (
            SELECT 1 FROM moderation_admin_seats s WHERE s.principal_id = NEW.moderator_id
        )
        ELSE false
    END) THEN
        RAISE EXCEPTION 'comment decision authority does not match the actor or target'
            USING ERRCODE = '23514', CONSTRAINT = 'comment_moderation_decisions_authority_matches';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER comment_moderation_decisions_authority_matches
    BEFORE INSERT ON comment_moderation_decisions
    FOR EACH ROW EXECUTE FUNCTION check_comment_decision_authority();
