-- Recovery gate (#347 AC-06, contract §1.3 step 5): while an operator reconciles the inbox
-- projection with the App after a restore, the Gateway socket answers `503 recovering` instead
-- of a projection that may be behind or ahead of the App. Ingest stays open so the App relay's
-- replay can run. A restore brings back the gate as it was at dump time, so the runbook closes it
-- again right after `pg_restore`, before `serve` starts.

CREATE TABLE recovery_state (
    singleton  boolean PRIMARY KEY DEFAULT true CHECK (singleton),
    recovering boolean NOT NULL DEFAULT false,
    -- Advances on every `recovery begin`; `end` keeps it.
    epoch      bigint NOT NULL DEFAULT 0 CHECK (epoch >= 0),
    changed_at timestamptz NOT NULL DEFAULT now(),
    CHECK (NOT recovering OR epoch >= 1)
);

INSERT INTO recovery_state (singleton) VALUES (true);

-- Append-only audit of `vink-notifications maintenance recovery begin|end`.
CREATE TABLE recovery_changes (
    id            bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    recovering    boolean NOT NULL,
    epoch         bigint NOT NULL CHECK (epoch >= 0),
    note          text NOT NULL CHECK (char_length(note) BETWEEN 1 AND 500),
    database_role text NOT NULL DEFAULT session_user,
    changed_at    timestamptz NOT NULL DEFAULT now()
);
