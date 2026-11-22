-- Ưng on comments (#365, VDR-NOTIFICATIONS@r2): the writer hears of an actor's first Ưng on a
-- comment once in its lifetime (P-02B per actor + comment), and a removal before Read takes the
-- contribution away (P-03A). Same rule and horizon as `tham_marks`, keyed by comment; `work_id`
-- is kept for operator queries. The App's `first_selection` fact, not this table, carries the
-- lifetime guarantee; the mark is defense in depth for ordering and replay.

CREATE TABLE ung_marks (
    recipient    uuid NOT NULL,
    comment_id   uuid NOT NULL,
    work_id      uuid NOT NULL,
    actor_id     uuid NOT NULL,
    selected     boolean NOT NULL,
    source_order bigint NOT NULL CHECK (source_order >= 0),
    signaled     boolean NOT NULL,
    updated_at   timestamptz NOT NULL,
    expires_at   timestamptz NOT NULL,
    PRIMARY KEY (recipient, comment_id, actor_id)
);
CREATE INDEX ung_marks_expiry ON ung_marks (expires_at);

-- The new type joins the closed sets of receipts and groups.
ALTER TABLE ingestion_receipts DROP CONSTRAINT ingestion_receipts_event_type_check;
ALTER TABLE ingestion_receipts ADD CONSTRAINT ingestion_receipts_event_type_check
    CHECK (event_type IN (
        'comment.root.approved', 'comment.reply.approved', 'work.moderation.decided',
        'comment.moderation.adverse', 'work.endorsement.tham.changed',
        'comment.endorsement.ung.changed', 'notification.invalidation'));
ALTER TABLE notification_groups DROP CONSTRAINT notification_groups_event_type_check;
ALTER TABLE notification_groups ADD CONSTRAINT notification_groups_event_type_check
    CHECK (event_type IN (
        'comment.root.approved', 'comment.reply.approved', 'work.moderation.decided',
        'comment.moderation.adverse', 'work.endorsement.tham.changed',
        'comment.endorsement.ung.changed'));
