-- Reproducible discussion benchmark dataset for #120 DISC-02/03 (synthetic, no real content).
-- Hot thread: the dev-seed sample work 33333333-…, 2000 roots, skewed replies, mixed states.
-- Background: 300 unpublished works with 300 comments each, so the comments table is not tiny.
-- Viewer A = principal 00000000-0000-4000-8000-000000000001 owns ~0.3% of the hot thread.
\set ON_ERROR_STOP 1
SELECT setseed(0.120203);

INSERT INTO principals (id, display_name)
SELECT ('00000000-0000-4000-8000-' || lpad(to_hex(g), 12, '0'))::uuid, 'Người đọc ' || g
  FROM generate_series(1, 5000) g;
INSERT INTO principals (id, display_name)
VALUES ('55555555-5555-4555-8555-555555555555', 'Kiểm duyệt viên');
INSERT INTO discussion_moderator_grants (id, principal_id, note)
VALUES ('77777777-7777-4777-8777-777777777777', '55555555-5555-4555-8555-555555555555', 'bench');

-- Threads: hot one plus background works without publication.
INSERT INTO discussion_threads (id, work_id)
VALUES ('44444444-4444-4444-8444-444444444444', '33333333-3333-4333-8333-333333333333');
INSERT INTO works (id, owner_id)
SELECT ('99999999-0000-4000-8000-' || lpad(to_hex(g), 12, '0'))::uuid,
       ('00000000-0000-4000-8000-' || lpad(to_hex(1 + (g % 5000)), 12, '0'))::uuid
  FROM generate_series(1, 300) g;
INSERT INTO discussion_threads (id, work_id)
SELECT ('88888888-0000-4000-8000-' || lpad(to_hex(g), 12, '0'))::uuid,
       ('99999999-0000-4000-8000-' || lpad(to_hex(g), 12, '0'))::uuid
  FROM generate_series(1, 300) g;

CREATE TEMP TABLE plan_roots AS
SELECT g AS n, gen_random_uuid() AS id,
       timestamp with time zone '2026-01-01 00:00:00+00' + make_interval(secs => g * 60) AS at,
       CASE WHEN random() < 0.003 THEN '00000000-0000-4000-8000-000000000001'::uuid
            ELSE ('00000000-0000-4000-8000-' || lpad(to_hex(2 + floor(random() * 4999)::int), 12, '0'))::uuid
       END AS author,
       random() AS state_roll,
       CASE WHEN g % 20 = 0 THEN 200             -- 5% hot roots
            WHEN g % 4 = 0 THEN 1 + (g % 5)      -- ~20% sparse
            ELSE 0 END AS replies
  FROM generate_series(1, 2000) g;

INSERT INTO comments (id, thread_id, kind, replied_to_reply, author_id, body, review,
                      moderation_version, admission_policy, admission_reason, created_at)
SELECT id, '44444444-4444-4444-8444-444444444444', 'root', false, author,
       'Gốc thử nghiệm số ' || n, 'pending', 1, 'premoderation.v1', 'policy_premoderation', at
  FROM plan_roots;

CREATE TEMP TABLE plan_replies AS
SELECT gen_random_uuid() AS id, r.id AS root_id,
       r.at + make_interval(secs => k) AS at,
       CASE WHEN random() < 0.003 THEN '00000000-0000-4000-8000-000000000001'::uuid
            ELSE ('00000000-0000-4000-8000-' || lpad(to_hex(2 + floor(random() * 4999)::int), 12, '0'))::uuid
       END AS author,
       random() AS state_roll, k
  FROM plan_roots r CROSS JOIN LATERAL generate_series(1, r.replies) k;

INSERT INTO comments (id, thread_id, kind, root_id, root_kind, replied_to_reply, author_id, body,
                      review, moderation_version, admission_policy, admission_reason, created_at)
SELECT id, '44444444-4444-4444-8444-444444444444', 'reply', root_id, 'root', false, author,
       'Trả lời thử nghiệm ' || k, 'pending', 1, 'premoderation.v1', 'policy_premoderation', at
  FROM plan_replies;

