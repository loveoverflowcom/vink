-- #365 (VDR-NOTIFICATIONS@r2): Ưng on a comment owes `comment.endorsement.ung.changed` to the
-- comment's writer, produced by Endorsement in the same transaction as the accepted change.
-- Widens the closed set of event types; the envelope rules stay those of #346.

ALTER TABLE notification_events DROP CONSTRAINT notification_events_event_type_check;
ALTER TABLE notification_events ADD CONSTRAINT notification_events_event_type_check
    CHECK (event_type IN (
        'comment.root.approved', 'comment.reply.approved', 'work.moderation.decided',
        'comment.moderation.adverse', 'work.endorsement.tham.changed',
        'comment.endorsement.ung.changed', 'notification.invalidation'));
