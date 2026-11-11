-- Native password credentials (ACC-03, O-12).
-- Identity owns credentials for username/password sign-in and registration;
-- username is case-insensitive ASCII, password_hash is an Argon2id PHC string.
-- The account_id is the subject id of authority vink-identity;
-- passwords never leave Identity, and App receives only the opaque session token.

CREATE TABLE account_passwords (
    account_id uuid PRIMARY KEY REFERENCES accounts (id),
    username text NOT NULL CONSTRAINT account_passwords_username_key UNIQUE
        CHECK (username = lower(username) AND octet_length(username) BETWEEN 3 AND 30
               AND username ~ '^[a-z0-9_.]+$'),
    password_hash text NOT NULL,
    version integer NOT NULL DEFAULT 1 CHECK (version >= 1),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX account_passwords_username_idx ON account_passwords (username);

COMMENT ON TABLE account_passwords IS
    'Username and Argon2id PHC password hash for native Identity accounts. The account_id is the subject id; username is case-insensitive ASCII, never public.';

CREATE TABLE password_login_attempts (
    id uuid PRIMARY KEY,
    username text NOT NULL CHECK (username = lower(username)),
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX password_login_attempts_username_created_idx
    ON password_login_attempts (username, created_at);

COMMENT ON TABLE password_login_attempts IS
    'Recent failed login attempts per username for throttling. Pruned after retention.';
