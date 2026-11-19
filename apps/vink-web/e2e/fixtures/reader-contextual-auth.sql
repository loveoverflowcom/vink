-- #328. Run only in the disposable verify-web Auth database, after dev-seed.
-- This arranges a public reply target and a second publication with comments closed.
-- Auth, discussion reads, permissions, and new comments then go through the real HTTP APIs.
BEGIN;
INSERT INTO discussion_threads (id, work_id)
VALUES ('3ac70000-0000-4000-8000-000000000001', '33333333-3333-4333-8333-333333333333')
ON CONFLICT (work_id) DO NOTHING;

INSERT INTO comments (id, thread_id, kind, replied_to_reply, author_id, body, review, moderation_version, admission_policy, admission_reason)
SELECT '3ac70000-0000-4000-8000-000000000002', id, 'root', false,
       '22222222-2222-4222-8222-222222222222', '[Mẫu #328] Một chiều rất khẽ.', 'pending', 1, 'premoderation.v1', 'policy_premoderation'
FROM discussion_threads
WHERE work_id = '33333333-3333-4333-8333-333333333333';
UPDATE comments SET review = 'approved', moderation_version = 2
WHERE id = '3ac70000-0000-4000-8000-000000000002';

INSERT INTO works (id, owner_id)
VALUES ('3ac70000-0000-4000-8000-000000000003', '11111111-1111-4111-8111-111111111111');
INSERT INTO revisions (id, work_id, title, document, content_hash, created_by)
SELECT '3ac70000-0000-4000-8000-000000000004', '3ac70000-0000-4000-8000-000000000003',
       r.title, r.document, r.content_hash, r.created_by
FROM revisions r JOIN publications p ON p.revision_id = r.id
WHERE p.work_id = '33333333-3333-4333-8333-333333333333';
INSERT INTO publications (work_id, revision_id, generation, published_at)
VALUES ('3ac70000-0000-4000-8000-000000000003', '3ac70000-0000-4000-8000-000000000004', 1, now());
INSERT INTO discussion_threads (id, work_id, comments_open)
VALUES ('3ac70000-0000-4000-8000-000000000005', '3ac70000-0000-4000-8000-000000000003', false);
COMMIT;
