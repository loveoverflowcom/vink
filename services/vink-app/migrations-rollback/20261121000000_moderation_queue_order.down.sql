-- Stop the #205 server before backout. Only a derived index is removed; no queue/audit data.
BEGIN;
DROP INDEX moderation_cases_work_queue_order;
DELETE FROM _sqlx_migrations WHERE version = 20261121000000;
COMMIT;
