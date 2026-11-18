-- Backout of the canonical document cutover, only while it is exact. Stop the App first.
-- No temporary tables (the migrator role cannot create them). Refused (nothing changes) when any revision exists: revisions are immutable and their
-- content_hash covers the canonical source, so they cannot be turned back into blocks. Also
-- refused when a Markdown draft exists or a default draft is not exactly representable as
-- blocks/lines (extra or whitespace-only blank lines, CR, NFD, leading/trailing blank lines):
-- converting it would silently drop authored text. Roll forward instead.
BEGIN;
LOCK TABLE working_drafts, revisions IN ACCESS EXCLUSIVE MODE;
DO $$
DECLARE
    whitespace constant text := E' \t\u00a0\u1680\u2000\u2001\u2002\u2003\u2004\u2005\u2006\u2007\u2008\u2009\u200a\u202f\u205f\u3000';
    forbidden constant text := U&'[\0001-\0008\000B-\001F\007F-\009F\2028\2029]';
BEGIN
    IF EXISTS (SELECT 1 FROM revisions) THEN
        RAISE EXCEPTION 'immutable canonical revisions exist; roll forward';
    END IF;
    IF EXISTS (
        SELECT 1 FROM working_drafts d,
               LATERAL (SELECT d.document ->> 'format' AS format,
                               d.document ->> 'source' AS source) c
         WHERE c.format <> 'default'
            OR char_length(replace(c.source, E'\n', '')) > 200000
            OR c.source COLLATE "C" ~ forbidden
            OR d.title IS NOT NFC NORMALIZED OR char_length(d.title) > 200
            OR d.title <> btrim(d.title, whitespace)
            OR d.title COLLATE "C" ~ forbidden OR d.title ~ E'[\t\n]'
            OR c.source IS DISTINCT FROM normalize(c.source, NFC)
            OR c.source ~ E'\r|^\n|\n$|\n\n\n'
            OR EXISTS (SELECT 1 FROM regexp_split_to_table(c.source, E'\n') AS l(line)
                        -- All non-control Unicode White_Space accepted in canonical source;
                        -- Rust's retired line validator rejects each whitespace-only line.
                        WHERE c.source <> ''
                          AND btrim(l.line, whitespace) = ''
                          AND l.line <> ''))
    THEN
        RAISE EXCEPTION 'a draft is not exactly representable as blocks/lines; roll forward';
    END IF;
END $$;
ALTER TABLE working_drafts DROP CONSTRAINT working_drafts_document_check;
ALTER TABLE revisions DROP CONSTRAINT revisions_document_check;
UPDATE working_drafts d
   SET document = jsonb_build_object('schema_version', 1, 'blocks', coalesce((
           SELECT jsonb_agg(jsonb_build_object('lines', to_jsonb(string_to_array(b.block, E'\n')))
                            ORDER BY b.n)
             FROM regexp_split_to_table(d.document ->> 'source', E'\n\n')
                  WITH ORDINALITY AS b(block, n)
            WHERE d.document ->> 'source' <> ''), '[]'::jsonb));
ALTER TABLE working_drafts ADD CONSTRAINT working_drafts_document_check
    CHECK (jsonb_typeof(document -> 'blocks') = 'array' AND (document ->> 'schema_version') IS NOT NULL);
ALTER TABLE revisions ADD CONSTRAINT revisions_document_check
    CHECK (jsonb_typeof(document -> 'blocks') = 'array' AND (document ->> 'schema_version') IS NOT NULL);
DROP FUNCTION vink_document_is_canonical(jsonb);
COMMENT ON COLUMN working_drafts.document IS NULL;
COMMENT ON COLUMN working_drafts.title IS NULL;
COMMENT ON COLUMN revisions.document IS NULL;
DELETE FROM _sqlx_migrations WHERE version = 20261122000000;
COMMIT;
