-- One canonical, unversioned document (#255, #263; VDR-DOCUMENT-SOURCE-FORMAT@r2).
-- Product decision 17/11/2026: replace the blocks/lines document completely; no v1/v2, no
-- dual read or write. Stored shape after this migration: {"format": "default"|"markdown",
-- "source": "<exact authored text>"}; nothing else is accepted by the CHECKs below.
--
-- OFFLINE CUTOVER: stop every App binary and worker before running it. Binaries older than this
-- migration cannot read the new shape, and this binary cannot read the old one.
--
-- Immutable history: revisions store the retired shape and a content_hash over it; evaluation
-- jobs, cases and appeals reference that digest. Converting them would rewrite immutable
-- snapshots and their identity digest, which needs a separately authorized history plan
-- (docs/architecture/document-source-format.md, "Lịch sử bất biến"). Until then this migration
-- refuses to run while any revision exists in the retired shape: it changes nothing and
-- records nothing, so no partial cutover can happen.

DO $$
DECLARE
    retired bigint;
BEGIN
    SELECT count(*) INTO retired FROM revisions WHERE NOT (document ? 'format');
    IF retired > 0 THEN
        RAISE EXCEPTION 'canonical document cutover blocked: % immutable revision(s) store the retired document shape', retired
            USING ERRCODE = 'object_not_in_prerequisite_state',
                  HINT = 'Converting them changes content_hash; follow the authorized history plan in docs/architecture/document-source-format.md before migrating.';
    END IF;
END $$;

-- The retired draft shape must be exactly {"schema_version": 1, "blocks": [{"lines": [text…]}…]};
-- anything else is refused instead of being guessed at.
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM working_drafts d
         WHERE NOT (d.document ? 'format')
           AND (d.document IS DISTINCT FROM jsonb_build_object(
                    'schema_version', 1, 'blocks', d.document -> 'blocks')
                OR jsonb_typeof(d.document -> 'blocks') IS DISTINCT FROM 'array'
                OR EXISTS (
                    SELECT 1 FROM jsonb_array_elements(d.document -> 'blocks') AS b(block)
                     WHERE jsonb_typeof(b.block) IS DISTINCT FROM 'object'
                        OR b.block IS DISTINCT FROM jsonb_build_object('lines', b.block -> 'lines')
                        OR jsonb_typeof(b.block -> 'lines') IS DISTINCT FROM 'array'
                        OR b.block -> 'lines' = '[]'::jsonb
                        OR EXISTS (SELECT 1 FROM jsonb_array_elements(b.block -> 'lines') AS l(line)
                                    WHERE jsonb_typeof(l.line) IS DISTINCT FROM 'string')))
    ) THEN
        RAISE EXCEPTION 'canonical document cutover blocked: a working draft has an unexpected document shape'
            USING ERRCODE = 'data_exception';
    END IF;
END $$;

-- The retired CHECK allowed rows the writer could never produce. Such lines cannot
-- round-trip through blank-line separators; invalid limits/titles would make the new
-- binary reject the converted draft. Refuse these rows before any UPDATE.
DO $$
DECLARE
    whitespace constant text := E' \t\u00a0\u1680\u2000\u2001\u2002\u2003\u2004\u2005\u2006\u2007\u2008\u2009\u200a\u202f\u205f\u3000';
    forbidden constant text := U&'[\0001-\0008\000A-\001F\007F-\009F\2028\2029]';
BEGIN
    IF EXISTS (
        SELECT 1 FROM working_drafts d
         WHERE NOT (d.document ? 'format') AND (
               d.title IS NOT NFC NORMALIZED OR char_length(d.title) > 200
               OR d.title <> btrim(d.title, whitespace)
               OR d.title COLLATE "C" ~ forbidden OR strpos(d.title, E'\t') > 0
               OR (SELECT coalesce(bool_or(
                              l.line IS NOT NFC NORMALIZED OR btrim(l.line, whitespace) = ''
                              OR l.line COLLATE "C" ~ forbidden), false)
                          OR coalesce(sum(char_length(l.line)), 0) > 200000
                     FROM jsonb_array_elements(d.document -> 'blocks') b(block),
                          LATERAL jsonb_array_elements_text(b.block -> 'lines') l(line)))
    ) THEN
        RAISE EXCEPTION 'canonical document cutover blocked: a retired draft cannot be converted exactly'
            USING ERRCODE = 'data_exception';
    END IF;
