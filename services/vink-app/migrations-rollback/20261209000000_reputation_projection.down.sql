-- Rollback of 20261209000000_reputation_projection (App MIGRATION role, one transaction, only
-- with a binary rollback to a version without the Điểm Vink consumer). The projection is derived
-- from the sources and can be rebuilt by a later backfill; the correction audit is the only
-- record that cannot, so the guard refuses while any correction exists or the feature is active.
--
-- Usage: psql "$VINK_MIGRATE_DATABASE_URL" -v ON_ERROR_STOP=1 -1 -f <this file>

DO $guard$
BEGIN
    IF EXISTS (SELECT FROM reputation_activation WHERE active) THEN
        RAISE EXCEPTION 'reputation is active; deactivate it first';
    END IF;
    IF EXISTS (SELECT FROM reputation_corrections) THEN
        RAISE EXCEPTION 'reputation corrections exist; keep this migration';
    END IF;
END
$guard$;

DROP TABLE reputation_corrections;
DROP FUNCTION reject_reputation_correction_change();
DROP TABLE reputation_ledger;
DROP TABLE reputation_profiles;
DROP TABLE reputation_entitlements;
DROP TABLE reputation_anchors;
DROP TABLE reputation_principal_facts;
DROP TABLE reputation_comment_facts;
DROP TABLE reputation_work_facts;
DROP TABLE reputation_readiness;
DELETE FROM _sqlx_migrations WHERE version = 20261209000000;
