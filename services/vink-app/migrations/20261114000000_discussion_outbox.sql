-- Discussion outbox (DISC-04, #120): durable events, one delivery per consumer, and the first
-- consumer's own receipt table. Lifecycle rules live in `domain::discussion::outbox`; the schema
-- holds the shapes that no decision may break.
--
-- Producer: `PgStore::submit_comment` writes the comment, its `comment.submitted` event and the
-- event's deliveries in ONE transaction, so a committed comment always owes its consumers and a
-- rolled-back one owes nothing. No network call happens in that transaction.
--
-- Worker (`vink-worker`): a short transaction claims due deliveries (FOR UPDATE SKIP LOCKED,
-- generation + 1, lease), commits, and a second transaction locks the delivery row, checks that
-- it is still leased under the claimed generation, applies the effect and records the outcome
-- before committing. A stale worker therefore writes neither an effect nor an outcome.
--
-- Backout: an older binary reports this unknown migration as a diverged schema and is not ready,
-- so a backout needs the schema rollback
-- (`migrations-rollback/20261114000000_discussion_outbox.down.sql`). That script locks both queue
-- tables and refuses while any delivery is ready, leased or dead, so it never discards owed work.

CREATE TABLE discussion_events (
    id uuid PRIMARY KEY,
    -- Free text so that a newer binary's types reach an older worker as "unsupported" (dead
    -- letter with a reason) instead of failing the producer's insert.
    event_type text NOT NULL CHECK (event_type ~ '^[a-z][a-z0-9_.]{0,62}$'),
    schema_version smallint NOT NULL CHECK (schema_version >= 1),
    comment_id uuid NOT NULL REFERENCES comments (id),
    -- The comment's moderation_version that the producing transition left.
    source_version bigint NOT NULL CHECK (source_version >= 1),
    created_at timestamptz NOT NULL DEFAULT now(),
    -- The logical event: one per transition of one comment at one version.
    CONSTRAINT discussion_events_identity UNIQUE (event_type, comment_id, source_version)
);

COMMENT ON TABLE discussion_events IS
    'Immutable discussion events (DISC-04). Identifiers and versions only: no body, author, email, session or reason. Pruned only after every delivery is terminal and retained long enough.';

-- Events never change after commit; retention may delete them (and their deliveries).
CREATE FUNCTION reject_discussion_event_change() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'discussion events are immutable'
        USING ERRCODE = 'integrity_constraint_violation';
END;
$$;

CREATE TRIGGER discussion_events_immutable
    BEFORE UPDATE ON discussion_events
    FOR EACH ROW EXECUTE FUNCTION reject_discussion_event_change();

CREATE TABLE discussion_deliveries (
    event_id uuid NOT NULL REFERENCES discussion_events (id) ON DELETE CASCADE,
    consumer text NOT NULL CHECK (consumer ~ '^[a-z][a-z0-9_.]{0,62}$'),
    state text NOT NULL DEFAULT 'ready'
        CHECK (state IN ('ready', 'leased', 'succeeded', 'skipped', 'dead')),
    -- Claims so far, counted when claimed (a crash or lost lease spends one).
    attempts integer NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    -- The claim fence: advances by one on every claim; never reset, not even by a requeue.
    generation bigint NOT NULL DEFAULT 0 CHECK (generation >= 0),
    -- ready: not claimable before; leased: the lease expiry.
    available_at timestamptz NOT NULL DEFAULT now(),
    -- The deadline counts from here: the event's creation, or the last operator requeue.
    deadline_from timestamptz NOT NULL DEFAULT now(),
    -- Leases that expired without a result and were reclaimed (or dead-lettered).
    expired_leases integer NOT NULL DEFAULT 0 CHECK (expired_leases >= 0),
    -- Terminal outcome code; never provider text, a body or an identifier.
    outcome text CHECK (outcome ~ '^[a-z][a-z_]{0,47}$'),
    last_error text CHECK (last_error ~ '^[a-z][a-z_]{0,47}$'),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    finished_at timestamptz,
    PRIMARY KEY (event_id, consumer),
    CONSTRAINT discussion_deliveries_finished
        CHECK ((state IN ('succeeded', 'skipped', 'dead')) = (finished_at IS NOT NULL)),
    CONSTRAINT discussion_deliveries_outcome
        CHECK ((state IN ('succeeded', 'skipped', 'dead')) = (outcome IS NOT NULL)),
    CONSTRAINT discussion_deliveries_claimed_when_leased
        CHECK (state <> 'leased' OR (generation >= 1 AND attempts >= 1))
);

