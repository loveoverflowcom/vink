-- Comment moderation authority (D2T-02c). Vink owns discussion, so it also owns who may moderate
-- it: an operator grants or revokes a principal with `vink-app moderator`; every decision rechecks
-- an ACTIVE grant inside its own transaction (FOR SHARE), so a revocation that commits first wins
-- and one in flight waits. Work ownership never implies a grant; recusal is enforced in Rust.

CREATE TABLE discussion_moderator_grants (
    id uuid PRIMARY KEY,
    principal_id uuid NOT NULL REFERENCES principals (id),
    note text NOT NULL CHECK (char_length(note) BETWEEN 1 AND 200),
    granted_at timestamptz NOT NULL DEFAULT now(),
    revoked_at timestamptz,
    CHECK (revoked_at IS NULL OR revoked_at >= granted_at)
);

-- At most one active grant per principal; revoked grants stay as history.
CREATE UNIQUE INDEX discussion_moderator_grants_active_key
    ON discussion_moderator_grants (principal_id) WHERE revoked_at IS NULL;

COMMENT ON TABLE discussion_moderator_grants IS
    'Who may moderate comments. Created/revoked only by the operator command; a revoked grant is never reactivated (grant again instead).';

CREATE FUNCTION guard_moderator_grant_update() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.id IS DISTINCT FROM OLD.id
        OR NEW.principal_id IS DISTINCT FROM OLD.principal_id
        OR NEW.note IS DISTINCT FROM OLD.note
        OR NEW.granted_at IS DISTINCT FROM OLD.granted_at
        OR OLD.revoked_at IS NOT NULL
    THEN
        RAISE EXCEPTION 'moderator grant % can only be revoked once', OLD.id
            USING ERRCODE = 'integrity_constraint_violation';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER discussion_moderator_grants_guard_update
    BEFORE UPDATE ON discussion_moderator_grants
    FOR EACH ROW EXECUTE FUNCTION guard_moderator_grant_update();

-- Each decision names the grant that authorized it and, for reject/remove, the reason shown to
-- the writer (MOD-04). NOT VALID: enforced for every new decision; this PR's earlier migration
-- created the table, and no route could write decisions before this one.
ALTER TABLE comment_moderation_decisions
    ADD COLUMN grant_id uuid REFERENCES discussion_moderator_grants (id),
    ADD COLUMN reason text CHECK (reason IS NULL OR char_length(reason) BETWEEN 1 AND 500);

ALTER TABLE comment_moderation_decisions
    ADD CONSTRAINT comment_moderation_decisions_grant_required CHECK (grant_id IS NOT NULL) NOT VALID,
    ADD CONSTRAINT comment_moderation_decisions_reason_required
        CHECK (action = 'approve' OR reason IS NOT NULL) NOT VALID;
