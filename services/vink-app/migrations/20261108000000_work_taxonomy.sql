-- Work taxonomy, PostgreSQL slice (TAG-PR-02, #96). The pure core of #95
-- (services/vink-app/src/domain/taxonomy) decides every change; this schema holds the structural
-- rules on its own so that direct SQL or an adapter bug cannot store a state no decision produces.
-- Storage choices, lock order, runbook and evidence: docs/development/taxonomy-postgres.md.
--
-- Physical representation (#96 CHOICE-01 A): a normalized catalog (`tags`, one row per slug in
-- `tag_names`) and normalized assignment rows, one row per SLOT, for both the private draft
-- (`work_draft_tags`) and the immutable public snapshot (`work_metadata_snapshot_tags`). Slot 0
-- is the form, slots 1..3 are themes. Composite foreign keys (tag_id, facet) → tags (id, facet)
-- make a wrong facet, a dangling tag and a fourth theme unrepresentable without any count CHECK
-- pretending to see other rows. Raising MAX_THEMES_PER_WORK needs an additive migration that
-- widens the two slot CHECKs; the PostgreSQL tests fail until it exists.
--
-- Concurrency (#96 CHOICE-02 B): draft metadata has its own version, separate from the working
-- draft's text version; a publication captures (revision, metadata snapshot) as an explicit pair.
--
-- Nothing here infers a tag from manuscript text. Existing works and publications stay untagged:
-- an absent draft metadata row means version 1 with no tags, and a publication without a bound
-- snapshot shows no tags. There is no backfill.

-- One row: the optimistic-concurrency version of the whole curated catalog (CatalogVersion).
-- Catalog changes lock it FOR UPDATE, so curator changes are serialized.
CREATE TABLE tag_catalog (
    singleton boolean PRIMARY KEY DEFAULT true CHECK (singleton),
    version bigint NOT NULL CHECK (version >= 1),
    updated_at timestamptz NOT NULL DEFAULT now()
);

INSERT INTO tag_catalog (singleton, version) VALUES (true, 1);

COMMENT ON TABLE tag_catalog IS
    'Single row holding the curated catalog version. Every accepted catalog change advances it, also a same-state one.';

CREATE TABLE tags (
    id uuid PRIMARY KEY,
    facet text NOT NULL CHECK (facet IN ('form', 'theme')),
    label text NOT NULL CHECK (char_length(label) BETWEEN 1 AND 40 AND label !~ '[[:cntrl:]]'),
    slug text NOT NULL CHECK (char_length(slug) BETWEEN 1 AND 40),
    -- Constant; lets tags_canonical_name_fkey reach a CANONICAL row of tag_names only.
    slug_kind text NOT NULL DEFAULT 'canonical' CHECK (slug_kind = 'canonical'),
    description text CHECK (char_length(description) BETWEEN 1 AND 280
        AND description !~ '[[:cntrl:]]'),
    status text NOT NULL CHECK (status IN ('active', 'retired')),
    -- A curator's hint for authors; never applied automatically.
    successor_id uuid,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT tags_id_facet_key UNIQUE (id, facet),
    CONSTRAINT tags_successor_only_when_retired CHECK (status = 'retired' OR successor_id IS NULL),
    CONSTRAINT tags_successor_not_self CHECK (successor_id <> id),
    -- The successor is another tag of the SAME facet.
    CONSTRAINT tags_successor_same_facet_fkey
        FOREIGN KEY (successor_id, facet) REFERENCES tags (id, facet)
);

COMMENT ON TABLE tags IS
    'Canonical catalog tags (#95 CHOICE-01 B, CHOICE-03 A). Identity and facet never change; retirement is terminal in the pilot; rows are never deleted.';

-- One row per slug, ever: a slug that named a tag (canonically, as an alias or as a former name)
-- names that tag forever. The primary key is the uniqueness the core relies on.
CREATE TABLE tag_names (
    slug text PRIMARY KEY CHECK (char_length(slug) BETWEEN 1 AND 40),
    tag_id uuid NOT NULL CONSTRAINT tag_names_tag_id_fkey REFERENCES tags (id),
    kind text NOT NULL CHECK (kind IN ('canonical', 'alias', 'former')),
    -- The curated alias spelling; canonical labels live on tags, former names keep only a slug.
    alias_label text CHECK (char_length(alias_label) BETWEEN 1 AND 40
        AND alias_label !~ '[[:cntrl:]]'),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT tag_names_alias_label CHECK ((kind = 'alias') = (alias_label IS NOT NULL)),
    CONSTRAINT tag_names_tag_slug_kind_key UNIQUE (tag_id, slug, kind)
);

-- Exactly one canonical name per tag: at most one here, at least one through the deferred key
-- below. A rename demotes the old canonical row before promoting the new one.
CREATE UNIQUE INDEX tag_names_one_canonical_key ON tag_names (tag_id) WHERE kind = 'canonical';

-- A tag's slug is its canonical name row. Deferred: a tag and its first name are inserted in
-- one transaction, and a rename moves both before commit.
ALTER TABLE tags ADD CONSTRAINT tags_canonical_name_fkey
    FOREIGN KEY (id, slug, slug_kind) REFERENCES tag_names (tag_id, slug, kind)
    DEFERRABLE INITIALLY DEFERRED;

COMMENT ON TABLE tag_names IS
    'Every slug a tag has had: its canonical name, curated aliases and former names (redirects). Slug and tag never change; rows are never deleted.';

CREATE FUNCTION guard_tag_update() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.id IS DISTINCT FROM OLD.id
        OR NEW.facet IS DISTINCT FROM OLD.facet
        OR NEW.created_at IS DISTINCT FROM OLD.created_at
    THEN
        RAISE EXCEPTION 'tag % identity and facet are immutable', OLD.id
            USING ERRCODE = 'integrity_constraint_violation';
    END IF;
    IF OLD.status = 'retired' THEN
        RAISE EXCEPTION 'tag % is retired; retirement is terminal', OLD.id
            USING ERRCODE = 'integrity_constraint_violation';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER tags_guard_update
    BEFORE UPDATE ON tags
    FOR EACH ROW EXECUTE FUNCTION guard_tag_update();

CREATE FUNCTION guard_tag_name_write() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'UPDATE' AND (
        NEW.slug IS DISTINCT FROM OLD.slug
        OR NEW.tag_id IS DISTINCT FROM OLD.tag_id
        OR NEW.created_at IS DISTINCT FROM OLD.created_at
    ) THEN
        RAISE EXCEPTION 'slug % belongs to its tag forever', OLD.slug
            USING ERRCODE = 'integrity_constraint_violation';
    END IF;
    -- A retired tag takes no new name, alias or rename.
    IF EXISTS (SELECT 1 FROM tags WHERE id = NEW.tag_id AND status = 'retired') THEN
        RAISE EXCEPTION 'names of retired tag % are frozen', NEW.tag_id
            USING ERRCODE = 'integrity_constraint_violation';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER tag_names_guard_write
    BEFORE INSERT OR UPDATE ON tag_names
    FOR EACH ROW EXECUTE FUNCTION guard_tag_name_write();

CREATE FUNCTION guard_tag_catalog_version() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.version <= OLD.version THEN
        RAISE EXCEPTION 'catalog version must advance (% -> %)', OLD.version, NEW.version
            USING ERRCODE = 'integrity_constraint_violation';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER tag_catalog_guard_version
    BEFORE UPDATE ON tag_catalog
    FOR EACH ROW EXECUTE FUNCTION guard_tag_catalog_version();

CREATE FUNCTION reject_taxonomy_removal() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION '% rows are retained', TG_TABLE_NAME
        USING ERRCODE = 'integrity_constraint_violation';
END;
$$;

-- Statement-level, so a DELETE matching no row is refused too, and TRUNCATE cannot bypass it.
CREATE TRIGGER tag_catalog_retained
    BEFORE DELETE OR TRUNCATE ON tag_catalog
    FOR EACH STATEMENT EXECUTE FUNCTION reject_taxonomy_removal();

CREATE TRIGGER tags_retained
    BEFORE DELETE OR TRUNCATE ON tags
    FOR EACH STATEMENT EXECUTE FUNCTION reject_taxonomy_removal();

CREATE TRIGGER tag_names_retained
    BEFORE DELETE OR TRUNCATE ON tag_names
    FOR EACH STATEMENT EXECUTE FUNCTION reject_taxonomy_removal();

-- Private draft metadata: the version (MetadataVersion) of a work's draft tags. Created lazily by
-- the first save; an absent row is version 1 with no tags. Never read by public paths.
CREATE TABLE work_draft_metadata (
    work_id uuid PRIMARY KEY CONSTRAINT work_draft_metadata_work_id_fkey REFERENCES works (id),
    version bigint NOT NULL DEFAULT 1 CHECK (version >= 1),
    updated_at timestamptz NOT NULL DEFAULT now()
);

COMMENT ON TABLE work_draft_metadata IS
    'Version of the private draft tags of a work, separate from the working draft text version (#96 CHOICE-02 B). Every accepted save advances it; rows are never deleted (a restart would let an old command apply again).';

CREATE TABLE work_draft_tags (
    work_id uuid NOT NULL CONSTRAINT work_draft_tags_work_id_fkey
        REFERENCES work_draft_metadata (work_id),
    slot smallint NOT NULL CHECK (slot BETWEEN 0 AND 3),
    tag_id uuid NOT NULL,
    facet text NOT NULL,
    CONSTRAINT work_draft_tags_pkey PRIMARY KEY (work_id, slot),
    -- One identity once per work, in either slot (aliases are resolved before they get here).
    CONSTRAINT work_draft_tags_once_key UNIQUE (work_id, tag_id),
    CONSTRAINT work_draft_tags_slot_facet CHECK ((slot = 0) = (facet = 'form')),
    CONSTRAINT work_draft_tags_tag_fkey FOREIGN KEY (tag_id, facet) REFERENCES tags (id, facet)
);

COMMENT ON TABLE work_draft_tags IS
    'The private draft tags: slot 0 is the form, slots 1..3 the themes in the author''s order. Replaced as a whole by an accepted save; never joined by public reads.';

CREATE FUNCTION guard_draft_metadata_version() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.work_id IS DISTINCT FROM OLD.work_id THEN
        RAISE EXCEPTION 'draft metadata identity is immutable'
            USING ERRCODE = 'integrity_constraint_violation';
    END IF;
    IF NEW.version <= OLD.version THEN
        RAISE EXCEPTION 'draft metadata version must advance (% -> %)', OLD.version, NEW.version
            USING ERRCODE = 'integrity_constraint_violation';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER work_draft_metadata_guard_version
    BEFORE UPDATE ON work_draft_metadata
    FOR EACH ROW EXECUTE FUNCTION guard_draft_metadata_version();

CREATE TRIGGER work_draft_metadata_retained
    BEFORE DELETE OR TRUNCATE ON work_draft_metadata
    FOR EACH STATEMENT EXECUTE FUNCTION reject_taxonomy_removal();

-- Immutable metadata snapshots, captured inside the publication owner's transaction and bound
-- beside the revision. A later draft edit makes a new snapshot; it never changes this one.
CREATE TABLE work_metadata_snapshots (
    id uuid PRIMARY KEY,
    work_id uuid NOT NULL CONSTRAINT work_metadata_snapshots_work_id_fkey REFERENCES works (id),
    -- Provenance: the draft metadata version the snapshot was captured from.
    metadata_version bigint NOT NULL CHECK (metadata_version >= 1),
    -- MetadataDigest (vink-work-metadata-v1), verified against the tag rows at commit.
    digest text NOT NULL CHECK (digest ~ '^[0-9a-f]{64}$'),
    created_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT work_metadata_snapshots_work_id_id_key UNIQUE (work_id, id)
);

COMMENT ON TABLE work_metadata_snapshots IS
    'Immutable tags captured for a publication (#95 CHOICE-02 B). Never updated or deleted; the digest is separate from the revision content_hash v1.';

CREATE TABLE work_metadata_snapshot_tags (
    snapshot_id uuid NOT NULL CONSTRAINT work_metadata_snapshot_tags_snapshot_id_fkey
        REFERENCES work_metadata_snapshots (id),
    slot smallint NOT NULL CHECK (slot BETWEEN 0 AND 3),
    tag_id uuid NOT NULL,
    facet text NOT NULL,
    CONSTRAINT work_metadata_snapshot_tags_pkey PRIMARY KEY (snapshot_id, slot),
    CONSTRAINT work_metadata_snapshot_tags_once_key UNIQUE (snapshot_id, tag_id),
    CONSTRAINT work_metadata_snapshot_tags_slot_facet CHECK ((slot = 0) = (facet = 'form')),
    CONSTRAINT work_metadata_snapshot_tags_tag_fkey
        FOREIGN KEY (tag_id, facet) REFERENCES tags (id, facet)
);

COMMENT ON TABLE work_metadata_snapshot_tags IS
    'Tag slots of an immutable metadata snapshot (slot 0 form, 1..3 themes in order). Sealed by the snapshot digest at commit.';

-- The same bytes as MetadataDigest::of in Rust: SHA-256 over 'vink-work-metadata-v1', then
-- 0x00 or 0x01 + the form's 16 bytes, the theme count as u64 big-endian and each theme's 16
-- bytes in order. Two independent implementations of one format; the tests compare both with
-- the golden vectors of #95.
CREATE FUNCTION work_metadata_digest(form uuid, themes uuid[]) RETURNS text
LANGUAGE sql IMMUTABLE PARALLEL SAFE AS $$
    SELECT encode(sha256(
        convert_to('vink-work-metadata-v1', 'UTF8')
        || CASE WHEN form IS NULL THEN '\x00'::bytea ELSE '\x01'::bytea || uuid_send(form) END
        || int8send(coalesce(cardinality(themes), 0)::bigint)
        || coalesce(
            (SELECT string_agg(uuid_send(t.id), ''::bytea ORDER BY t.n)
               FROM unnest(themes) WITH ORDINALITY AS t (id, n)),
            ''::bytea)
    ), 'hex')
$$;

CREATE FUNCTION verify_metadata_snapshot(snapshot uuid) RETURNS void
LANGUAGE plpgsql AS $$
DECLARE
    stored text;
    actual text;
BEGIN
    SELECT s.digest,
           work_metadata_digest(
               (SELECT t.tag_id FROM work_metadata_snapshot_tags t
                 WHERE t.snapshot_id = s.id AND t.slot = 0),
               ARRAY(SELECT t.tag_id FROM work_metadata_snapshot_tags t
                      WHERE t.snapshot_id = s.id AND t.slot > 0 ORDER BY t.slot))
      INTO stored, actual
      FROM work_metadata_snapshots s
     WHERE s.id = snapshot;
    IF stored IS DISTINCT FROM actual THEN
        RAISE EXCEPTION 'metadata snapshot % does not match its digest', snapshot
            USING ERRCODE = 'integrity_constraint_violation';
    END IF;
END;
$$;

CREATE FUNCTION seal_metadata_snapshot() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    PERFORM verify_metadata_snapshot(NEW.id);
    RETURN NULL;
END;
$$;

CREATE FUNCTION seal_metadata_snapshot_tag() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    PERFORM verify_metadata_snapshot(NEW.snapshot_id);
    RETURN NULL;
END;
$$;

-- Deferred to commit: a snapshot and its slots are written in one transaction, then checked
-- together. A slot added to an older snapshot by a later transaction changes the digest and
-- fails that transaction's commit.
CREATE CONSTRAINT TRIGGER work_metadata_snapshots_sealed
    AFTER INSERT ON work_metadata_snapshots
    DEFERRABLE INITIALLY DEFERRED
    FOR EACH ROW EXECUTE FUNCTION seal_metadata_snapshot();

CREATE CONSTRAINT TRIGGER work_metadata_snapshot_tags_sealed
    AFTER INSERT ON work_metadata_snapshot_tags
    DEFERRABLE INITIALLY DEFERRED
    FOR EACH ROW EXECUTE FUNCTION seal_metadata_snapshot_tag();

CREATE FUNCTION reject_metadata_snapshot_change() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION '% rows are immutable', TG_TABLE_NAME
        USING ERRCODE = 'integrity_constraint_violation';
END;
$$;

CREATE TRIGGER work_metadata_snapshots_immutable
    BEFORE UPDATE OR DELETE OR TRUNCATE ON work_metadata_snapshots
    FOR EACH STATEMENT EXECUTE FUNCTION reject_metadata_snapshot_change();

CREATE TRIGGER work_metadata_snapshot_tags_immutable
    BEFORE UPDATE OR DELETE OR TRUNCATE ON work_metadata_snapshot_tags
    FOR EACH STATEMENT EXECUTE FUNCTION reject_metadata_snapshot_change();

-- The publication binds the pair (revision, metadata snapshot) in ONE row, so a pointer change
-- moves both atomically and a public read of that row cannot mix two publications. NULL is a
-- publication without metadata (every publication before this migration): it shows no tags.
-- Only the publication owner's transaction may set it (PUB-01..03); no route writes it.
ALTER TABLE publications
    ADD COLUMN metadata_snapshot_id uuid,
    ADD CONSTRAINT publications_metadata_of_work_fkey
        FOREIGN KEY (work_id, metadata_snapshot_id)
        REFERENCES work_metadata_snapshots (work_id, id);

COMMENT ON COLUMN publications.metadata_snapshot_id IS
    'Metadata snapshot bound with revision_id; it must belong to the same work. NULL: published without metadata (shows no tags).';
