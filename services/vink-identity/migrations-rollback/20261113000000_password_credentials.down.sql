-- Rollback of 20261113000000_password_credentials.

DROP INDEX IF EXISTS password_login_attempts_username_created_idx;
DROP TABLE IF EXISTS password_login_attempts;
DROP INDEX IF EXISTS account_passwords_username_idx;
DROP TABLE IF EXISTS account_passwords;
DELETE FROM _sqlx_migrations WHERE version = 20261113000000;
