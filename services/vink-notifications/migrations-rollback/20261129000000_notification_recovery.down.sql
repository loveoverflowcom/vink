-- Backout of 20261129000000_notification_recovery (run by the migration role on a copy first).
-- Refuses while the gate is closed: dropping it would reopen an unverified projection.

DO $guard$
BEGIN
    IF EXISTS (SELECT FROM recovery_state WHERE recovering) THEN
        RAISE EXCEPTION 'recovery is in progress; end it before backout';
    END IF;
END
$guard$;

DROP TABLE recovery_changes;
DROP TABLE recovery_state;
DELETE FROM _sqlx_migrations WHERE version = 20261129000000;
