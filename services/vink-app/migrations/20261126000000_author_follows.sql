-- Social Follow Graph (#294): CHOICE-02 A (retained pair state + append-only receipts)
-- and CHOICE-03 A (index-backed live aggregates over active edges).
-- Every edge is keyed by two distinct author profiles (PrincipalId). Accepted order advances
-- on every accepted command (including same-state intents) to prevent ABA replays; version
-- advances only when active flips, so active == ((version % 2) = 1).

CREATE TABLE author_follows (
    follower_id uuid NOT NULL REFERENCES author_profiles (principal_id),
    followee_id uuid NOT NULL REFERENCES author_profiles (principal_id),
    active boolean NOT NULL,
    version bigint NOT NULL,
    accepted_order bigint NOT NULL,
    active_since timestamptz,
    cursor_key uuid NOT NULL DEFAULT gen_random_uuid(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT author_follows_pkey PRIMARY KEY (follower_id, followee_id),
    CONSTRAINT author_follows_no_self CHECK (follower_id <> followee_id),
    CONSTRAINT author_follows_cursor_key_key UNIQUE (cursor_key),
    CONSTRAINT author_follows_tokens_check CHECK (
        version >= 0
        AND accepted_order >= 1
        AND version <= accepted_order
        AND active = ((version % 2) = 1)
        AND active = (active_since IS NOT NULL)
    )
);

CREATE INDEX author_follows_followers_active_idx
    ON author_follows (followee_id, active_since DESC, cursor_key ASC)
    WHERE active;

CREATE INDEX author_follows_following_active_idx
    ON author_follows (follower_id, active_since DESC, cursor_key ASC)
    WHERE active;

CREATE TABLE author_follow_receipts (
    actor_id uuid NOT NULL REFERENCES author_profiles (principal_id),
    command_id uuid NOT NULL,
    target_id uuid NOT NULL REFERENCES author_profiles (principal_id),
    expected_order bigint NOT NULL CHECK (expected_order >= 0),
    desired_following boolean NOT NULL,
    committed_following boolean NOT NULL,
    committed_version bigint NOT NULL,
    committed_order bigint NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT author_follow_receipts_pkey PRIMARY KEY (actor_id, command_id),
    CONSTRAINT author_follow_receipts_no_self CHECK (actor_id <> target_id),
    CONSTRAINT author_follow_receipts_committed_check CHECK (
        committed_order = expected_order + 1
        AND committed_following = desired_following
        AND committed_version >= 0
        AND committed_version <= committed_order
        AND committed_following = ((committed_version % 2) = 1)
    )
);

CREATE FUNCTION guard_author_follow_update() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.follower_id IS DISTINCT FROM OLD.follower_id
        OR NEW.followee_id IS DISTINCT FROM OLD.followee_id
        OR NEW.cursor_key IS DISTINCT FROM OLD.cursor_key THEN
        RAISE EXCEPTION 'author_follows row identity is immutable'
            USING ERRCODE = 'integrity_constraint_violation';
    END IF;
    IF NEW.accepted_order <= OLD.accepted_order THEN
        RAISE EXCEPTION 'author_follows accepted_order must advance (% -> %)',
            OLD.accepted_order, NEW.accepted_order
            USING ERRCODE = 'integrity_constraint_violation';
    END IF;
    IF NEW.version < OLD.version THEN
        RAISE EXCEPTION 'author_follows version cannot decrease (% -> %)',
            OLD.version, NEW.version
            USING ERRCODE = 'integrity_constraint_violation';
    END IF;
    RETURN NEW;
END;
$$;

CREATE FUNCTION reject_author_follow_removal() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION '% rows are retained', TG_TABLE_NAME
        USING ERRCODE = 'integrity_constraint_violation';
END;
$$;

CREATE TRIGGER author_follows_guard_update
    BEFORE UPDATE ON author_follows
    FOR EACH ROW EXECUTE FUNCTION guard_author_follow_update();

CREATE TRIGGER author_follows_retained
    BEFORE DELETE OR TRUNCATE ON author_follows
    FOR EACH STATEMENT EXECUTE FUNCTION reject_author_follow_removal();

CREATE TRIGGER author_follow_receipts_append_only
    BEFORE UPDATE OR DELETE OR TRUNCATE ON author_follow_receipts
    FOR EACH STATEMENT EXECUTE FUNCTION reject_author_follow_removal();
