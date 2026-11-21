-- Backout of 20261128000000_notification_inbox (run by the migration role on a copy first).
-- Refuses while any inbox group is still within its retention: dropping it would lose inbox
-- history the product keeps for up to 180 days. Stop the relay first (App deliveries stay
-- queued and are re-sent after a forward migration; receipts are rebuilt by those deliveries).

DO $guard$
BEGIN
    IF EXISTS (SELECT FROM notification_groups WHERE expires_at > now()) THEN
        RAISE EXCEPTION 'notification groups within retention exist; purge or keep them before backout';
    END IF;
END
$guard$;

DROP TABLE inbox_commands;
DROP TABLE comment_fences;
DROP TABLE tham_marks;
DROP TABLE group_members;
DROP TABLE notification_groups;
DROP TABLE ingestion_receipts;
DROP TABLE inboxes;
DELETE FROM _sqlx_migrations WHERE version = 20261128000000;
