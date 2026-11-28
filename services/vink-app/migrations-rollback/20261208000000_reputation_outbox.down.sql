-- Rollback of 20261208000000_reputation_outbox (App MIGRATION role, one transaction, only with
-- a binary rollback to a version without the Điểm Vink producers). Refuses while the feature is
-- active or any event is owed: events are obligations of committed source changes.
--
-- Usage: psql "$VINK_MIGRATE_DATABASE_URL" -v ON_ERROR_STOP=1 -1 -f <this file>

DO $guard$
BEGIN
    IF EXISTS (SELECT FROM reputation_activation WHERE active) THEN
        RAISE EXCEPTION 'reputation is active; deactivate it first';
    END IF;
    IF EXISTS (SELECT FROM reputation_events) THEN
        RAISE EXCEPTION 'reputation events are owed; keep this migration';
    END IF;
END
$guard$;

DROP TRIGGER account_deletions_reputation_marker ON account_deletions;
DROP TRIGGER principal_suspensions_reputation_marker ON principal_suspensions;
DROP TRIGGER comments_reputation_marker ON comments;
DROP TRIGGER work_publications_reputation_marker ON work_publications;
DROP TRIGGER publications_reputation_marker ON publications;
DROP FUNCTION reputation_mark_principal();
DROP FUNCTION reputation_mark_comment();
DROP FUNCTION reputation_mark_work();
DROP FUNCTION reputation_producing_epoch();
DROP TABLE reputation_events;
DROP FUNCTION reject_reputation_event_update();
DROP TABLE reputation_activation_changes;
DROP TABLE reputation_activation;
DELETE FROM _sqlx_migrations WHERE version = 20261208000000;
