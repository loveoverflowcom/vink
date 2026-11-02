-- Test-only registry: proves the lifecycle with a caller-supplied migrator, not a Vink schema.
CREATE TABLE notes (id bigint PRIMARY KEY, body text NOT NULL);
