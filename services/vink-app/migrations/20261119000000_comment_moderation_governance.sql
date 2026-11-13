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
