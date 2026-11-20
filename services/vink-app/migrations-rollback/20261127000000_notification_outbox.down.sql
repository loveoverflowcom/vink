-- Rollback of 20261127000000_notification_outbox (App MIGRATION role, one transaction, only with
-- a binary rollback to a version that does not embed it). A binary-only backout is NOT enough:
-- an older binary sees this unknown migration as a diverged schema and stays not ready. Stop the
-- new App and worker (or deactivate first: `vink-app notifications deactivate <note>`), run this
-- script, then start the older binaries.
--
-- It REFUSES while any notification delivery is ready, leased or dead, so no owed delivery and
-- no dead letter is dropped silently: let the relay drain (or requeue and finish dead letters)
-- first. When it runs it drops acknowledged events and deliveries and the operator audits.
-- Business rows (comments, decisions, endorsements, publications, suspensions) are not touched.
--
-- Usage: psql "$VINK_MIGRATE_DATABASE_URL" -v ON_ERROR_STOP=1 -1 -f <this file>
-- `services/vink-app/tests/postgres/notification.rs` runs this file on real PostgreSQL.

-- Block producers (they read the activation row) and relays before the check.
LOCK TABLE notification_activation, notification_events, notification_deliveries
    IN ACCESS EXCLUSIVE MODE;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM notification_deliveries WHERE state IN ('ready', 'leased', 'dead')
    ) THEN
        RAISE EXCEPTION 'notification outbox still owes work (ready, leased or dead deliveries); drain it before rolling back';
    END IF;
END $$;

DROP TABLE IF EXISTS notification_delivery_requeues;
DROP TABLE IF EXISTS notification_deliveries;
DROP TABLE IF EXISTS notification_events;
DROP TABLE IF EXISTS notification_activation_changes;
DROP TABLE IF EXISTS notification_activation;
DROP FUNCTION IF EXISTS reject_notification_audit_change();
DROP FUNCTION IF EXISTS reject_notification_event_change();
DELETE FROM _sqlx_migrations WHERE version = 20261127000000;
