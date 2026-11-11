-- Rollback of 20261114000000_password_enrollment (run by an operator with the Identity MIGRATION
-- role, in one transaction, only together with a binary rollback to a version that does not embed
-- it).
--
-- Loses only transient data: enrollment challenges (and, by cascade, their deliveries); a person
-- asks for a new code. Enrolled hashes and usernames stay in `account_passwords`, which belongs
-- to 20261113000000. An older binary still applies its own password RPC input bounds.
--
-- Usage: psql "$VINK_IDENTITY_MIGRATE_DATABASE_URL" -v ON_ERROR_STOP=1 -1 -f <this file>
-- `services/vink-identity/tests/postgres/enrollment.rs` runs this file on real PostgreSQL.

DELETE FROM email_challenges WHERE purpose = 'enroll_password';

ALTER TABLE email_challenges DROP COLUMN enrollment_session_id;

ALTER TABLE mail_deliveries DROP CONSTRAINT mail_deliveries_purpose_check;
ALTER TABLE mail_deliveries ADD CONSTRAINT mail_deliveries_purpose_check
    CHECK (purpose IN ('sign_in', 'change_email'));

ALTER TABLE email_challenges DROP CONSTRAINT email_challenges_purpose_check;
ALTER TABLE email_challenges ADD CONSTRAINT email_challenges_purpose_check
    CHECK (purpose IN ('sign_in', 'change_email'));

DELETE FROM _sqlx_migrations WHERE version = 20261114000000;
