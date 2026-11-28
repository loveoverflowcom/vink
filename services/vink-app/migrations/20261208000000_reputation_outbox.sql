-- Điểm Vink (#600, #645; VDR-REPUTATION-CONTRACT@r1 K3): activation switch and the durable
-- outbox of accepted source changes that can move a rating. Decisions live in
-- `domain::reputation`; the consumer arrives with #646. Nothing here reads or writes a score.
--
-- Community events (`tham`, `ung`): `persistence/endorsement.rs` writes one row in the SAME
-- transaction as an accepted Thấm/Ưng whose version changed, after every lock of that path, with
-- the context it read under those locks (work public, hidden revision, comment public). A no-op,
-- a replayed receipt, a refused command or a rollback writes nothing.
--
-- Markers (`work`, `comment`, `principal`): AFTER ROW triggers on the rows an endorsement
-- transaction locks (`publications` FOR SHARE, `comments` FOR SHARE), on the work's paired
-- lifecycle row and on standing/deletion rows. A row trigger runs after its statement took the row
-- lock, so a marker is numbered after every endorsement that held that row: for conflicting
-- transactions, id order is commit order. Each marker carries the facts as of its transaction.
-- DEL-05 does not emit per-endorsement events: the `principal` marker with `principal_deleted`
-- is numbered after the actor's in-flight endorsements (account deletion locks the principal row
-- first) and makes every entitlement of that actor terminal.
--
-- Activation: producers write only while active and stamp the epoch they read, reading
-- `reputation_activation` FOR KEY SHARE (unconditionally, so a cutover waits for them).
-- `vink-app reputation activate|deactivate` takes the row FOR UPDATE; a transition commits wholly
-- before or wholly after a cutover. The row starts inactive. Nothing is backfilled here (#646).
--
-- Rows are immutable; the consumer deletes a row in the transaction that applies it (#646).
-- Backout: `migrations-rollback/20261208000000_reputation_outbox.down.sql` refuses while events
-- exist or the feature is active.

CREATE TABLE reputation_activation (
    singleton boolean PRIMARY KEY DEFAULT true CHECK (singleton),
    active boolean NOT NULL DEFAULT false,
    -- Advances on every activation; deactivation keeps it. Never 0 while active.
    epoch bigint NOT NULL DEFAULT 0 CHECK (epoch >= 0),
    changed_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT reputation_activation_epoch CHECK (NOT active OR epoch >= 1)
);

INSERT INTO reputation_activation (singleton) VALUES (true);

COMMENT ON TABLE reputation_activation IS
    'Điểm Vink cutover (#645). Producers read it FOR KEY SHARE and write events only while active, stamped with this epoch. Starts inactive.';

CREATE TABLE reputation_activation_changes (
    id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    active boolean NOT NULL,
    epoch bigint NOT NULL CHECK (epoch >= 0),
    note text NOT NULL CHECK (char_length(note) BETWEEN 1 AND 500),
    database_role text NOT NULL DEFAULT session_user,
    changed_at timestamptz NOT NULL DEFAULT now()
);

COMMENT ON TABLE reputation_activation_changes IS
    'Append-only audit of `vink-app reputation activate|deactivate`. The note names the operator and the reason.';

CREATE TABLE reputation_events (
    id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    kind text NOT NULL CHECK (kind IN ('tham', 'ung', 'work', 'comment', 'principal')),
    activation_epoch bigint NOT NULL CHECK (activation_epoch >= 1),
    -- now() of the source transaction: the accepted time that decides the period (Q41).
    accepted_at timestamptz NOT NULL,
    actor_id uuid,
    recipient_id uuid,
    work_id uuid,
    revision_id uuid,
    comment_id uuid,
    principal_id uuid,
    selected boolean,
    version bigint,
    accepted_order bigint,
    work_public boolean,
    hidden_revision_id uuid,
    comment_public boolean,
    principal_suspended boolean,
    principal_deleted boolean,
    CONSTRAINT reputation_events_shape CHECK (coalesce(CASE kind
        WHEN 'tham' THEN actor_id IS NOT NULL AND recipient_id IS NOT NULL
            AND work_id IS NOT NULL AND revision_id IS NOT NULL AND comment_id IS NULL
            AND principal_id IS NULL AND selected IS NOT NULL AND version IS NOT NULL
            AND accepted_order IS NOT NULL AND work_public IS NOT NULL AND comment_public IS NULL
            AND principal_suspended IS NULL AND principal_deleted IS NULL
        WHEN 'ung' THEN actor_id IS NOT NULL AND recipient_id IS NOT NULL
            AND work_id IS NOT NULL AND revision_id IS NULL AND comment_id IS NOT NULL
            AND principal_id IS NULL AND selected IS NOT NULL AND version IS NOT NULL
            AND accepted_order IS NOT NULL AND work_public IS NOT NULL
            AND hidden_revision_id IS NULL AND comment_public IS NOT NULL
            AND principal_suspended IS NULL AND principal_deleted IS NULL
        WHEN 'work' THEN work_id IS NOT NULL AND work_public IS NOT NULL
            AND actor_id IS NULL AND recipient_id IS NULL AND revision_id IS NULL
            AND comment_id IS NULL AND principal_id IS NULL AND selected IS NULL
            AND version IS NULL AND accepted_order IS NULL AND comment_public IS NULL
            AND principal_suspended IS NULL AND principal_deleted IS NULL
        WHEN 'comment' THEN work_id IS NOT NULL AND comment_id IS NOT NULL
            AND comment_public IS NOT NULL AND actor_id IS NULL AND recipient_id IS NULL
            AND revision_id IS NULL AND principal_id IS NULL AND selected IS NULL
            AND version IS NULL AND accepted_order IS NULL AND work_public IS NULL
            AND hidden_revision_id IS NULL AND principal_suspended IS NULL
            AND principal_deleted IS NULL
        ELSE principal_id IS NOT NULL AND principal_suspended IS NOT NULL
            AND principal_deleted IS NOT NULL AND actor_id IS NULL AND recipient_id IS NULL
            AND work_id IS NULL AND revision_id IS NULL AND comment_id IS NULL
            AND selected IS NULL AND version IS NULL AND accepted_order IS NULL
            AND work_public IS NULL AND hidden_revision_id IS NULL AND comment_public IS NULL
        END, false)),
    -- The reachable endorsement states of LVE: ON exactly at an even version, order >= 2.
    CONSTRAINT reputation_events_tokens CHECK (kind NOT IN ('tham', 'ung') OR (
        version >= 2 AND accepted_order >= version AND selected = (version % 2 = 0)))
);

COMMENT ON TABLE reputation_events IS
    'Điểm Vink outbox (#645): accepted Thấm/Ưng changes with their context, and as-of markers for work, comment and principal facts. Immutable; the #646 consumer deletes a row when it applies it.';

CREATE INDEX reputation_events_actor_idx ON reputation_events (actor_id) WHERE actor_id IS NOT NULL;

CREATE FUNCTION reject_reputation_event_update() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'reputation events are immutable'
        USING ERRCODE = 'integrity_constraint_violation';
END;
$$;

CREATE TRIGGER reputation_events_immutable
    BEFORE UPDATE ON reputation_events
    FOR EACH STATEMENT EXECUTE FUNCTION reject_reputation_event_update();

-- The epoch a producer stamps, or NULL while inactive. Always takes the activation row FOR KEY
-- SHARE, also when inactive, so a cutover waits for this transaction.
CREATE FUNCTION reputation_producing_epoch() RETURNS bigint
LANGUAGE plpgsql AS $$
DECLARE
    is_active boolean;
    current_epoch bigint;
BEGIN
    SELECT active, epoch INTO is_active, current_epoch
      FROM reputation_activation FOR KEY SHARE;
    IF is_active THEN
        RETURN current_epoch;
    END IF;
    RETURN NULL;
END;
$$;

-- Work marker: public = a current public pointer exists; the hidden revision is not an anchor.
CREATE FUNCTION reputation_mark_work() RETURNS trigger
LANGUAGE plpgsql AS $$
DECLARE
    subject uuid;
    stamp bigint;
BEGIN
    IF TG_OP = 'DELETE' THEN
        subject := OLD.work_id;
    ELSE
        subject := NEW.work_id;
    END IF;
    stamp := reputation_producing_epoch();
    IF stamp IS NULL THEN
        RETURN NULL;
    END IF;
    INSERT INTO reputation_events (kind, activation_epoch, accepted_at, work_id, work_public,
                                   hidden_revision_id)
    SELECT 'work', stamp, now(), subject,
           EXISTS (SELECT 1 FROM publications p WHERE p.work_id = subject),
           (SELECT wp.hidden_revision_id FROM work_publications wp WHERE wp.work_id = subject);
    RETURN NULL;
END;
$$;

CREATE TRIGGER publications_reputation_marker
    AFTER INSERT OR UPDATE OR DELETE ON publications
    FOR EACH ROW EXECUTE FUNCTION reputation_mark_work();

CREATE TRIGGER work_publications_reputation_marker
    AFTER UPDATE ON work_publications
    FOR EACH ROW
    WHEN (OLD.audience IS DISTINCT FROM NEW.audience
          OR OLD.lifecycle IS DISTINCT FROM NEW.lifecycle
          OR OLD.hidden_revision_id IS DISTINCT FROM NEW.hidden_revision_id
          OR OLD.pointer_revision_id IS DISTINCT FROM NEW.pointer_revision_id)
    EXECUTE FUNCTION reputation_mark_work();

-- Comment marker: public = approved and present (Ưng also needs the work public; the consumer
-- combines it with the work facts).
CREATE FUNCTION reputation_mark_comment() RETURNS trigger
LANGUAGE plpgsql AS $$
DECLARE
    stamp bigint;
BEGIN
    stamp := reputation_producing_epoch();
    IF stamp IS NULL THEN
        RETURN NULL;
    END IF;
    INSERT INTO reputation_events (kind, activation_epoch, accepted_at, work_id, comment_id,
                                   comment_public)
    SELECT 'comment', stamp, now(), t.work_id, NEW.id,
           NEW.review = 'approved' AND NEW.removal IS NULL
      FROM discussion_threads t WHERE t.id = NEW.thread_id;
    RETURN NULL;
END;
$$;

CREATE TRIGGER comments_reputation_marker
    AFTER UPDATE ON comments
    FOR EACH ROW
    WHEN (OLD.review IS DISTINCT FROM NEW.review OR OLD.removal IS DISTINCT FROM NEW.removal)
    EXECUTE FUNCTION reputation_mark_comment();

-- Principal marker: full suspension (recipient hidden from public outputs, Q08) and account
-- deletion (DEL-05 actor terminal, recipient purge, Q20/Q21).
CREATE FUNCTION reputation_mark_principal() RETURNS trigger
LANGUAGE plpgsql AS $$
DECLARE
    stamp bigint;
BEGIN
    stamp := reputation_producing_epoch();
    IF stamp IS NULL THEN
        RETURN NULL;
    END IF;
    INSERT INTO reputation_events (kind, activation_epoch, accepted_at, principal_id,
                                   principal_suspended, principal_deleted)
    VALUES ('principal', stamp, now(), NEW.principal_id,
            EXISTS (SELECT 1 FROM principal_suspensions s
                     WHERE s.principal_id = NEW.principal_id AND s.lifted_at IS NULL),
            EXISTS (SELECT 1 FROM account_deletions d WHERE d.principal_id = NEW.principal_id));
    RETURN NULL;
END;
$$;

CREATE TRIGGER principal_suspensions_reputation_marker
    AFTER INSERT OR UPDATE ON principal_suspensions
    FOR EACH ROW EXECUTE FUNCTION reputation_mark_principal();

CREATE TRIGGER account_deletions_reputation_marker
    AFTER INSERT ON account_deletions
    FOR EACH ROW WHEN (NEW.principal_id IS NOT NULL)
    EXECUTE FUNCTION reputation_mark_principal();
