-- Every name ever issued is bound permanently to the same verified principal.
-- Current/alias/tombstone derive from one pointer, including after multiple renames.
CREATE TABLE author_handle_names (
    handle text COLLATE "C" PRIMARY KEY CHECK (
        length(handle) BETWEEN 1 AND 40 AND handle ~ '^[a-z0-9]+(-[a-z0-9]+)*$'
        AND handle NOT IN ('admin','api','vink','support','moderator','toi','ban-viet','kiem-duyet')
    ),
    principal_id uuid NOT NULL REFERENCES principal_bindings(principal_id),
    UNIQUE (principal_id, handle)
);
CREATE FUNCTION keep_author_handle_name() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'issued author names are permanent' USING ERRCODE = 'integrity_constraint_violation';
END;
$$;
CREATE TRIGGER keep_author_handle_name BEFORE UPDATE OR DELETE ON author_handle_names
    FOR EACH ROW EXECUTE FUNCTION keep_author_handle_name();

CREATE TABLE author_profiles (
    principal_id uuid PRIMARY KEY REFERENCES principal_bindings(principal_id),
    -- NULL is the terminal profile tombstone. Reservations survive account deletion.
    current_handle text COLLATE "C",
    setup_complete boolean NOT NULL DEFAULT false,
    remaining_changes smallint NOT NULL DEFAULT 2 CHECK (remaining_changes BETWEEN 0 AND 2),
    version bigint NOT NULL DEFAULT 0 CHECK (version >= 0),
    CHECK (setup_complete OR remaining_changes = 2),
    FOREIGN KEY (principal_id, current_handle) REFERENCES author_handle_names(principal_id, handle)
);
CREATE TRIGGER keep_retired_profile BEFORE UPDATE ON author_profiles
    FOR EACH ROW WHEN (OLD.current_handle IS NULL) EXECUTE FUNCTION keep_author_handle_name();
CREATE TRIGGER keep_profile_history BEFORE DELETE ON author_profiles
    FOR EACH ROW EXECUTE FUNCTION keep_author_handle_name();

CREATE TABLE author_handle_receipts (
    principal_id uuid NOT NULL REFERENCES author_profiles(principal_id),
    command_id uuid NOT NULL,
    expected_version bigint NOT NULL,
    intent text NOT NULL CHECK (intent IN ('setup','rename')),
    requested_handle text NOT NULL,
    accepted_version bigint NOT NULL,
    remaining_changes smallint NOT NULL,
    PRIMARY KEY (principal_id, command_id)
);

-- Existing verified bindings must be reconciled before publishing any address.
CREATE TABLE author_handle_rollout (
    singleton boolean PRIMARY KEY CHECK (singleton),
    ready boolean NOT NULL
);
INSERT INTO author_handle_rollout VALUES (true, NOT EXISTS (SELECT FROM principal_bindings));

-- A mixed-version writer can still provision an account after schema expansion.
-- At commit, close the read gate only if it did not also create the profile.
CREATE FUNCTION check_profile_rollout() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NOT EXISTS (SELECT FROM author_profiles WHERE principal_id = NEW.principal_id) THEN
        UPDATE author_handle_rollout SET ready = false WHERE singleton;
    END IF;
    RETURN NULL;
END;
$$;
CREATE CONSTRAINT TRIGGER check_profile_rollout AFTER INSERT ON principal_bindings
    DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION check_profile_rollout();
