-- Rollback of 20261113000000_own_comment_overlay (App MIGRATION role, one transaction, only with a
-- binary rollback to a version that does not embed it). Drops one index; no row is touched. The
-- overlay route keeps working without it, at the cost of a table scan per page.
--
-- Usage: psql "$VINK_MIGRATE_DATABASE_URL" -v ON_ERROR_STOP=1 -1 -f <this file>
-- `services/vink-app/tests/postgres/discussion_reads.rs` runs this file on real PostgreSQL.

DROP INDEX IF EXISTS comments_own_unpublished_idx;
DELETE FROM _sqlx_migrations WHERE version = 20261113000000;