END $$;

-- Deterministic draft conversion of the text that is still stored: lines joined by LF, blocks by
-- one blank line, title column untouched, format default (the retired drafts had a separate
-- title and were never Markdown). Blank lines, CRLF or NFD that earlier writes already dropped
-- cannot be recovered. Version and updated_at stay: the author's text did not change.
ALTER TABLE working_drafts DROP CONSTRAINT working_drafts_document_check;
ALTER TABLE revisions DROP CONSTRAINT revisions_document_check;

-- No temporary tables: the provisioned migrator role cannot create them.
DO $$
DECLARE
    retired bigint;
    converted bigint;
BEGIN
    SELECT count(*) INTO retired FROM working_drafts WHERE NOT (document ? 'format');
    UPDATE working_drafts d
       SET document = jsonb_build_object(
               'format', 'default',
               'source', coalesce((
                   SELECT string_agg(
                              (SELECT string_agg(l.line, E'\n' ORDER BY l.n)
                                 FROM jsonb_array_elements_text(b.block -> 'lines')
                                      WITH ORDINALITY AS l(line, n)),
                              E'\n\n' ORDER BY b.n)
                     FROM jsonb_array_elements(d.document -> 'blocks')
                          WITH ORDINALITY AS b(block, n)),
                   ''))
     WHERE NOT (d.document ? 'format');
    GET DIAGNOSTICS converted = ROW_COUNT;
    IF converted IS DISTINCT FROM retired THEN
        RAISE EXCEPTION 'canonical document cutover converted % of % drafts', converted, retired;
    END IF;
END $$;

CREATE FUNCTION vink_document_is_canonical(document jsonb) RETURNS boolean
LANGUAGE sql IMMUTABLE STRICT PARALLEL SAFE AS $function$
SELECT coalesce(CASE WHEN jsonb_typeof(document) = 'object' THEN
           document ?& ARRAY['format', 'source']
           AND (document - 'format' - 'source') = '{}'::jsonb
           AND jsonb_typeof(document -> 'format') = 'string'
           AND document ->> 'format' IN ('default', 'markdown')
           AND jsonb_typeof(document -> 'source') = 'string'
           AND octet_length(document ->> 'source') <= 2097152
       END, false)
$function$;

COMMENT ON FUNCTION vink_document_is_canonical(jsonb) IS
    'The one stored document shape {format, source}. Shape and byte cap only; the domain owns text and title validation.';

ALTER TABLE working_drafts
    ADD CONSTRAINT working_drafts_document_check CHECK (vink_document_is_canonical(document));

ALTER TABLE revisions
    ADD CONSTRAINT revisions_document_check CHECK (vink_document_is_canonical(document));

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM working_drafts WHERE NOT vink_document_is_canonical(document))
       OR EXISTS (SELECT 1 FROM working_drafts d LEFT JOIN works w ON w.id = d.work_id
                   WHERE w.id IS NULL)
    THEN
        RAISE EXCEPTION 'canonical document cutover left a non-canonical or orphaned draft';
    END IF;
END $$;

COMMENT ON COLUMN working_drafts.document IS
    'Canonical document {format, source}: exact authored source; format default|markdown.';
COMMENT ON COLUMN working_drafts.title IS
    'Effective title: the separate title of a default document, or the first-H1 projection of a markdown one (derived in the same save).';
COMMENT ON COLUMN revisions.document IS
    'Immutable canonical document {format, source}; content_hash covers format, effective title and exact source.';
