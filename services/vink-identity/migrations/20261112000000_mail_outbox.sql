-- Durable mail delivery for the email-code authority (D-05 production path).
--
-- A challenge and its delivery intent are written in the SAME transaction; no network call runs
-- inside it. The delivery worker of `vink-identity serve` claims due rows with
-- FOR UPDATE SKIP LOCKED and a lease, sends outside any transaction and records the outcome only
-- while it still holds that lease (lease_token). Delivery is at-least-once: a worker that crashes
-- after the provider accepted a message, or loses its lease, can cause one duplicate of the same
-- code, which is harmless (the code still needs the client-held challenge secret).
--
-- The code is kept in clear only while the delivery is pending, because the worker must put it
-- in the mail; it is cleared in the same statement that records a terminal state. A pending code
-- alone cannot answer its challenge: the stored verifier also needs the secret the client holds.
-- Nothing here holds a session credential or a challenge secret.

CREATE TABLE mail_deliveries (
    challenge_id uuid PRIMARY KEY REFERENCES email_challenges (id) ON DELETE CASCADE,
    purpose text NOT NULL CHECK (purpose IN ('sign_in', 'change_email')),
    recipient text NOT NULL
        CHECK (recipient = lower(recipient) AND octet_length(recipient) BETWEEN 3 AND 254),
    code text CHECK (code ~ '^[0-9]{6}$'),
    state text NOT NULL DEFAULT 'pending'
        CHECK (state IN ('pending', 'sent', 'failed', 'cancelled')),
    attempts integer NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    next_attempt_at timestamptz NOT NULL DEFAULT now(),
    lease_token uuid,
    -- A category, never provider text (which can quote the recipient).
    last_error text CHECK (last_error IN
        ('timeout', 'connect', 'tls', 'transient', 'permanent', 'configuration', 'expired')),
    last_status smallint CHECK (last_status BETWEEN 200 AND 599),
    created_at timestamptz NOT NULL DEFAULT now(),
    finished_at timestamptz,
    CONSTRAINT mail_deliveries_code_while_pending CHECK ((state = 'pending') = (code IS NOT NULL)),
    CONSTRAINT mail_deliveries_finished CHECK ((state = 'pending') = (finished_at IS NULL)),
    CONSTRAINT mail_deliveries_lease_while_pending CHECK (state = 'pending' OR lease_token IS NULL)
);

CREATE INDEX mail_deliveries_due_idx ON mail_deliveries (next_attempt_at) WHERE state = 'pending';

COMMENT ON TABLE mail_deliveries IS
    'Transactional outbox of one-time codes. One row per challenge; code present only while pending; delivered at least once by the serve worker.';

-- Throttle reads that are not per address: the global issuance ceiling (all addresses) and the
-- per-account ceiling of verified email changes.
CREATE INDEX email_challenges_created_idx ON email_challenges (created_at);
CREATE INDEX email_challenges_account_created_idx ON email_challenges (account_id, created_at)
    WHERE account_id IS NOT NULL;

-- Retention (`vink-identity maintenance prune`, also hourly in `serve`): challenges, and with them
-- their deliveries, are deleted 7 days after expiry; sessions 30 days after they closed (expiry or
-- revocation, whichever is later). Accounts and verified addresses are never deleted. This index
-- serves the session prune only; verification looks sessions up by token_hash.
CREATE INDEX sessions_closed_idx ON sessions (GREATEST(expires_at, COALESCE(revoked_at, expires_at)));
