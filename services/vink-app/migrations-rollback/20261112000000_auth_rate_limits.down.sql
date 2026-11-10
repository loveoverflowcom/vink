-- Rollback of 20261112000000_auth_rate_limits (App MIGRATION role, one transaction, only with a
-- binary rollback to a version that does not embed it). Loses only rate-limit counters; no
-- principal, binding, suspension, work or comment is touched.
--
-- Usage: psql "$VINK_MIGRATE_DATABASE_URL" -v ON_ERROR_STOP=1 -1 -f <this file>
-- `services/vink-app/tests/postgres/abuse.rs` runs this file on real PostgreSQL.

DROP TABLE IF EXISTS auth_rate_limits;
DELETE FROM _sqlx_migrations WHERE version = 20261112000000;
