-- Offline cutover. Inventory and retire invalid subjects with Identity maintenance first.
-- A remaining invalid account aborts this transaction before any feature data is removed.
LOCK TABLE accounts, account_emails, account_deletions IN ACCESS EXCLUSIVE MODE;
DO $$ BEGIN
    IF EXISTS (SELECT FROM account_deletions WHERE applied_at IS NULL) THEN
        RAISE EXCEPTION 'Drain pending App deletion receipts before verified-email migration';
    END IF;
END $$;
ALTER TABLE accounts ADD COLUMN retired boolean NOT NULL DEFAULT false;
UPDATE accounts SET retired = true WHERE id IN (SELECT account_id FROM account_deletions);
ALTER TABLE accounts
    ADD COLUMN live_subject uuid GENERATED ALWAYS AS (CASE WHEN NOT retired THEN id END) STORED UNIQUE,
    ADD COLUMN retired_subject uuid GENERATED ALWAYS AS (CASE WHEN retired THEN id END) STORED UNIQUE,
    ADD CHECK (NOT retired OR disabled_at IS NOT NULL),
    ADD FOREIGN KEY (live_subject) REFERENCES account_emails(account_id) DEFERRABLE INITIALLY DEFERRED,
    ADD FOREIGN KEY (retired_subject) REFERENCES account_deletions(account_id) DEFERRABLE INITIALLY DEFERRED;
ALTER TABLE account_emails ADD FOREIGN KEY (account_id) REFERENCES accounts(live_subject) DEFERRABLE INITIALLY DEFERRED;
ALTER TABLE account_deletions ADD FOREIGN KEY (account_id) REFERENCES accounts(retired_subject) DEFERRABLE INITIALLY DEFERRED;

-- Match the UTF-8 mailbox and label bounds of EmailAddress::parse. Avoid locale-dependent
-- POSIX classes for Unicode White_Space and Cc.
ALTER TABLE account_emails ADD CONSTRAINT account_emails_canonical CHECK (
    address IS NFC NORMALIZED AND address ~ '^[^@]+@[^@]+$'
    AND octet_length(split_part(address, '@', 1)) BETWEEN 1 AND 64
    AND split_part(address, '@', 2) ~ '^[^.-]([^.]*[^.-])?([.][^.-]([^.]*[^.-])?)+$'
    AND address !~ U&'[\0001-\0020\007f-\00a0\1680\2000-\200a\2028\2029\202f\205f\3000]'
);

CREATE FUNCTION mark_retired_email_subject() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    UPDATE accounts SET retired = true, disabled_at = COALESCE(disabled_at, now()) WHERE id = NEW.account_id;
    RETURN NEW;
END;
$$;
CREATE TRIGGER mark_retired_email_subject AFTER INSERT ON account_deletions
    FOR EACH ROW EXECUTE FUNCTION mark_retired_email_subject();

DELETE FROM email_challenges WHERE purpose = 'attach_email';
ALTER TABLE email_challenges
    DROP COLUMN attachment_session_id, DROP COLUMN attachment_password_version,
    DROP CONSTRAINT email_challenges_purpose_check, DROP CONSTRAINT email_challenges_purpose_account,
    ADD CHECK (purpose IN ('sign_in', 'register_password', 'change_email', 'enroll_password', 'delete_account')),
    ADD CHECK (purpose = 'delete_account' OR
        ((purpose IN ('sign_in', 'register_password')) = (account_id IS NULL)));
ALTER TABLE mail_deliveries DROP CONSTRAINT mail_deliveries_purpose_check,
    ADD CHECK (purpose IN ('sign_in', 'register_password', 'change_email', 'enroll_password', 'delete_account'));
DROP TABLE email_attachment_reauth_attempts;
