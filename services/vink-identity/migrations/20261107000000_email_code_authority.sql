-- Native email-code authority (D-05/S-03 spike). Identity owns accounts, verified addresses,
-- one-time challenges and sessions. None of these tables is a Vink principal: the App binds
-- (authority, subject) to its own principal and owns every permission, suspension and work.
-- Secrets are stored only as SHA-256 verifiers; codes, challenge secrets and session
-- credentials never reach this database in clear.

CREATE TABLE accounts (
    id uuid PRIMARY KEY,
    created_at timestamptz NOT NULL DEFAULT now(),
    disabled_at timestamptz
);

COMMENT ON TABLE accounts IS
    'Subject of authority vink-identity. The id is never reused or derived from an address; disabling closes every session.';

CREATE TABLE account_emails (
    account_id uuid PRIMARY KEY REFERENCES accounts (id),
    address text NOT NULL CONSTRAINT account_emails_address_key UNIQUE
        CHECK (address = lower(address) AND octet_length(address) BETWEEN 3 AND 254),
    verified_at timestamptz NOT NULL
);

COMMENT ON TABLE account_emails IS
    'The one verified address of an account (pilot). Only a verified change replaces it; a freed address can later prove a different account and never inherits the old subject.';

CREATE TABLE email_challenges (
    id uuid PRIMARY KEY,
    purpose text NOT NULL CHECK (purpose IN ('sign_in', 'change_email')),
    address text NOT NULL CHECK (address = lower(address) AND octet_length(address) BETWEEN 3 AND 254),
    account_id uuid REFERENCES accounts (id),
    code_hash bytea NOT NULL CHECK (octet_length(code_hash) = 32),
    failed_attempts integer NOT NULL DEFAULT 0 CHECK (failed_attempts >= 0),
    created_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL,
    consumed_at timestamptz,
    CONSTRAINT email_challenges_purpose_account CHECK ((purpose = 'sign_in') = (account_id IS NULL)),
    CONSTRAINT email_challenges_expiry CHECK (expires_at > created_at)
);

CREATE INDEX email_challenges_address_created_idx ON email_challenges (address, created_at);

COMMENT ON TABLE email_challenges IS
    'One-time code challenges. code_hash = SHA-256(domain, client-held secret, code): a row alone cannot answer its challenge. Consumed at most once.';

CREATE TABLE sessions (
    id uuid PRIMARY KEY,
    account_id uuid NOT NULL REFERENCES accounts (id),
    token_hash bytea NOT NULL CONSTRAINT sessions_token_hash_key UNIQUE
        CHECK (octet_length(token_hash) = 32),
    created_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL,
    revoked_at timestamptz,
    revoked_reason text CHECK (revoked_reason IN ('logout', 'account_disabled', 'email_changed')),
    CONSTRAINT sessions_expiry CHECK (expires_at > created_at),
    CONSTRAINT sessions_revocation CHECK ((revoked_at IS NULL) = (revoked_reason IS NULL))
);

CREATE INDEX sessions_active_account_idx ON sessions (account_id) WHERE revoked_at IS NULL;

COMMENT ON TABLE sessions IS
    'Opaque server-side sessions, verified on every protected App request. Revocation is a row update: it takes effect on the next verification, with no token lifetime to wait out.';
