-- Curated catalog audit (TAG-PR-03, #97). Additive to the #96 schema: one append-only row per
-- accepted catalog change, written in the SAME transaction as the change by the operator command
-- `vink-app tag …` (the governance entrypoint, #97 CHOICE-02 A). API, choices and runbook:
-- docs/development/taxonomy-api.md.
--
-- Every accepted change (also a same-state one) advances `tag_catalog.version` by exactly one, so
-- the audit is keyed by the version it produced. The database holds the pairing on its own:
-- a version step without its audit row, and an audit row ahead of the committed version, both
-- fail at commit, also for direct SQL. Rows record what changed, through which authority
-- and as which database role; they never hold manuscript text, private drafts or principals.

CREATE TABLE tag_catalog_changes (
    catalog_version bigint PRIMARY KEY CHECK (catalog_version >= 2),
    operation text NOT NULL
        CHECK (operation IN ('create', 'rename', 'describe', 'alias', 'retire')),
    tag_id uuid NOT NULL CONSTRAINT tag_catalog_changes_tag_id_fkey REFERENCES tags (id),
    -- The slug the change claimed (create, rename, alias); NULL for describe and retire.
    slug text CHECK (char_length(slug) BETWEEN 1 AND 40),
    -- The successor hint of a retirement, if any.
    successor_id uuid CONSTRAINT tag_catalog_changes_successor_id_fkey REFERENCES tags (id),
    -- false for an accepted same-state change (the version still advanced).
    changed boolean NOT NULL,
    -- The authority that decided `can_curate`. Only the operator command exists in the pilot.
    authority text NOT NULL CHECK (authority IN ('operator')),
    -- Who held the database credential. Attributes a change to a credential, not to a person.
    database_role text NOT NULL DEFAULT session_user,
    recorded_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT tag_catalog_changes_slug_claims
        CHECK ((slug IS NOT NULL) = (operation IN ('create', 'rename', 'alias'))),
    CONSTRAINT tag_catalog_changes_successor_only_on_retire
        CHECK (operation = 'retire' OR successor_id IS NULL)
);

COMMENT ON TABLE tag_catalog_changes IS
    'Append-only audit of accepted curated catalog changes, one row per catalog version step, written with the change.';

-- Immutable history: no UPDATE, DELETE or TRUNCATE (statement level, so an empty match is refused).
CREATE TRIGGER tag_catalog_changes_immutable
    BEFORE UPDATE OR DELETE OR TRUNCATE ON tag_catalog_changes
    FOR EACH STATEMENT EXECUTE FUNCTION reject_metadata_snapshot_change();

CREATE FUNCTION require_tag_catalog_audit() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM tag_catalog_changes WHERE catalog_version = NEW.version) THEN
        RAISE EXCEPTION 'catalog version % has no audit row', NEW.version
            USING ERRCODE = 'integrity_constraint_violation';
    END IF;
    RETURN NULL;
END;
$$;

CREATE FUNCTION require_audited_version() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    -- An earlier version already has its row (primary key); a later one has not happened.
    IF NOT EXISTS (SELECT 1 FROM tag_catalog WHERE version >= NEW.catalog_version) THEN
        RAISE EXCEPTION 'audit row % is ahead of the catalog version', NEW.catalog_version
            USING ERRCODE = 'integrity_constraint_violation';
    END IF;
    RETURN NULL;
END;
$$;

-- Deferred to commit: the change writes the version step and its audit row in one transaction,
-- in either order, and both are checked together.
CREATE CONSTRAINT TRIGGER tag_catalog_audited
    AFTER UPDATE ON tag_catalog
    DEFERRABLE INITIALLY DEFERRED
    FOR EACH ROW EXECUTE FUNCTION require_tag_catalog_audit();

CREATE CONSTRAINT TRIGGER tag_catalog_changes_versioned
    AFTER INSERT ON tag_catalog_changes
    DEFERRABLE INITIALLY DEFERRED
    FOR EACH ROW EXECUTE FUNCTION require_audited_version();
