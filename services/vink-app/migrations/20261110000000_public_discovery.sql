-- Public discovery, PostgreSQL slice (TAG-PR-04, #98). Searches read only through the publication
-- pointer: the pointed revision's title, the owner's current display name and the metadata
-- snapshot bound in the same row. Working drafts and private draft tags are never joined.
-- Choices, query plans and measurements: docs/development/discovery-api.md.
--
-- `vink_search_key` mirrors `SearchKey::of` (services/vink-app/src/domain/taxonomy/text.rs): NFD,
-- combining marks dropped, `đ`/`Đ` → `d`, lowercase, every run of non-alphanumeric characters one
-- space, trimmed. For Vietnamese and Latin text the two give identical keys (the PostgreSQL tests
-- compare them over every Vietnamese letter in NFC and NFD). Outside that range they may differ
-- (marks outside the listed blocks, superscript digits); both sides of a match are folded here, so
-- that only affects how those characters match, never what is public. The explicit ICU collation
-- makes lowercase and the alphanumeric class independent of the database's default locale; a
-- server built without ICU fails this migration instead of folding differently.
CREATE FUNCTION vink_search_key(input text) RETURNS text
LANGUAGE sql IMMUTABLE STRICT PARALLEL SAFE AS $$
    SELECT btrim(
        regexp_replace(
            lower(
                translate(
                    regexp_replace(
                        normalize(input, NFD),
                        '[̀-ͯ᪰-᫿᷀-᷿⃐-⃿︠-︯]',
                        '', 'g'),
                    'đĐ', 'dd')
                COLLATE "und-x-icu"),
            '[^[:alnum:]]+', ' ', 'g'),
        ' ')
$$;

COMMENT ON FUNCTION vink_search_key(text) IS
    'Accent- and case-folded search key of public text (mirrors SearchKey::of). Finds candidates; never decides identity.';

-- Stored keys (#98 CHOICE-02 B, measured: folding 20 000 titles per search took ~0.2 s). Generated
-- columns are computed by PostgreSQL in the same statement that writes the text, so no writer can
-- forget them, no repair job exists and a key can never belong to other text. Revisions are
-- immutable, so their key never changes; a display-name change recomputes its key in its own
-- UPDATE. Adding them rewrites both tables once. If an ICU upgrade ever changed how a character
-- folds, `ALTER TABLE … ALTER COLUMN … SET EXPRESSION AS (vink_search_key(…))` recomputes them.
ALTER TABLE revisions
    ADD COLUMN title_search_key text GENERATED ALWAYS AS (vink_search_key(title)) STORED;

ALTER TABLE principals
    ADD COLUMN display_name_search_key text
        GENERATED ALWAYS AS (vink_search_key(display_name)) STORED;

COMMENT ON COLUMN revisions.title_search_key IS
    'vink_search_key(title), maintained by PostgreSQL. A search aid, never an identity.';
COMMENT ON COLUMN principals.display_name_search_key IS
    'vink_search_key(display_name), maintained by PostgreSQL. A search aid, never an identity.';

-- The public order (newest first, then work id), so a page reads its rows from the index and
-- stops, and a cursor starts at its position instead of skipping everything before it.
CREATE INDEX publications_public_order_idx ON publications (published_at DESC, work_id);

-- Snapshots carrying a tag, for tag filters. The unique key (snapshot_id, tag_id) serves the
-- opposite direction (tags of one snapshot).
CREATE INDEX work_metadata_snapshot_tags_tag_idx
    ON work_metadata_snapshot_tags (tag_id, snapshot_id);
