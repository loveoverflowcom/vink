-- Notifications v1 (#347): ingestion receipts, write-time rollup, Seen/Read and retention in the
-- Notification Service's own database (K1). Nothing here refers to the App (`vink`) or Identity
-- (`vink_identity`) databases; recipients and actors are opaque principal IDs from the envelope.
--
-- T-03 CHOICE-17 B: every write to an inbox serializes on its `inboxes` row (one lock covers the
-- receipt's effect, Read, Seen and expiry for that recipient) and the partial unique index keeps
-- at most one open group per key as a constraint, not only as a convention.
-- T-05 CHOICE-19 A: `last_seq` advances under that lock, so sequence order is commit order.
-- T-08 CHOICE-22 A: plain tables and indexes, purged in batches by expiry.

CREATE TABLE inboxes (
    recipient  uuid PRIMARY KEY,
    -- The sequence of this inbox; snapshots and versions are values of it.
    last_seq   bigint NOT NULL DEFAULT 0 CHECK (last_seq >= 0),
    -- Seen watermark (P-09A): groups whose newest signal is at or below it are seen.
    seen_seq   bigint NOT NULL DEFAULT 0 CHECK (seen_seq >= 0),
    created_at timestamptz NOT NULL,
    updated_at timestamptz NOT NULL,
    CHECK (seen_seq <= last_seq)
);

-- One row per event ID ever accepted, applied or not (contract §1.3). Written in the same
-- transaction as the event's effect.
CREATE TABLE ingestion_receipts (
    event_id       uuid PRIMARY KEY,
    payload_digest text NOT NULL CHECK (payload_digest ~ '^[0-9a-f]{64}$'),
    event_type     text NOT NULL CHECK (event_type IN (
        'comment.root.approved', 'comment.reply.approved', 'work.moderation.decided',
        'comment.moderation.adverse', 'work.endorsement.tham.changed', 'notification.invalidation')),
    -- signal: a new signal; reconcile: membership/marks only; stale: older than what is known;
    -- late: past the late cutoff (reconciled, never signalled); noted: no stored effect.
    effect         text NOT NULL CHECK (effect IN ('signal', 'reconcile', 'stale', 'late', 'noted')),
    occurred_at    timestamptz NOT NULL,
    received_at    timestamptz NOT NULL,
    expires_at     timestamptz NOT NULL CHECK (expires_at > received_at)
);
CREATE INDEX ingestion_receipts_expiry ON ingestion_receipts (expires_at);

CREATE TABLE notification_groups (
    group_id          uuid PRIMARY KEY,
    recipient         uuid NOT NULL REFERENCES inboxes (recipient) ON DELETE CASCADE,
    event_type        text NOT NULL CHECK (event_type IN (
        'comment.root.approved', 'comment.reply.approved', 'work.moderation.decided',
        'comment.moderation.adverse', 'work.endorsement.tham.changed')),
    target_key        text NOT NULL CHECK (char_length(target_key) BETWEEN 1 AND 200),
    -- The typed target of the newest contribution, as received (contract §2.4).
    target            jsonb NOT NULL,
    target_order      bigint NOT NULL,
    work_id           uuid,
    -- COM-11 stamp of the newest signal, compared by the Gateway with the current epoch.
    publication_epoch bigint,
    action_required   boolean NOT NULL DEFAULT false,
    -- The one unread, non-empty group still accepting contributions for its key.
    open              boolean NOT NULL,
    created_at        timestamptz NOT NULL,
    expires_at        timestamptz NOT NULL,
    last_signal_at    timestamptz NOT NULL,
    group_version     bigint NOT NULL CHECK (group_version >= 1),
    signal_seq        bigint NOT NULL CHECK (signal_seq >= 1),
    -- The sequence of the group's first signal: MarkAllRead(S) touches only groups that
    -- existed at S.
    first_seq         bigint NOT NULL CHECK (first_seq >= 1),
    read_through      bigint NOT NULL DEFAULT 0 CHECK (read_through >= 0),
    actor_count       integer NOT NULL DEFAULT 0 CHECK (actor_count >= 0),
    item_count        integer NOT NULL DEFAULT 0 CHECK (item_count >= 0),
    actors_preview    uuid[] NOT NULL DEFAULT '{}' CHECK (cardinality(actors_preview) <= 3),
    CHECK (first_seq <= signal_seq AND signal_seq <= group_version AND read_through <= group_version),
    -- P-06A: no group outlives 180 days from its creation.
    CHECK (expires_at <= created_at + interval '180 days'),
    CHECK (NOT open OR (signal_seq > read_through AND actor_count + item_count > 0))
);
CREATE UNIQUE INDEX notification_groups_one_open
    ON notification_groups (recipient, event_type, target_key) WHERE open;
CREATE INDEX notification_groups_inbox
    ON notification_groups (recipient, signal_seq DESC, group_id);
CREATE INDEX notification_groups_expiry ON notification_groups (expires_at);

-- Who contributed to an actor-counted group, one row per contribution (approved comment, or the
-- revision for a Thấm). Deactivated, not deleted, so a silent relike can restore it.
CREATE TABLE group_members (
    group_id     uuid NOT NULL REFERENCES notification_groups (group_id) ON DELETE CASCADE,
    contribution uuid NOT NULL,
    actor_id     uuid NOT NULL,
    active       boolean NOT NULL,
    source_order bigint NOT NULL,
    last_seq     bigint NOT NULL,
    PRIMARY KEY (group_id, contribution, actor_id)
);
CREATE INDEX group_members_contribution ON group_members (contribution) WHERE active;

-- P-02B/P-03A (T-04 CHOICE-18 A): the latest desired Thấm state of one actor on one revision,
-- per recipient, ordered by the source's accepted_order. Kept independently of inbox groups.
CREATE TABLE tham_marks (
    recipient    uuid NOT NULL,
    work_id      uuid NOT NULL,
    revision_id  uuid NOT NULL,
    actor_id     uuid NOT NULL,
    selected     boolean NOT NULL,
    source_order bigint NOT NULL CHECK (source_order >= 0),
    signaled     boolean NOT NULL,
    updated_at   timestamptz NOT NULL,
    expires_at   timestamptz NOT NULL,
    PRIMARY KEY (recipient, work_id, revision_id, actor_id)
);
CREATE INDEX tham_marks_expiry ON tham_marks (expires_at);

-- The newest invalidation of a comment, so an older approval delivered late cannot restore it.
CREATE TABLE comment_fences (
    comment_id   uuid PRIMARY KEY,
    source_order bigint NOT NULL CHECK (source_order >= 0),
    expires_at   timestamptz NOT NULL
);
CREATE INDEX comment_fences_expiry ON comment_fences (expires_at);

-- Idempotent Seen/Read/MarkAllRead (contract §2.2): the answer of a command, replayed for the
-- same key and request, refused for the same key and another request.
CREATE TABLE inbox_commands (
    recipient   uuid NOT NULL,
    command_key text NOT NULL CHECK (char_length(command_key) BETWEEN 1 AND 128),
    request     text NOT NULL,
    response    jsonb NOT NULL,
    created_at  timestamptz NOT NULL,
    expires_at  timestamptz NOT NULL,
    PRIMARY KEY (recipient, command_key)
);
CREATE INDEX inbox_commands_expiry ON inbox_commands (expires_at);
