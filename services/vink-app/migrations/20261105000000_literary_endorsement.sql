-- Literary endorsement (LVE, #73), PostgreSQL slice. One positive signal per (actor, target)
-- on a public work ("Đánh giá cao"), a root comment or a flat reply ("Hữu ích"); policy
-- `lve-core-v1` in services/vink-app/src/domain/endorsement.rs decides every write.
--
-- Storage (CHOICE-04 A): one table per target kind, because the anchors differ: a work
-- endorsement selects the REVISION the reader read (CHOICE-02 C) and a comment endorsement
-- selects the comment itself (bodies are immutable, D-08). Plain foreign keys give each table
-- its exact target kind; no polymorphic (target_type, target_id) pair exists.
--
-- Ordering state (CHOICE-03, accepted-intent version): a row exists only after an accepted
-- intent, so a stored version is >= 2 (an actor without a row is version 1, not selected).
-- Clearing keeps the row with its advanced version: deleting it would restart the version and
-- let an old duplicate apply again (ABA). The triggers below hold that without Rust: identity
-- is immutable, every update advances the version, and DELETE/TRUNCATE are refused. Removing
-- rows belongs to a future account/target purge policy with its own migration.
--
-- Aggregates (CHOICE-05 A) are counted on read from these canonical rows; there is no counter,
-- outbox or cache to reconcile. Self-endorsement, visibility and comments-closed are policy
-- decisions of the pure core, rechecked in each write transaction, not constraints here.

CREATE TABLE work_endorsements (
    work_id uuid NOT NULL CONSTRAINT work_endorsements_work_id_fkey REFERENCES works (id),
    actor_id uuid NOT NULL CONSTRAINT work_endorsements_actor_id_fkey REFERENCES principals (id),
    -- The anchor: the revision the actor read and endorsed; NULL once cleared.
    revision_id uuid,
    version bigint NOT NULL CONSTRAINT work_endorsements_version_check CHECK (version >= 2),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT work_endorsements_pkey PRIMARY KEY (work_id, actor_id),
    -- The anchor revision belongs to the SAME work (MATCH SIMPLE: not checked while NULL).
    CONSTRAINT work_endorsements_revision_of_work_fkey
        FOREIGN KEY (work_id, revision_id) REFERENCES revisions (work_id, id)
);

COMMENT ON TABLE work_endorsements IS
    'LVE work endorsements (lve-core-v1). Effective (counted) only while revision_id is the current publication''s revision; a cleared row keeps its version. Rows are never deleted.';

-- Counting one work's effective selections: (work_id, revision_id = current) is an index-only
-- range. Own-state reads use the primary key.
CREATE INDEX work_endorsements_anchor_idx ON work_endorsements (work_id, revision_id)
    WHERE revision_id IS NOT NULL;

CREATE TABLE comment_endorsements (
    comment_id uuid NOT NULL CONSTRAINT comment_endorsements_comment_id_fkey
        REFERENCES comments (id),
    actor_id uuid NOT NULL CONSTRAINT comment_endorsements_actor_id_fkey
        REFERENCES principals (id),
    selected boolean NOT NULL,
    version bigint NOT NULL CONSTRAINT comment_endorsements_version_check CHECK (version >= 2),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT comment_endorsements_pkey PRIMARY KEY (comment_id, actor_id)
);

COMMENT ON TABLE comment_endorsements IS
    'LVE root/reply endorsements (lve-core-v1). The comment is the anchor; the work is reached through comments.thread_id, never stored twice. Rows are never deleted.';

CREATE INDEX comment_endorsements_selected_idx ON comment_endorsements (comment_id)
    WHERE selected;

-- No index leads with actor_id: principals are never deleted, so the actor foreign keys have no
-- reverse lookup to serve; add one with the account purge slice after measuring.

CREATE FUNCTION guard_endorsement_update() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    -- TG_ARGV[0] names the target column of the table.
    IF NEW.actor_id IS DISTINCT FROM OLD.actor_id
        OR to_jsonb(NEW) -> TG_ARGV[0] IS DISTINCT FROM to_jsonb(OLD) -> TG_ARGV[0]
    THEN
        RAISE EXCEPTION '% row identity is immutable', TG_TABLE_NAME
            USING ERRCODE = 'integrity_constraint_violation';
    END IF;
    IF NEW.version <= OLD.version THEN
        RAISE EXCEPTION '% version must advance (% -> %)', TG_TABLE_NAME, OLD.version, NEW.version
            USING ERRCODE = 'integrity_constraint_violation';
    END IF;
    RETURN NEW;
END;
$$;

CREATE FUNCTION reject_endorsement_removal() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION '% rows are retained: clearing keeps the row and its version', TG_TABLE_NAME
        USING ERRCODE = 'integrity_constraint_violation';
END;
$$;

CREATE TRIGGER work_endorsements_guard_update
    BEFORE UPDATE ON work_endorsements
    FOR EACH ROW EXECUTE FUNCTION guard_endorsement_update('work_id');

CREATE TRIGGER comment_endorsements_guard_update
    BEFORE UPDATE ON comment_endorsements
    FOR EACH ROW EXECUTE FUNCTION guard_endorsement_update('comment_id');

-- Statement-level, so a DELETE matching no row is refused too, and TRUNCATE cannot bypass it.
CREATE TRIGGER work_endorsements_retained
    BEFORE DELETE OR TRUNCATE ON work_endorsements
    FOR EACH STATEMENT EXECUTE FUNCTION reject_endorsement_removal();

CREATE TRIGGER comment_endorsements_retained
    BEFORE DELETE OR TRUNCATE ON comment_endorsements
    FOR EACH STATEMENT EXECUTE FUNCTION reject_endorsement_removal();
