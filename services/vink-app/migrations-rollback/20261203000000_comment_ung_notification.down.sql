-- Rollback of 20261203000000_comment_ung_notification (App MIGRATION role, one transaction,
-- only with a binary rollback to a version that does not produce Ưng events). Refuses while an
-- Ưng event exists: events are immutable obligations and the narrower check would reject them.
--
-- Usage: psql "$VINK_MIGRATE_DATABASE_URL" -v ON_ERROR_STOP=1 -1 -f <this file>

DO $guard$
BEGIN
    IF EXISTS (SELECT FROM notification_events
               WHERE event_type = 'comment.endorsement.ung.changed') THEN
        RAISE EXCEPTION 'Ưng notification events exist; keep this migration';
    END IF;
END
$guard$;

ALTER TABLE notification_events DROP CONSTRAINT notification_events_event_type_check;
ALTER TABLE notification_events ADD CONSTRAINT notification_events_event_type_check
    CHECK (event_type IN (
        'comment.root.approved', 'comment.reply.approved', 'work.moderation.decided',
        'comment.moderation.adverse', 'work.endorsement.tham.changed',
        'notification.invalidation'));
DELETE FROM _sqlx_migrations WHERE version = 20261203000000;
