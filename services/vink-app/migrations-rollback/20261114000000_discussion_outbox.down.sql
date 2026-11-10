-- Rollback of 20261114000000_discussion_outbox (App MIGRATION role, one transaction, only with a
-- binary rollback to a version that does not embed it). A binary-only backout is NOT enough: an
-- older binary sees this unknown migration as a diverged schema and stays not ready
-- (docs/architecture/discussion-outbox.md §6). Stop the new App, let the new worker drain (or
-- finish dead letters), stop it, run this script, then start the older binaries.
--
-- It REFUSES while any delivery is ready, leased or dead, so no owed work and no dead letter is
-- dropped silently: drain the queue (or requeue and finish dead letters) first. When it runs it
-- drops finished events and deliveries, the requeue audit and the shadow observations
-- (measurement-only data, never a review decision). Comments, receipts and moderation are not
-- touched.
--
-- Usage: psql "$VINK_MIGRATE_DATABASE_URL" -v ON_ERROR_STOP=1 -1 -f <this file>
-- `services/vink-app/tests/postgres/outbox.rs` runs this file on real PostgreSQL.

-- Block concurrent producers and workers first, so no delivery commits between the check and the
-- drop (a new-binary App still serving waits here, then fails its submission and rolls back).
LOCK TABLE discussion_events, discussion_deliveries IN ACCESS EXCLUSIVE MODE;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM discussion_deliveries WHERE state IN ('ready', 'leased', 'dead')
    ) THEN
        RAISE EXCEPTION 'discussion outbox still owes work (ready, leased or dead deliveries); drain it before rolling back';
    END IF;
END $$;

DROP TABLE IF EXISTS comment_admission_observations;
DROP TABLE IF EXISTS discussion_delivery_requeues;
DROP TABLE IF EXISTS discussion_deliveries;
DROP TABLE IF EXISTS discussion_events;
DROP FUNCTION IF EXISTS reject_delivery_requeue_change();
DROP FUNCTION IF EXISTS reject_discussion_event_change();
DELETE FROM _sqlx_migrations WHERE version = 20261114000000;
