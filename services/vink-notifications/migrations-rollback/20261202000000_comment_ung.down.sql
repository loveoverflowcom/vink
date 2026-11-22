-- Backout of 20261202000000_comment_ung (run by the migration role on a copy first). Refuses
-- while an Ưng group is still within retention: dropping the marks would let a relike after the
-- backout signal again (P-02B).

DO $guard$
BEGIN
    IF EXISTS (
        SELECT FROM notification_groups
        WHERE event_type = 'comment.endorsement.ung.changed' AND expires_at > now()
    ) THEN
        RAISE EXCEPTION 'Ưng groups are within retention; purge them before backout';
    END IF;
END
$guard$;

-- Expired Ưng groups and receipts would violate the narrower checks; they carry no live
-- obligation (receipts of applied Ưng events only guard replays of an undone feature).
DELETE FROM notification_groups WHERE event_type = 'comment.endorsement.ung.changed';
DELETE FROM ingestion_receipts WHERE event_type = 'comment.endorsement.ung.changed';
ALTER TABLE ingestion_receipts DROP CONSTRAINT ingestion_receipts_event_type_check;
ALTER TABLE ingestion_receipts ADD CONSTRAINT ingestion_receipts_event_type_check
    CHECK (event_type IN (
        'comment.root.approved', 'comment.reply.approved', 'work.moderation.decided',
        'comment.moderation.adverse', 'work.endorsement.tham.changed', 'notification.invalidation'));
ALTER TABLE notification_groups DROP CONSTRAINT notification_groups_event_type_check;
ALTER TABLE notification_groups ADD CONSTRAINT notification_groups_event_type_check
    CHECK (event_type IN (
        'comment.root.approved', 'comment.reply.approved', 'work.moderation.decided',
        'comment.moderation.adverse', 'work.endorsement.tham.changed'));
DROP TABLE ung_marks;
DELETE FROM _sqlx_migrations WHERE version = 20261202000000;
