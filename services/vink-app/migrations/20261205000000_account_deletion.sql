-- An Identity subject can be fenced before it has a PrincipalId. A nullable binding also
-- prevents a delayed provisioning request from creating a new author for an old subject.
CREATE TABLE account_deletions (
    account_id uuid PRIMARY KEY,
    operation_id uuid NOT NULL UNIQUE,
    accepted_at timestamptz NOT NULL,
    principal_id uuid UNIQUE REFERENCES principal_bindings(principal_id),
    purged boolean NOT NULL DEFAULT false
);

CREATE FUNCTION reject_account_deletion_change() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'UPDATE' AND NEW.account_id = OLD.account_id
       AND NEW.operation_id = OLD.operation_id AND NEW.accepted_at = OLD.accepted_at
       AND NEW.principal_id IS NOT DISTINCT FROM OLD.principal_id
       AND NEW.purged THEN
        RETURN NEW;
    END IF;
    RAISE EXCEPTION 'account deletion fence is permanent' USING ERRCODE = 'integrity_constraint_violation';
END;
$$;
CREATE TRIGGER account_deletions_immutable BEFORE UPDATE OR DELETE ON account_deletions
    FOR EACH ROW EXECUTE FUNCTION reject_account_deletion_change();
CREATE TRIGGER account_deletions_no_truncate BEFORE TRUNCATE ON account_deletions
    FOR EACH STATEMENT EXECUTE FUNCTION reject_account_deletion_change();

-- A deleted revision keeps the immutable identity, digest and moderation bindings; its
-- original title and manuscript become NULL together, exactly once, after the work retires.
ALTER TABLE revisions ALTER COLUMN title DROP NOT NULL, ALTER COLUMN document DROP NOT NULL;
ALTER TABLE revisions ADD CONSTRAINT revisions_erased_together CHECK ((title IS NULL) = (document IS NULL));
CREATE OR REPLACE FUNCTION reject_revision_update() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF OLD.title IS NOT NULL AND NEW.title IS NULL AND NEW.document IS NULL
       AND NEW.id = OLD.id AND NEW.work_id = OLD.work_id
       AND NEW.content_hash = OLD.content_hash AND NEW.created_by = OLD.created_by
       AND NEW.created_at = OLD.created_at
       AND NEW.metadata_snapshot_id IS NOT DISTINCT FROM OLD.metadata_snapshot_id
       AND EXISTS (
            SELECT 1 FROM works w JOIN account_deletions d ON d.principal_id = w.owner_id
             JOIN work_publications wp ON wp.work_id = w.id
             WHERE w.id = OLD.work_id AND wp.lifecycle IN ('author_deleted', 'removed')
       ) THEN
        RETURN NEW;
    END IF;
    RAISE EXCEPTION 'revision % is an immutable snapshot', OLD.id
        USING ERRCODE = 'integrity_constraint_violation';
END;
$$;

CREATE FUNCTION erase_retired_work_metadata() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM works w JOIN account_deletions d ON d.principal_id = w.owner_id
        JOIN work_publications wp ON wp.work_id = w.id
        WHERE w.id = CASE WHEN TG_TABLE_NAME = 'work_draft_metadata'
                          THEN (to_jsonb(OLD)->>'work_id')::uuid
                          ELSE (SELECT work_id FROM work_metadata_snapshots
                                 WHERE id = (to_jsonb(OLD)->>'snapshot_id')::uuid)
                      END
          AND wp.lifecycle IN ('author_deleted', 'removed')
    ) THEN
        RETURN OLD;
    END IF;
    RAISE EXCEPTION '% rows are retained for active work', TG_TABLE_NAME
        USING ERRCODE = 'integrity_constraint_violation';
END;
$$;
DROP TRIGGER work_draft_metadata_retained ON work_draft_metadata;
CREATE TRIGGER work_draft_metadata_retained BEFORE TRUNCATE ON work_draft_metadata
    FOR EACH STATEMENT EXECUTE FUNCTION reject_taxonomy_removal();
CREATE TRIGGER work_draft_metadata_erase BEFORE DELETE ON work_draft_metadata
    FOR EACH ROW EXECUTE FUNCTION erase_retired_work_metadata();

DROP TRIGGER work_metadata_snapshot_tags_immutable ON work_metadata_snapshot_tags;
CREATE TRIGGER work_metadata_snapshot_tags_immutable BEFORE UPDATE OR TRUNCATE ON work_metadata_snapshot_tags
    FOR EACH STATEMENT EXECUTE FUNCTION reject_metadata_snapshot_change();
CREATE TRIGGER work_metadata_snapshot_tags_erase BEFORE DELETE ON work_metadata_snapshot_tags
    FOR EACH ROW EXECUTE FUNCTION erase_retired_work_metadata();

CREATE INDEX comments_pending_author ON comments (author_id, id)
    WHERE review = 'pending' AND removal IS NULL;
CREATE INDEX work_endorsements_selected_actor ON work_endorsements (actor_id) WHERE selected;
CREATE INDEX comment_endorsements_selected_actor ON comment_endorsements (actor_id) WHERE selected;
