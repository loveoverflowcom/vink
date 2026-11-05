-- Literary endorsement (LVE, #73), PostgreSQL slice. One positive signal per actor and target:
-- one published REVISION of a work ("Đánh giá cao", CHOICE-02 B) or one original root comment or
-- flat reply ("Hữu ích"). services/vink-app/src/domain/endorsement.rs (`decide_endorsement`)
-- decides every write; this schema holds the structural rules on its own.
--
-- Storage (CHOICE-04 A): one table per target kind, each with plain foreign keys to its exact
-- target; a work row's revision must belong to that work. No polymorphic (target_type, target_id)
-- pair exists. Receipts name their target with typed nullable keys and an exactly-one CHECK.
--
-- Ordering state (core CHOICE-03 B): every accepted command advances `accepted_order` (also when
-- the selection already held); `version` advances only when the selection changes. A row exists
-- only after an accepted command, so a stored order is >= 2 (absent = OFF, version 1, order 1),
-- and 1 <= version <= order. Removing keeps the row: deleting it would restart the order and let
-- an old command apply again (ABA). Triggers hold that without Rust: identity is immutable, the
-- order only advances, the version never decreases, rows and receipts are never deleted or
-- truncated. Purging belongs to a future account/target policy with its own migration.
--
-- Receipts (`endorsement_receipts`): one per (actor, command key) across ALL targets, with the
-- exact command (target, expected order, intent) and what it committed. Append-only; no
-- retention policy exists yet, so they are kept.
--
-- Aggregates (CHOICE-05 A) are counted on read from the canonical rows; there is no counter,
-- outbox or cache to reconcile. Self-endorsement, visibility and capability are decisions of the
-- pure core, rechecked in each write transaction, not constraints here.

CREATE TABLE work_endorsements (
    work_id uuid NOT NULL CONSTRAINT work_endorsements_work_id_fkey REFERENCES works (id),
    revision_id uuid NOT NULL,
    actor_id uuid NOT NULL CONSTRAINT work_endorsements_actor_id_fkey REFERENCES principals (id),
    selected boolean NOT NULL,
    version bigint NOT NULL,
    accepted_order bigint NOT NULL,
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT work_endorsements_pkey PRIMARY KEY (work_id, revision_id, actor_id),
    CONSTRAINT work_endorsements_tokens_check
        CHECK (version >= 1 AND accepted_order >= 2 AND version <= accepted_order),
    -- The anchor revision belongs to the SAME work.
    CONSTRAINT work_endorsements_revision_of_work_fkey
        FOREIGN KEY (work_id, revision_id) REFERENCES revisions (work_id, id)
);

COMMENT ON TABLE work_endorsements IS
    'LVE endorsements of one revision of a work. Counted only while that revision is the current publication; a removed row keeps its order. Rows are never deleted.';

CREATE INDEX work_endorsements_selected_idx ON work_endorsements (work_id, revision_id)
    WHERE selected;

CREATE TABLE comment_endorsements (
    comment_id uuid NOT NULL CONSTRAINT comment_endorsements_comment_id_fkey
        REFERENCES comments (id),
    actor_id uuid NOT NULL CONSTRAINT comment_endorsements_actor_id_fkey
        REFERENCES principals (id),
    selected boolean NOT NULL,
    version bigint NOT NULL,
    accepted_order bigint NOT NULL,
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT comment_endorsements_pkey PRIMARY KEY (comment_id, actor_id),
    CONSTRAINT comment_endorsements_tokens_check
        CHECK (version >= 1 AND accepted_order >= 2 AND version <= accepted_order)
);

COMMENT ON TABLE comment_endorsements IS
    'LVE root/reply endorsements. The original comment is the anchor; its work is reached through comments.thread_id, never stored twice. Rows are never deleted.';

CREATE INDEX comment_endorsements_selected_idx ON comment_endorsements (comment_id)
    WHERE selected;

CREATE TABLE endorsement_receipts (
    actor_id uuid NOT NULL CONSTRAINT endorsement_receipts_actor_id_fkey
        REFERENCES principals (id),
    command_key uuid NOT NULL,
    -- The command's target: a work revision, or a comment (its work comes from its thread).
    work_id uuid,
    revision_id uuid,
    comment_id uuid CONSTRAINT endorsement_receipts_comment_id_fkey REFERENCES comments (id),
    expected_order bigint NOT NULL CHECK (expected_order >= 1),
    -- The intent, which is also the committed selection.
    selected boolean NOT NULL,
    committed_version bigint NOT NULL,
    committed_order bigint NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT endorsement_receipts_pkey PRIMARY KEY (actor_id, command_key),
    CONSTRAINT endorsement_receipts_one_target CHECK (
        (work_id IS NOT NULL AND revision_id IS NOT NULL AND comment_id IS NULL)
        OR (work_id IS NULL AND revision_id IS NULL AND comment_id IS NOT NULL)
    ),
    CONSTRAINT endorsement_receipts_committed_check CHECK (
        committed_order = expected_order + 1
        AND committed_version >= 1 AND committed_version <= committed_order
    ),
    CONSTRAINT endorsement_receipts_revision_of_work_fkey
        FOREIGN KEY (work_id, revision_id) REFERENCES revisions (work_id, id)
);

COMMENT ON TABLE endorsement_receipts IS
    'LVE command receipts: same actor + key with the same command replays the committed state; another command is refused. Append-only; retention is not implemented.';

-- No index leads with actor_id on the state tables: principals are never deleted, so the actor
-- foreign keys have no reverse lookup to serve; add one with the account purge slice.

CREATE FUNCTION guard_endorsement_update() RETURNS trigger
LANGUAGE plpgsql AS $$
DECLARE
    column_name text;
BEGIN
    -- TG_ARGV names the target columns of the table.
    IF NEW.actor_id IS DISTINCT FROM OLD.actor_id THEN
        RAISE EXCEPTION '% row identity is immutable', TG_TABLE_NAME
            USING ERRCODE = 'integrity_constraint_violation';
    END IF;
    FOREACH column_name IN ARRAY TG_ARGV LOOP
        IF to_jsonb(NEW) -> column_name IS DISTINCT FROM to_jsonb(OLD) -> column_name THEN
            RAISE EXCEPTION '% row identity is immutable', TG_TABLE_NAME
                USING ERRCODE = 'integrity_constraint_violation';
        END IF;
    END LOOP;
    IF NEW.accepted_order <= OLD.accepted_order THEN
        RAISE EXCEPTION '% accepted order must advance (% -> %)', TG_TABLE_NAME,
            OLD.accepted_order, NEW.accepted_order
            USING ERRCODE = 'integrity_constraint_violation';
    END IF;
    IF NEW.version < OLD.version THEN
        RAISE EXCEPTION '% version cannot decrease', TG_TABLE_NAME
            USING ERRCODE = 'integrity_constraint_violation';
    END IF;
    RETURN NEW;
END;
$$;

CREATE FUNCTION reject_endorsement_removal() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION '% rows are retained', TG_TABLE_NAME
        USING ERRCODE = 'integrity_constraint_violation';
END;
$$;

CREATE TRIGGER work_endorsements_guard_update
    BEFORE UPDATE ON work_endorsements
    FOR EACH ROW EXECUTE FUNCTION guard_endorsement_update('work_id', 'revision_id');

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

CREATE TRIGGER endorsement_receipts_append_only
    BEFORE UPDATE OR DELETE OR TRUNCATE ON endorsement_receipts
    FOR EACH STATEMENT EXECUTE FUNCTION reject_endorsement_removal();
