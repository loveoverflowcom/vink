-- Notifications v1 producer outbox and relay deliveries (#346; Epic #305 R3; contract
-- `contracts/http/v1/notifications.md` §1). Rules live in `domain::notification`; this schema
-- holds the shapes no decision may break.
--
-- Producer: the business transaction that commits a transition (comment moderation, the
-- writer's own removal, an operator hold, a work decision, a cleared public pointer, a Thấm
-- change, a standing change) writes its envelope and the one relay delivery in that SAME
-- transaction, after reading `notification_activation` under FOR KEY SHARE. A rolled-back or
-- no-op transition owes nothing; an enqueue error rolls the transition back. No network call
-- happens there.
--
-- Activation (T-01 cutover): producers write only while active and stamp the epoch they read.
-- `vink-app notifications activate|deactivate` updates the row FOR UPDATE, so a transition
-- commits either wholly before or wholly after a cutover. Nothing is backfilled. The row starts
-- inactive: no event is written until an operator activates the feature with a receiver ready.
--
-- Relay (T-02): one short transaction claims due deliveries (FOR UPDATE SKIP LOCKED, generation
-- + 1, lease) and commits; the envelope is sent outside any App transaction; the receiver's
-- answer is recorded in a new transaction that locks the delivery and writes only if it is still
-- leased under the claimed generation (fenced ACK). Delivery is at least once; the receiver's
-- receipt (event_id + payload_digest) makes a duplicate one effect.
--
-- Backout: an older binary reports this unknown migration as a diverged schema and is not
-- ready. `migrations-rollback/20261127000000_notification_outbox.down.sql` refuses while any
-- delivery is ready, leased or dead, so it never discards owed work.

CREATE TABLE notification_activation (
    singleton boolean PRIMARY KEY DEFAULT true CHECK (singleton),
    active boolean NOT NULL DEFAULT false,
    -- Advances on every activation; deactivation keeps it. Never 0 while active.
    epoch bigint NOT NULL DEFAULT 0 CHECK (epoch >= 0),
    changed_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT notification_activation_epoch CHECK (NOT active OR epoch >= 1)
);

INSERT INTO notification_activation (singleton) VALUES (true);

COMMENT ON TABLE notification_activation IS
    'Notifications v1 cutover (#346). Producers read it FOR KEY SHARE in their own transaction and write events only while active, stamped with this epoch. Starts inactive.';

CREATE TABLE notification_activation_changes (
    id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    active boolean NOT NULL,
    epoch bigint NOT NULL CHECK (epoch >= 0),
    note text NOT NULL CHECK (char_length(note) BETWEEN 1 AND 500),
    database_role text NOT NULL DEFAULT session_user,
    changed_at timestamptz NOT NULL DEFAULT now()
);

COMMENT ON TABLE notification_activation_changes IS
    'Append-only audit of `vink-app notifications activate|deactivate`. The note names the operator and the reason.';

CREATE TABLE notification_events (
    id uuid PRIMARY KEY,
    event_type text NOT NULL CHECK (event_type IN (
        'comment.root.approved', 'comment.reply.approved', 'work.moderation.decided',
        'comment.moderation.adverse', 'work.endorsement.tham.changed',
        'notification.invalidation')),
    schema_version smallint NOT NULL CHECK (schema_version = 1),
    source_owner text NOT NULL CHECK (source_owner IN (
        'discussion', 'publication_moderation', 'endorsement', 'principal')),
    -- The logical event (type, scope, source order[, Thấm actor]): one obligation per
    -- transition even if a producer ran twice.
    dedup_key text NOT NULL UNIQUE CHECK (dedup_key ~ '^[a-z][a-z0-9_./-]{0,255}$'),
    activation_epoch bigint NOT NULL CHECK (activation_epoch >= 1),
    -- The contract's canonical envelope without `payload_digest`, written once by the producer
    -- and sent verbatim (plus the digest) on every attempt and replay.
    envelope text NOT NULL CHECK (left(envelope, 1) = '{'),
    payload_digest text NOT NULL CHECK (payload_digest ~ '^[0-9a-f]{64}$'),
    created_at timestamptz NOT NULL DEFAULT now()
);

