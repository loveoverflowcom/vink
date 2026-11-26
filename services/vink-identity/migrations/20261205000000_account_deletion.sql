ALTER TABLE email_challenges DROP CONSTRAINT email_challenges_purpose_check;
ALTER TABLE email_challenges DROP CONSTRAINT email_challenges_purpose_account;
ALTER TABLE email_challenges ADD CONSTRAINT email_challenges_purpose_check
    CHECK (purpose IN ('sign_in', 'change_email', 'enroll_password', 'attach_email', 'delete_account'));
-- A decoy challenge cannot retire a subject.
ALTER TABLE email_challenges ADD CONSTRAINT email_challenges_purpose_account CHECK (
    (purpose = 'sign_in' AND account_id IS NULL) OR purpose = 'delete_account'
    OR (purpose IN ('change_email', 'enroll_password', 'attach_email') AND account_id IS NOT NULL)
);
ALTER TABLE mail_deliveries DROP CONSTRAINT mail_deliveries_purpose_check;
ALTER TABLE mail_deliveries ADD CONSTRAINT mail_deliveries_purpose_check
    CHECK (purpose IN ('sign_in', 'change_email', 'enroll_password', 'attach_email', 'delete_account'));

-- The permanent receipt is also the outbox for App. It stores no reusable proof or credential.
CREATE TABLE account_deletions (
    account_id uuid PRIMARY KEY REFERENCES accounts(id),
    operation_id uuid NOT NULL UNIQUE,
    accepted_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    applied_at timestamptz
);
CREATE INDEX account_deletions_pending ON account_deletions (operation_id)
    WHERE applied_at IS NULL;

CREATE FUNCTION guard_account_deletion() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'UPDATE' AND NEW.account_id = OLD.account_id
       AND NEW.operation_id = OLD.operation_id AND NEW.accepted_at = OLD.accepted_at
       AND (OLD.applied_at IS NULL OR NEW.applied_at IS NOT DISTINCT FROM OLD.applied_at) THEN
        RETURN NEW;
    END IF;
    RAISE EXCEPTION 'account deletion is permanent' USING ERRCODE = 'integrity_constraint_violation';
END;
$$;
CREATE TRIGGER guard_account_deletion BEFORE UPDATE OR DELETE ON account_deletions
    FOR EACH ROW EXECUTE FUNCTION guard_account_deletion();

CREATE FUNCTION refuse_retired_account_enable() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.disabled_at IS NULL AND EXISTS (
        SELECT 1 FROM account_deletions WHERE account_id = NEW.id
    ) THEN
        RAISE EXCEPTION 'retired account cannot be enabled' USING ERRCODE = 'integrity_constraint_violation';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER refuse_retired_account_enable BEFORE UPDATE OF disabled_at ON accounts
    FOR EACH ROW EXECUTE FUNCTION refuse_retired_account_enable();
