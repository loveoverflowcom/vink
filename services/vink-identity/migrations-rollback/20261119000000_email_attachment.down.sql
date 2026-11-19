-- Run with the Identity migration role in one transaction, alongside a rollback to an older
-- binary. Delete only attach-specific proof/mail/re-auth attempts. Keep verified bindings.
-- Usage: psql "$VINK_IDENTITY_MIGRATE_DATABASE_URL" -v ON_ERROR_STOP=1 -1 -f <this file>
DROP TABLE email_attachment_reauth_attempts;
DELETE FROM email_challenges WHERE purpose = 'attach_email';
ALTER TABLE email_challenges DROP COLUMN attachment_session_id, DROP COLUMN attachment_password_version;
ALTER TABLE email_challenges DROP CONSTRAINT email_challenges_purpose_check;
ALTER TABLE email_challenges ADD CONSTRAINT email_challenges_purpose_check
    CHECK (purpose IN ('sign_in', 'change_email', 'enroll_password'));
ALTER TABLE mail_deliveries DROP CONSTRAINT mail_deliveries_purpose_check;
ALTER TABLE mail_deliveries ADD CONSTRAINT mail_deliveries_purpose_check
    CHECK (purpose IN ('sign_in', 'change_email', 'enroll_password'));

DELETE FROM _sqlx_migrations WHERE version = 20261119000000;