COMMENT ON TABLE notification_events IS
    'Immutable Notifications v1 envelopes (#346). Identifiers, versions, codes and booleans only: no body, email, display name, moderator identity, session or reason.';

CREATE FUNCTION reject_notification_event_change() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'notification events are immutable'
        USING ERRCODE = 'integrity_constraint_violation';
END;
$$;

CREATE TRIGGER notification_events_immutable
    BEFORE UPDATE ON notification_events
    FOR EACH ROW EXECUTE FUNCTION reject_notification_event_change();

CREATE TABLE notification_deliveries (
    event_id uuid PRIMARY KEY REFERENCES notification_events (id) ON DELETE CASCADE,
    state text NOT NULL DEFAULT 'ready'
        CHECK (state IN ('ready', 'leased', 'succeeded', 'dead')),
    attempts integer NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    -- The claim fence: advances by one on every claim; never reset (not by requeue or replay).
    generation bigint NOT NULL DEFAULT 0 CHECK (generation >= 0),
    available_at timestamptz NOT NULL DEFAULT now(),
    deadline_from timestamptz NOT NULL DEFAULT now(),
    expired_leases integer NOT NULL DEFAULT 0 CHECK (expired_leases >= 0),
    outcome text CHECK (outcome ~ '^[a-z][a-z_]{0,47}$'),
    last_error text CHECK (last_error ~ '^[a-z][a-z_]{0,47}$'),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    finished_at timestamptz,
    CONSTRAINT notification_deliveries_finished
        CHECK ((state IN ('succeeded', 'dead')) = (finished_at IS NOT NULL)),
    CONSTRAINT notification_deliveries_outcome
        CHECK ((state IN ('succeeded', 'dead')) = (outcome IS NOT NULL)),
    CONSTRAINT notification_deliveries_claimed_when_leased
        CHECK (state <> 'leased' OR (generation >= 1 AND attempts >= 1))
);

CREATE INDEX notification_deliveries_claimable_idx ON notification_deliveries (available_at)
    WHERE state IN ('ready', 'leased');
CREATE INDEX notification_deliveries_dead_idx ON notification_deliveries (finished_at)
    WHERE state = 'dead';
CREATE INDEX notification_events_created_idx ON notification_events (created_at);

COMMENT ON TABLE notification_deliveries IS
    'What each notification event owes the Notification Service (#346). At least once; acknowledged only under the claim generation fence.';

CREATE TABLE notification_delivery_requeues (
    id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    event_id uuid NOT NULL,
    -- requeue: a dead letter after a fix; replay: an acknowledged event re-sent after a
    -- receiver restore (contract §1.3 step 5).
    kind text NOT NULL CHECK (kind IN ('requeue', 'replay')),
    previous_outcome text NOT NULL,
    previous_attempts integer NOT NULL,
    note text NOT NULL CHECK (char_length(note) BETWEEN 1 AND 500),
    database_role text NOT NULL DEFAULT session_user,
    requeued_at timestamptz NOT NULL DEFAULT now()
);

COMMENT ON TABLE notification_delivery_requeues IS
    'Append-only audit of `vink-app notifications requeue|replay`. Outlives the event it names.';

CREATE FUNCTION reject_notification_audit_change() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'notification operator audit is append-only'
        USING ERRCODE = 'integrity_constraint_violation';
END;
$$;

CREATE TRIGGER notification_delivery_requeues_append_only
    BEFORE UPDATE OR DELETE ON notification_delivery_requeues
    FOR EACH ROW EXECUTE FUNCTION reject_notification_audit_change();

CREATE TRIGGER notification_activation_changes_append_only
    BEFORE UPDATE OR DELETE ON notification_activation_changes
    FOR EACH ROW EXECUTE FUNCTION reject_notification_audit_change();