-- The claim scan: ready rows that are due and leases that expired, oldest first.
CREATE INDEX discussion_deliveries_claimable_idx ON discussion_deliveries (available_at)
    WHERE state IN ('ready', 'leased');
-- Operator listing of dead letters.
CREATE INDEX discussion_deliveries_dead_idx ON discussion_deliveries (finished_at)
    WHERE state = 'dead';
-- Retention: events whose deliveries all ended, oldest first.
CREATE INDEX discussion_events_created_idx ON discussion_events (created_at);

COMMENT ON TABLE discussion_deliveries IS
    'What each discussion event owes one consumer (DISC-04). At least once per delivery; an App-database effect commits atomically with its outcome under the claim generation fence.';

CREATE TABLE discussion_delivery_requeues (
    id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    event_id uuid NOT NULL,
    consumer text NOT NULL,
    previous_outcome text NOT NULL,
    previous_attempts integer NOT NULL,
    note text NOT NULL CHECK (char_length(note) BETWEEN 1 AND 500),
    database_role text NOT NULL DEFAULT session_user,
    requeued_at timestamptz NOT NULL DEFAULT now()
);

COMMENT ON TABLE discussion_delivery_requeues IS
    'Append-only audit of operator requeues (`vink-app outbox requeue`). Outlives the event it names. database_role is the shared operator credential; the note names the operator and reason.';

CREATE FUNCTION reject_delivery_requeue_change() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'outbox requeue audit is append-only'
        USING ERRCODE = 'integrity_constraint_violation';
END;
$$;

CREATE TRIGGER discussion_delivery_requeues_append_only
    BEFORE UPDATE OR DELETE ON discussion_delivery_requeues
    FOR EACH ROW EXECUTE FUNCTION reject_delivery_requeue_change();

-- First consumer: what the publish-first candidate would have decided, for measurement only
-- (D-08 delta condition 7). Never read by a public or viewer path and never written back to
-- `comments`. One row per comment and candidate is the effect's receipt: it outlives event
-- retention, so a requeue or a re-emitted event cannot record the observation twice.
CREATE TABLE comment_admission_observations (
    comment_id uuid NOT NULL REFERENCES comments (id),
    candidate_policy text NOT NULL CHECK (candidate_policy IN ('trusted-cohort.v1')),
    -- Provenance only (events are pruned, so no foreign key).
    event_id uuid NOT NULL,
    outcome text NOT NULL CHECK (outcome IN ('pending', 'approved', 'refused')),
    reason text NOT NULL CHECK (reason IN (
        'policy_premoderation', 'thread_reviewed', 'content_flagged', 'trust_unknown',
        'not_trusted', 'trust_revoked', 'trust_stale', 'content_unassessed', 'content_mismatch',
        'content_stale', 'trusted_and_clear', 'suspended', 'standing_unknown')),
    -- The comment's effective state when observed (a human decision may already exist).
    observed_version bigint NOT NULL CHECK (observed_version >= 1),
    observed_review text NOT NULL CHECK (observed_review IN ('pending', 'approved', 'rejected')),
    observed_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (comment_id, candidate_policy),
    CONSTRAINT comment_admission_observations_outcome_reason CHECK (
        (outcome = 'refused') = (reason IN ('suspended', 'standing_unknown'))
        AND (outcome = 'approved') = (reason = 'trusted_and_clear'))
);

COMMENT ON TABLE comment_admission_observations IS
    'Shadow-only admission observations (DISC-04 consumer admission_shadow.v1). Not a review decision; no route reads it.';
