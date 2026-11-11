-- Credential enrollment (ACC-03): a signed-in email-code account adds a username/password to the
-- SAME account. The proof is a fresh one-time code mailed to the account's own verified address,
-- so it reuses the challenge and delivery tables with one more purpose; no table is added and no
-- credential system runs beside `account_passwords`.
--
-- `email_challenges_purpose_account` already requires an account for every purpose other than
-- `sign_in`, so an enrollment challenge is always bound to the account that asked for it.

ALTER TABLE email_challenges DROP CONSTRAINT email_challenges_purpose_check;
ALTER TABLE email_challenges ADD CONSTRAINT email_challenges_purpose_check
    CHECK (purpose IN ('sign_in', 'change_email', 'enroll_password'));

ALTER TABLE email_challenges ADD COLUMN enrollment_session_id uuid
    REFERENCES sessions (id) ON DELETE CASCADE;
ALTER TABLE email_challenges ADD CONSTRAINT email_challenges_enrollment_session_check
    CHECK ((purpose = 'enroll_password') = (enrollment_session_id IS NOT NULL));
CREATE INDEX email_challenges_enrollment_session_idx ON email_challenges (enrollment_session_id)
    WHERE enrollment_session_id IS NOT NULL;

ALTER TABLE mail_deliveries DROP CONSTRAINT mail_deliveries_purpose_check;
ALTER TABLE mail_deliveries ADD CONSTRAINT mail_deliveries_purpose_check
    CHECK (purpose IN ('sign_in', 'change_email', 'enroll_password'));
