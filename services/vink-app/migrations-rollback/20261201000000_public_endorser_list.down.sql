-- Rollback of 20261201000000_public_endorser_list (App MIGRATION role, one transaction, only
-- with a binary rollback to a version that does not embed it). Positions are derived data: the
-- public Thấm list is the only reader, and dropping them loses no endorsement.
--
-- Usage: psql "$VINK_MIGRATE_DATABASE_URL" -v ON_ERROR_STOP=1 -1 -f <this file>

DROP INDEX work_endorsements_public_list_idx;
ALTER TABLE work_endorsements DROP CONSTRAINT work_endorsements_cursor_key_key;
ALTER TABLE work_endorsements DROP COLUMN cursor_key;
DELETE FROM _sqlx_migrations WHERE version = 20261201000000;
