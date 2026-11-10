-- Rollback of 20261112000000_mail_outbox (run by an operator with the Identity MIGRATION role,
-- in one transaction, only together with a binary rollback to a version that does not embed it).
--
-- Loses only transient data: pending and finished deliveries (codes in flight are worthless
-- after rollback; people ask for a new code). Accounts, verified addresses, challenges and
-- sessions are untouched, so nobody is signed out and no account is lost.
--
-- Usage: psql "$VINK_IDENTITY_MIGRATE_DATABASE_URL" -v ON_ERROR_STOP=1 -1 -f <this file>
-- `services/vink-identity/tests/postgres/delivery.rs` runs this file on real PostgreSQL.

DROP INDEX IF EXISTS sessions_closed_idx;
DROP INDEX IF EXISTS email_challenges_account_created_idx;
DROP INDEX IF EXISTS email_challenges_created_idx;
DROP TABLE IF EXISTS mail_deliveries;
DELETE FROM _sqlx_migrations WHERE version = 20261112000000;
