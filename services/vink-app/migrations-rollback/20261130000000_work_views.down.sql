-- Rollback of 20261130000000_work_views (App MIGRATION role, one transaction, only with a binary
-- rollback to a version that does not embed it). A binary-only backout is NOT enough: an older
-- binary sees this unknown migration as a diverged schema and stays not ready. Stop the new App
-- and worker, run this script, then start the older binaries.
--
-- It REFUSES once any view was counted, so no author's total is dropped silently. Windows and
-- receipts alone (all expire within 24 hours, hold no total) do not block it.
--
-- Usage: psql "$VINK_MIGRATE_DATABASE_URL" -v ON_ERROR_STOP=1 -1 -f <this file>
-- `services/vink-app/tests/postgres/work_view.rs` runs this file on real PostgreSQL.

LOCK TABLE work_view_totals, work_view_days, work_view_windows, work_view_receipts,
    work_view_counting IN ACCESS EXCLUSIVE MODE;

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM work_view_totals) OR EXISTS (SELECT 1 FROM work_view_days) THEN
        RAISE EXCEPTION 'work views were already counted; refusing to drop authors'' totals';
    END IF;
END $$;

DROP TABLE IF EXISTS work_view_counting;
DROP TABLE IF EXISTS work_view_receipts;
DROP TABLE IF EXISTS work_view_windows;
DROP TABLE IF EXISTS work_view_days;
DROP TABLE IF EXISTS work_view_totals;
DROP FUNCTION IF EXISTS reject_work_view_rewrite();
DROP FUNCTION IF EXISTS guard_work_view_window();
DROP FUNCTION IF EXISTS guard_work_view_day();
DROP FUNCTION IF EXISTS guard_work_view_total();
DELETE FROM _sqlx_migrations WHERE version = 20261130000000;
