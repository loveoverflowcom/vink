-- Điểm Vink projection (#646; VDR-REPUTATION-CONTRACT@r1 K4..K7): the consumer of
-- `reputation_events` (#645) keeps as-of facts, one row per entitlement with its two Q41 credit
-- machines, one projection row per recipient, a per-entitlement period ledger and correction
-- audit. Rules live in `domain::reputation`; this schema holds the shapes no decision may break.
--
-- Readiness: `vink-app reputation backfill` writes the projection from one REPEATABLE READ
-- snapshot of the sources while producers are active, deletes the events that snapshot already
-- reflects, and sets `ready_at` and each calendar's first full period (Q33). Before that, reads
-- are unavailable and the consumer waits. No history before activation is invented.
--
-- Consumer (`vink-worker`): one batch at a time (transaction advisory lock), events in id order,
-- each applied and deleted in the same transaction. Maturity (Q42) is applied as of the threshold
-- before any later event of the same actor.
--
-- Privacy: identifiers, counters and states only; no text from sources. Recipient deletion
-- purges the recipient's projection and ledger rows (Q21); correction audit keeps the operator's
-- note, never a source body.
--
-- Backout: `migrations-rollback/20261209000000_reputation_projection.down.sql` drops these
-- tables (a derived projection) only while `reputation_activation` is inactive.

CREATE TABLE reputation_readiness (
    singleton boolean PRIMARY KEY DEFAULT true CHECK (singleton),
    ready_at timestamptz,
    week_start bigint,
    month_start bigint,
    formula_version integer NOT NULL DEFAULT 1 CHECK (formula_version = 1),
    CONSTRAINT reputation_readiness_complete CHECK (
        (ready_at IS NULL) = (week_start IS NULL) AND (ready_at IS NULL) = (month_start IS NULL))
);

INSERT INTO reputation_readiness (singleton) VALUES (true);

CREATE TABLE reputation_work_facts (
    work_id uuid PRIMARY KEY,
    work_public boolean NOT NULL,
    hidden_revision_id uuid
);

CREATE TABLE reputation_comment_facts (
    comment_id uuid PRIMARY KEY,
    work_id uuid NOT NULL,
    comment_public boolean NOT NULL
);

CREATE TABLE reputation_principal_facts (
    principal_id uuid PRIMARY KEY,
    -- Set once maturity (created_at + 7 × 24h) has been applied to this actor.
    matured_at timestamptz,
    suspended boolean NOT NULL DEFAULT false,
    deleted boolean NOT NULL DEFAULT false
);

-- Thấm anchors as of the processed events: one row per (actor, work, revision), with the
-- source order that fences stale or replayed events.
CREATE TABLE reputation_anchors (
    actor_id uuid NOT NULL,
    work_id uuid NOT NULL,
    revision_id uuid NOT NULL,
    selected boolean NOT NULL,
    accepted_order bigint NOT NULL CHECK (accepted_order >= 1),
    PRIMARY KEY (actor_id, work_id, revision_id)
);

CREATE TABLE reputation_entitlements (
    component text NOT NULL CHECK (component IN ('impact', 'contribution')),
    actor_id uuid NOT NULL,
    -- WorkId for Impact, CommentId for Contribution.
    target_id uuid NOT NULL,
    work_id uuid NOT NULL,
    recipient_id uuid NOT NULL,
    -- Contribution only: the Ưng selection and its source order (the comment is the anchor).
    anchor_selected boolean NOT NULL DEFAULT false,
    anchor_order bigint NOT NULL DEFAULT 1 CHECK (anchor_order >= 1),
    effective boolean NOT NULL,
    terminal boolean NOT NULL,
    week_state text NOT NULL,
    week_pending bigint,
    week_last bigint,
    month_state text NOT NULL,
    month_pending bigint,
    month_last bigint,
    PRIMARY KEY (component, actor_id, target_id),
    CONSTRAINT reputation_entitlements_terminal CHECK (NOT (terminal AND effective)),
    CONSTRAINT reputation_entitlements_week CHECK (
        week_state IN ('unstarted', 'unseen', 'active', 'withdrawn', 'burned')
        AND (week_state = 'withdrawn') = (week_pending IS NOT NULL)),
    CONSTRAINT reputation_entitlements_month CHECK (
        month_state IN ('unstarted', 'unseen', 'active', 'withdrawn', 'burned')
        AND (month_state = 'withdrawn') = (month_pending IS NOT NULL))
);

CREATE INDEX reputation_entitlements_work_idx ON reputation_entitlements (work_id);
CREATE INDEX reputation_entitlements_actor_idx ON reputation_entitlements (actor_id);
CREATE INDEX reputation_entitlements_recipient_idx ON reputation_entitlements (recipient_id);

CREATE TABLE reputation_profiles (
    principal_id uuid PRIMARY KEY,
    impact bigint NOT NULL CHECK (impact >= 0 AND impact < 1099511627776),
    contribution bigint NOT NULL CHECK (contribution >= 0 AND contribution < 1099511627776),
    score_micros bigint NOT NULL CHECK (score_micros >= 1000000000),
    formula_version integer NOT NULL CHECK (formula_version = 1),
    updated_at timestamptz NOT NULL DEFAULT now()
);

-- Top Rating candidates (Q17): recognized members only, canonical order.
CREATE INDEX reputation_profiles_rank_idx ON reputation_profiles (score_micros DESC, principal_id)
    WHERE impact > 0 OR contribution > 0;

-- Net community credit of one entitlement in one period (Q41: -1, 0 or +1). Minimized
-- actor-recipient ledger (Q40); weights apply when aggregating (Q28).
CREATE TABLE reputation_ledger (
    calendar text NOT NULL CHECK (calendar IN ('week', 'month')),
    period bigint NOT NULL,
    component text NOT NULL CHECK (component IN ('impact', 'contribution')),
    actor_id uuid NOT NULL,
    target_id uuid NOT NULL,
    recipient_id uuid NOT NULL,
    net smallint NOT NULL CHECK (net BETWEEN -1 AND 1),
    PRIMARY KEY (calendar, period, component, actor_id, target_id)
);

CREATE INDEX reputation_ledger_recipient_idx ON reputation_ledger (calendar, period, recipient_id);

CREATE TABLE reputation_corrections (
    id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    idempotency_key text NOT NULL UNIQUE CHECK (char_length(idempotency_key) BETWEEN 1 AND 100),
    scope text NOT NULL CHECK (char_length(scope) BETWEEN 1 AND 200),
    note text NOT NULL CHECK (char_length(note) BETWEEN 1 AND 500),
    entitlements integer NOT NULL CHECK (entitlements >= 0),
    ledger_rows integer NOT NULL CHECK (ledger_rows >= 0),
    database_role text NOT NULL DEFAULT session_user,
    applied_at timestamptz NOT NULL DEFAULT now()
);

COMMENT ON TABLE reputation_corrections IS
    'Append-only audit of `vink-app reputation correct apply` (Q38): scope, operator note with evidence and reason, effect counts. Source counts are never changed.';

CREATE FUNCTION reject_reputation_correction_change() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'reputation corrections are append-only'
        USING ERRCODE = 'integrity_constraint_violation';
END;
$$;

CREATE TRIGGER reputation_corrections_append_only
    BEFORE UPDATE OR DELETE OR TRUNCATE ON reputation_corrections
    FOR EACH STATEMENT EXECUTE FUNCTION reject_reputation_correction_change();