-- Background threads: 300 x (60 roots + 240 replies spread over the roots).
CREATE TEMP TABLE plan_bg_roots AS
SELECT gen_random_uuid() AS id, t, g,
       ('88888888-0000-4000-8000-' || lpad(to_hex(t), 12, '0'))::uuid AS thread_id,
       timestamp with time zone '2026-01-01 00:00:00+00' + make_interval(secs => t * 1000 + g) AS at
  FROM generate_series(1, 300) t CROSS JOIN generate_series(1, 60) g;
INSERT INTO comments (id, thread_id, kind, replied_to_reply, author_id, body, review,
                      moderation_version, admission_policy, admission_reason, created_at)
SELECT id, thread_id, 'root', false,
       ('00000000-0000-4000-8000-' || lpad(to_hex(1 + ((t * 61 + g) % 5000)), 12, '0'))::uuid,
       'Nền ' || g, 'pending', 1, 'premoderation.v1', 'policy_premoderation', at
  FROM plan_bg_roots;
INSERT INTO comments (id, thread_id, kind, root_id, root_kind, replied_to_reply, author_id, body,
                      review, moderation_version, admission_policy, admission_reason, created_at)
SELECT gen_random_uuid(), r.thread_id, 'reply', r.id, 'root', false,
       ('00000000-0000-4000-8000-' || lpad(to_hex(1 + ((r.t * 7 + r.g * 13 + k) % 5000)), 12, '0'))::uuid,
       'Nền trả lời', 'pending', 1, 'premoderation.v1', 'policy_premoderation',
       r.at + make_interval(secs => k)
  FROM plan_bg_roots r CROSS JOIN generate_series(1, 4) k;

-- Review outcomes, one decision step each (version 1 -> 2), as a moderator would write them.
-- Roots: 80% approved, 10% pending, 5% rejected, 5% approved then removed by moderation.
UPDATE comments c SET review = s.review, removal = s.removal, moderation_version = 2
  FROM (SELECT id,
               CASE WHEN state_roll < 0.80 THEN 'approved' WHEN state_roll < 0.90 THEN 'pending'
                    WHEN state_roll < 0.95 THEN 'rejected' ELSE 'approved' END AS review,
               CASE WHEN state_roll >= 0.95 THEN 'moderation' END AS removal
          FROM plan_roots) s
 WHERE c.id = s.id AND NOT (s.review = 'pending' AND s.removal IS NULL);
-- Replies: 85% approved, 8% pending, 4% rejected, 3% removed by moderation.
UPDATE comments c SET review = s.review, removal = s.removal, moderation_version = 2
  FROM (SELECT id,
               CASE WHEN state_roll < 0.85 THEN 'approved' WHEN state_roll < 0.93 THEN 'pending'
                    WHEN state_roll < 0.97 THEN 'rejected' ELSE 'approved' END AS review,
               CASE WHEN state_roll >= 0.97 THEN 'moderation' END AS removal
          FROM plan_replies) s
 WHERE c.id = s.id AND NOT (s.review = 'pending' AND s.removal IS NULL);
UPDATE comments SET review = 'approved', moderation_version = 2
 WHERE thread_id <> '44444444-4444-4444-8444-444444444444' AND random() < 0.9;

INSERT INTO comment_moderation_decisions (comment_id, version, action, moderator_id, grant_id, reason)
SELECT id, 2,
       CASE WHEN removal IS NOT NULL THEN 'remove' WHEN review = 'rejected' THEN 'reject' ELSE 'approve' END,
       '55555555-5555-4555-8555-555555555555', '77777777-7777-4777-8777-777777777777',
       CASE WHEN removal IS NOT NULL OR review = 'rejected' THEN 'Lý do thử nghiệm.' END
  FROM comments WHERE moderation_version = 2;

VACUUM (ANALYZE) comments;
VACUUM (ANALYZE) comment_moderation_decisions;
VACUUM (ANALYZE) principals;
SELECT kind, review, removal, count(*) FROM comments
 WHERE thread_id = '44444444-4444-4444-8444-444444444444'
 GROUP BY 1, 2, 3 ORDER BY 1, 2, 3;
SELECT count(*) AS total_comments FROM comments;
SELECT count(*) AS own_unpublished_of_a FROM comments
 WHERE thread_id = '44444444-4444-4444-8444-444444444444'
   AND author_id = '00000000-0000-4000-8000-000000000001'
   AND removal IS NULL AND review <> 'approved';
