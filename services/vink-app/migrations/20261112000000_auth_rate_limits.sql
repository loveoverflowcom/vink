-- Rate limits of the sign-in edge (D-05). The App is the process that sees the client network,
-- so it owns these counters; Identity keeps its own per-address, per-account and global
-- ceilings. One row per (bucket, network key, fixed window); a request increments its row with
-- one atomic upsert, so every App process shares the same count.
--
-- network_key = SHA-256(domain, bucket, IPv4 address or IPv6 /64): pseudonymised, never the
-- address. Rows are deleted one day after their window ends (`vink-app maintenance prune`,
-- hourly in `vink-worker`).

CREATE TABLE auth_rate_limits (
    bucket text NOT NULL CHECK (bucket ~ '^[a-z_.]{1,64}$'),
    network_key bytea NOT NULL CHECK (octet_length(network_key) = 32),
    window_start timestamptz NOT NULL,
    window_seconds integer NOT NULL CHECK (window_seconds BETWEEN 1 AND 86400),
    hits integer NOT NULL CHECK (hits >= 1),
    CONSTRAINT auth_rate_limits_pkey PRIMARY KEY (bucket, network_key, window_start)
);

CREATE INDEX auth_rate_limits_window_idx ON auth_rate_limits (window_start);

COMMENT ON TABLE auth_rate_limits IS
    'Fixed-window request counters per client network for sign-in routes. Pseudonymised keys; pruned a day after the window.';
