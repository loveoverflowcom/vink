-- Offline rollback only: stop workers and drain/expire pending deliveries before downgrade.
ALTER TABLE mail_deliveries DROP COLUMN payload_fingerprint;

DELETE FROM _sqlx_migrations WHERE version = 20261120000000;
