-- Viewer P = principal …0002 writes 120 pending/rejected comments in the hot thread (roots and
-- replies spread over the whole timeline), so the overlay needs several pages.
\set ON_ERROR_STOP 1
SELECT setseed(0.4);
CREATE TEMP TABLE p_rows AS
SELECT gen_random_uuid() AS id, g,
       (SELECT id FROM comments WHERE thread_id = '44444444-4444-4444-8444-444444444444'
          AND kind = 'root' AND review = 'approved' AND removal IS NULL
        ORDER BY created_at OFFSET (g * 13) % 1500 LIMIT 1) AS root_id
  FROM generate_series(1, 120) g;
INSERT INTO comments (id, thread_id, kind, root_id, root_kind, replied_to_reply, author_id, body,
                      review, moderation_version, admission_policy, admission_reason, created_at)
SELECT p.id, '44444444-4444-4444-8444-444444444444',
       CASE WHEN g % 3 = 0 THEN 'root' ELSE 'reply' END,
       CASE WHEN g % 3 = 0 THEN NULL ELSE p.root_id END,
       CASE WHEN g % 3 = 0 THEN NULL ELSE 'root' END,
       false, '00000000-0000-4000-8000-000000000002', 'Lời của P ' || g, 'pending', 1,
       'premoderation.v1', 'policy_premoderation',
       (SELECT created_at FROM comments WHERE id = p.root_id) + make_interval(secs => 30 + g)
  FROM p_rows p;
UPDATE comments SET review = 'rejected', moderation_version = 2
 WHERE author_id = '00000000-0000-4000-8000-000000000002' AND body LIKE '%5';
INSERT INTO comment_moderation_decisions (comment_id, version, action, moderator_id, grant_id, reason)
SELECT id, 2, 'reject', '55555555-5555-4555-8555-555555555555', '77777777-7777-4777-8777-777777777777', 'Lý do thử nghiệm.'
  FROM comments WHERE author_id = '00000000-0000-4000-8000-000000000002' AND review = 'rejected';
VACUUM (ANALYZE) comments;
SELECT review, count(*) FROM comments WHERE author_id = '00000000-0000-4000-8000-000000000002' GROUP BY 1;
