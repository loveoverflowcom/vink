-- An accepted clear, even for a missing work, must fence an older save.
-- Only the saving principal owns these rows. Work disappearance does not reset its fence.
CREATE TABLE saved_works (
    principal_id uuid NOT NULL REFERENCES principals (id) ON DELETE CASCADE,
    work_id uuid NOT NULL,
    generation bigint NOT NULL CHECK (generation > 1),
    saved_at timestamptz,
    cursor_key uuid NOT NULL DEFAULT gen_random_uuid(),
    PRIMARY KEY (principal_id, work_id)
);

CREATE INDEX saved_works_recent ON saved_works (principal_id, saved_at DESC, work_id)
    WHERE saved_at IS NOT NULL;

-- Both App retirement entry points (including a pre-upgrade writer) purge in
-- their own transaction. A retained principal/binding never owns retry data.
CREATE FUNCTION purge_retired_saved_works() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    DELETE FROM saved_works WHERE principal_id = NEW.principal_id;
    RETURN NULL;
END;
$$;
CREATE TRIGGER saved_works_retirement AFTER INSERT ON social_retirements
    FOR EACH ROW EXECUTE FUNCTION purge_retired_saved_works();
CREATE TRIGGER saved_works_profile_retirement AFTER UPDATE OF current_handle ON author_profiles
    FOR EACH ROW WHEN (OLD.current_handle IS NOT NULL AND NEW.current_handle IS NULL)
    EXECUTE FUNCTION purge_retired_saved_works();

-- Live classification of the App's existing publication and account facts. Only a
-- readable target exposes a revision to the saved-list metadata join.
CREATE VIEW saved_work_targets AS
SELECT w.id AS work_id, w.owner_id, visibility.kind,
       CASE WHEN visibility.kind = 'readable' THEN p.revision_id END AS revision_id,
       CASE WHEN visibility.kind = 'readable' THEN p.metadata_snapshot_id END AS metadata_snapshot_id
FROM works w
JOIN public_author_attribution a ON a.id = w.owner_id
LEFT JOIN publications p ON p.work_id = w.id
LEFT JOIN work_publications wp ON wp.work_id = w.id
CROSS JOIN LATERAL (SELECT CASE
    WHEN wp.lifecycle = 'removed' OR wp.review_withdrawn_revision_id IS NOT NULL THEN NULL
    WHEN a.retired OR wp.lifecycle = 'author_deleted' THEN 'author_deleted'
    WHEN wp.hidden_revision_id IS NOT NULL THEN 'hidden'
    WHEN p.revision_id IS NOT NULL THEN 'readable'
END AS kind) visibility
WHERE visibility.kind IS NOT NULL;
