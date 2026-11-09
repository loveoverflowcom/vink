-- TAG-PR-06, ONLY the disposable verify-web scratch database.
-- SQL publication fixtures qualify public reads; no author publish workflow is exercised.
BEGIN;
CREATE TEMP TABLE discovery_fixture ON COMMIT DROP AS
SELECT n, md5('vink-100-work-' || n)::uuid AS work,
       md5('vink-100-revision-' || n)::uuid AS revision,
       md5('vink-100-snapshot-' || n)::uuid AS snapshot,
       '[Mẫu #100] Kiểm thử ' || CASE WHEN n = 14 THEN 'Bến hiếm sau trang đầu' ELSE n::text END AS title,
       'Một dòng công khai dùng riêng cho kiểm thử #100.'::text AS body,
       (SELECT id FROM tags WHERE label = 'Thơ') AS form,
       (SELECT id FROM tags WHERE label = 'Ký ức') AS theme
FROM generate_series(1,14) n;
INSERT INTO works (id, owner_id)
SELECT work, '11111111-1111-4111-8111-111111111111' FROM discovery_fixture;
INSERT INTO revisions (id, work_id, title, document, content_hash, created_by)
SELECT revision, work, title,
       jsonb_build_object('schema_version',1,'blocks',jsonb_build_array(jsonb_build_object('lines',jsonb_build_array(body)))),
       encode(sha256(convert_to('vink-revision-content-v1','UTF8') ||
           int8send(octet_length(title)::bigint) || convert_to(title,'UTF8') ||
           int8send(1) || int8send(1) || int8send(octet_length(body)::bigint) || convert_to(body,'UTF8')),'hex'),
       '11111111-1111-4111-8111-111111111111'
FROM discovery_fixture;
INSERT INTO work_metadata_snapshots (id, work_id, metadata_version, digest)
SELECT snapshot, work, 1, work_metadata_digest(form, ARRAY[theme]) FROM discovery_fixture;
INSERT INTO work_metadata_snapshot_tags (snapshot_id, slot, tag_id, facet)
SELECT snapshot, 0, form, 'form' FROM discovery_fixture
UNION ALL SELECT snapshot, 1, theme, 'theme' FROM discovery_fixture;
INSERT INTO publications (work_id, revision_id, generation, published_at, metadata_snapshot_id)
SELECT work, revision, 1, now() - n * interval '1 second', snapshot FROM discovery_fixture;
COMMIT;
