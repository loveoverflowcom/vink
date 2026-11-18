-- SOC-FEED-01 (#291). Classify the pre-cutover cohort and install both triggers in
-- one transaction. A busy writer can finish normally; retry migration on lock_not_available.
LOCK TABLE publications, works IN SHARE ROW EXCLUSIVE MODE NOWAIT;

CREATE TABLE IF NOT EXISTS work_first_publications (
    work_id uuid PRIMARY KEY REFERENCES works (id) ON DELETE CASCADE,
    provenance text NOT NULL,
    source text NOT NULL,
    first_published_at timestamptz,
    legacy_published_at timestamptz,
    CHECK (CASE provenance
        WHEN 'never_public' THEN source IN ('new_work', 'no_revision')
            AND first_published_at IS NULL AND legacy_published_at IS NULL
        WHEN 'exact' THEN source = 'pointer_insert'
            AND first_published_at IS NOT NULL AND legacy_published_at IS NULL
        WHEN 'legacy' THEN source = 'existing_pointer'
            AND first_published_at IS NULL AND legacy_published_at IS NOT NULL
        WHEN 'unknown' THEN source IN ('withdrawn_without_time', 'incomplete_history')
            AND first_published_at IS NULL AND legacy_published_at IS NULL
        ELSE false END)
);

-- A pointer's mutable published_at only dates that pointer. An empty epoch or an
-- absent pointer cannot prove a revision was never public before paired state existed.
-- The revision identity index suffices; no manuscript, digest or decision is read.
INSERT INTO work_first_publications (work_id, provenance, source, legacy_published_at)
SELECT w.id,
       CASE WHEN p.work_id IS NOT NULL THEN 'legacy'
            WHEN history.was_public OR history.has_revision THEN 'unknown'
            ELSE 'never_public' END,
       CASE WHEN p.work_id IS NOT NULL THEN 'existing_pointer'
            WHEN history.was_public THEN 'withdrawn_without_time'
            WHEN history.has_revision THEN 'incomplete_history'
            ELSE 'no_revision' END,
       p.published_at
  FROM works w
  LEFT JOIN publications p ON p.work_id = w.id
  LEFT JOIN work_publications wp ON wp.work_id = w.id
 CROSS JOIN LATERAL (
       SELECT wp.public_epoch > 0 OR cardinality(wp.ever_public) > 0
              OR wp.pointer_revision_id IS NOT NULL OR wp.hidden_revision_id IS NOT NULL
              OR wp.review_withdrawn_revision_id IS NOT NULL OR wp.tombstone_revision_id IS NOT NULL
                AS was_public,
              EXISTS (SELECT FROM revisions r WHERE r.work_id = w.id) AS has_revision
 ) history
ON CONFLICT (work_id) DO NOTHING;

-- Once classified, a work may only move from known-unpublished to exact with
-- a live pointer. Withdrawing the pointer never authorizes deleting its history.
CREATE OR REPLACE FUNCTION guard_first_publication() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'UPDATE' AND OLD.provenance = 'never_public' AND NEW.provenance = 'exact'
       AND NEW.work_id = OLD.work_id AND EXISTS (SELECT FROM publications WHERE work_id = OLD.work_id) THEN
        RETURN NEW;
    END IF;
    IF TG_OP = 'DELETE' AND NOT EXISTS (SELECT FROM works WHERE id = OLD.work_id) THEN
        RETURN OLD; -- The owning work itself was physically deleted (FK cascade).
    END IF;
    RAISE EXCEPTION 'first-publication history is immutable for work %', OLD.work_id
        USING ERRCODE = 'integrity_constraint_violation';
END;
$$;
CREATE OR REPLACE TRIGGER guard_first_publication BEFORE UPDATE OR DELETE ON work_first_publications
    FOR EACH ROW EXECUTE FUNCTION guard_first_publication();

CREATE OR REPLACE FUNCTION initialize_first_publication() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    INSERT INTO work_first_publications (work_id, provenance, source)
    VALUES (NEW.id, 'never_public', 'new_work');
    RETURN NULL;
END;
$$;
CREATE OR REPLACE TRIGGER initialize_first_publication AFTER INSERT ON works
    FOR EACH ROW EXECUTE FUNCTION initialize_first_publication();

CREATE OR REPLACE FUNCTION record_first_publication() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    -- This AFTER INSERT only runs for the winner, after any wait on the pointer.
    -- A replacement via ON CONFLICT DO UPDATE never reaches this trigger.
    UPDATE work_first_publications
       SET provenance = 'exact', source = 'pointer_insert', first_published_at = clock_timestamp()
     WHERE work_id = NEW.work_id AND provenance = 'never_public';
    IF NOT FOUND AND NOT EXISTS (SELECT FROM work_first_publications WHERE work_id = NEW.work_id) THEN
        RAISE EXCEPTION 'missing first-publication history for work %', NEW.work_id
            USING ERRCODE = 'integrity_constraint_violation';
    END IF;
    RETURN NULL;
END;
$$;
CREATE OR REPLACE TRIGGER record_first_publication AFTER INSERT ON publications
    FOR EACH ROW EXECUTE FUNCTION record_first_publication();

-- Publication's set-based input for #296. Discovery still owns viewer membership,
-- author eligibility, ordering and cursor; revision/metadata come from today's pointer.
CREATE OR REPLACE VIEW publications_with_exact_first_time AS
SELECT p.work_id, p.revision_id, p.metadata_snapshot_id, f.first_published_at, f.provenance
  FROM publications p
  JOIN work_first_publications f USING (work_id)
 WHERE f.provenance = 'exact';
