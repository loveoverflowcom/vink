-- First verified email on an existing password account. Reuse email challenges and durable mail.
ALTER TABLE email_challenges DROP CONSTRAINT email_challenges_purpose_check;
ALTER TABLE email_challenges ADD CONSTRAINT email_challenges_purpose_check
    CHECK (purpose IN ('sign_in', 'change_email', 'enroll_password', 'attach_email'));
ALTER TABLE mail_deliveries DROP CONSTRAINT mail_deliveries_purpose_check;
ALTER TABLE mail_deliveries ADD CONSTRAINT mail_deliveries_purpose_check
    CHECK (purpose IN ('sign_in', 'change_email', 'enroll_password', 'attach_email'));

ALTER TABLE email_challenges ADD COLUMN attachment_session_id uuid
    REFERENCES sessions (id) ON DELETE CASCADE;
ALTER TABLE email_challenges ADD COLUMN attachment_password_version integer;
ALTER TABLE email_challenges ADD CONSTRAINT email_challenges_attachment_session_check
    CHECK ((purpose = 'attach_email') = (attachment_session_id IS NOT NULL));
ALTER TABLE email_challenges ADD CONSTRAINT email_challenges_attachment_version_check
    CHECK ((purpose = 'attach_email') = (attachment_password_version IS NOT NULL)
           AND attachment_password_version >= 1);
CREATE INDEX email_challenges_attachment_session_idx ON email_challenges (attachment_session_id)
    WHERE attachment_session_id IS NOT NULL;

-- Count all password rechecks, even when a request disconnects during Argon2 verification.
-- Independent from anonymous sign-in's per-username counter.
CREATE TABLE email_attachment_reauth_attempts (
    id uuid PRIMARY KEY,
    account_id uuid NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp()
);
CREATE INDEX email_attachment_reauth_attempts_account_created_idx
    ON email_attachment_reauth_attempts (account_id, created_at);
