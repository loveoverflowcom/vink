-- Vink literary core, bootstrap slice.
-- Working drafts (mutable, owner-only, optimistic version) stay separate from immutable revision
-- snapshots and from the public pointer. No moderation, publish intent or withdrawal tables yet:
-- no command writes publications; only the local development seed creates the reader fixture.

CREATE TABLE principals (
    id uuid PRIMARY KEY,
    display_name text NOT NULL CHECK (char_length(display_name) BETWEEN 1 AND 80),
    created_at timestamptz NOT NULL DEFAULT now()
);

COMMENT ON TABLE principals IS
    'Stable Vink principal (vink_user_id). Credentials and sessions belong to the auth authority still open in D-05; none are stored here.';

CREATE TABLE works (
    id uuid PRIMARY KEY,
    owner_id uuid NOT NULL CONSTRAINT works_owner_id_fkey REFERENCES principals (id),
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX works_owner_id_idx ON works (owner_id);

CREATE TABLE working_drafts (
    work_id uuid PRIMARY KEY REFERENCES works (id) ON DELETE CASCADE,
    version bigint NOT NULL CHECK (version >= 1),
    title text NOT NULL,
    document jsonb NOT NULL CHECK (jsonb_typeof(document -> 'blocks') = 'array'
        AND (document ->> 'schema_version') IS NOT NULL),
    updated_at timestamptz NOT NULL DEFAULT now()
);

COMMENT ON TABLE working_drafts IS
    'Private working copy. Saves are conditional on version (optimistic concurrency); never read by public paths.';

CREATE TABLE revisions (
    id uuid PRIMARY KEY,
    work_id uuid NOT NULL REFERENCES works (id),
    title text NOT NULL CHECK (char_length(title) >= 1),
    document jsonb NOT NULL CHECK (jsonb_typeof(document -> 'blocks') = 'array'
        AND (document ->> 'schema_version') IS NOT NULL),
    content_hash text NOT NULL CHECK (content_hash ~ '^[0-9a-f]{64}$'),
    created_by uuid NOT NULL REFERENCES principals (id),
    created_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT revisions_work_id_id_key UNIQUE (work_id, id)
);

CREATE FUNCTION reject_revision_update() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'revision % is an immutable snapshot', OLD.id
        USING ERRCODE = 'integrity_constraint_violation';
END;
$$;

CREATE TRIGGER revisions_are_immutable
    BEFORE UPDATE ON revisions
    FOR EACH ROW EXECUTE FUNCTION reject_revision_update();

CREATE TABLE publications (
    work_id uuid PRIMARY KEY REFERENCES works (id),
    revision_id uuid NOT NULL,
    generation bigint NOT NULL CHECK (generation >= 1),
    published_at timestamptz NOT NULL,
    CONSTRAINT publications_revision_of_work_fkey
        FOREIGN KEY (work_id, revision_id) REFERENCES revisions (work_id, id)
);

COMMENT ON TABLE publications IS
    'Current public pointer per work. The pointed revision must belong to the same work. Publish/withdraw commands (PUB-01..03) are not implemented.';
