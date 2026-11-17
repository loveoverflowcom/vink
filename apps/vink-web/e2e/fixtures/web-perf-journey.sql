-- #234. Only xtask's disposable vink_verify_web scratch database.
-- Reuses the #100 fixture's canonical document encoding/digest and publication metadata snapshot.
-- #100 seeds n=1..14. This delta adds n=15..200, yielding exactly 200 labelled public
-- journey works. The dev-seed sample work is intentionally outside that count.
BEGIN;
CREATE TEMP TABLE perf_works ON COMMIT DROP AS
SELECT n, md5('vink-100-work-' || n)::uuid AS work,
       md5('vink-100-revision-' || n)::uuid AS revision,
       md5('vink-100-snapshot-' || n)::uuid AS snapshot,
       '[Mẫu #100] Kiểm thử ' || n::text AS title,
       'Một dòng công khai dùng riêng cho kiểm thử #100.'::text AS body,
       (SELECT id FROM tags WHERE label = 'Thơ') AS form,
       (SELECT id FROM tags WHERE label = 'Ký ức') AS theme
FROM generate_series(15, 200) n;

INSERT INTO works (id, owner_id)
SELECT work, '11111111-1111-4111-8111-111111111111' FROM perf_works;
INSERT INTO revisions (id, work_id, title, document, content_hash, created_by)
SELECT revision, work, title,
       jsonb_build_object('format', 'default', 'source', body),
       encode(sha256(convert_to('vink-document-content', 'UTF8') ||
         int8send(octet_length('default')::bigint) || convert_to('default', 'UTF8') ||
         int8send(octet_length(title)::bigint) || convert_to(title, 'UTF8') ||
         int8send(octet_length(body)::bigint) || convert_to(body, 'UTF8')), 'hex'),
       '11111111-1111-4111-8111-111111111111'
FROM perf_works;
INSERT INTO work_metadata_snapshots (id, work_id, metadata_version, digest)
SELECT snapshot, work, 1, work_metadata_digest(form, ARRAY[theme])
FROM perf_works;
INSERT INTO work_metadata_snapshot_tags (snapshot_id, slot, tag_id, facet)
SELECT snapshot, 0, form, 'form' FROM perf_works
UNION ALL
SELECT snapshot, 1, theme, 'theme' FROM perf_works;
-- Strictly older than every #100 publication, independent of the wall-clock gap between the
-- two seeds: discovery orders by published_at DESC, so #100 work 1 (the hot thread below)
-- deterministically stays the first filtered row that the warm journey opens.
INSERT INTO publications (work_id, revision_id, generation, published_at, metadata_snapshot_id)
SELECT work, revision, 1,
       (SELECT min(published_at) FROM publications) - n * interval '1 second', snapshot
FROM perf_works;

-- One hot public thread with 800 roots and 200 one-level replies: 1,000 comments,
-- deliberately much larger than the actual public page limit. The browser never mounts
-- this entire corpus. Moderation provenance is the only supported premoderation policy.
INSERT INTO discussion_threads (id, work_id)
VALUES (md5('vink-234-thread-1')::uuid, md5('vink-100-work-1')::uuid);
INSERT INTO comments (id, thread_id, kind, replied_to_reply, author_id, body,
                      review, moderation_version, admission_policy, admission_reason, created_at)
SELECT md5('vink-234-comment-' || n)::uuid, md5('vink-234-thread-1')::uuid,
       'root', false, '11111111-1111-4111-8111-111111111111',
       '[Mẫu #234] Nhận xét ' || n, 'pending', 1,
       'premoderation.v1', 'policy_premoderation',
       timestamptz '2026-01-01 00:00:00+00' + n * interval '1 second'
FROM generate_series(1, 800) n;
INSERT INTO comments (id, thread_id, kind, root_id, root_kind, replied_to_reply, author_id,
                      body, review, moderation_version, admission_policy, admission_reason, created_at)
SELECT md5('vink-234-comment-' || n)::uuid, md5('vink-234-thread-1')::uuid,
       'reply', md5('vink-234-comment-' || (n - 800))::uuid, 'root', false,
       '11111111-1111-4111-8111-111111111111',
       '[Mẫu #234] Phản hồi ' || n, 'pending', 1,
       'premoderation.v1', 'policy_premoderation',
       timestamptz '2026-01-01 00:00:00+00' + n * interval '1 second'
FROM generate_series(801, 1000) n;
-- Scratch-only seeding of review outcomes, not evidence of the production moderation workflow.
UPDATE comments SET review = 'approved', moderation_version = 2
WHERE thread_id = md5('vink-234-thread-1')::uuid;
COMMIT;
