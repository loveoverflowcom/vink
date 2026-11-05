-- LVE PostgreSQL evidence dataset (#73). Run on a SCRATCH database after every App migration,
-- never on a shared or production database. Deterministic: setseed() fixes random().
--
-- Shape: 20 000 principals; one hot public work (3 revisions, publication at r3) with 15 000
-- work endorsements (6 000 effective at r3, 5 000 at r2, 2 000 at r1, 2 000 cleared); 200 other
-- public works with 50 endorsements each; the hot work's thread has 500 roots and 1 500 replies
-- (80 % approved, 10 % pending, 5 % rejected, 5 % removed by moderation); comment endorsements
-- are skewed: one hot root with 5 000 rows (4 000 selected), then about 2 000/(rank) per comment, with
-- one row in five cleared.
SELECT setseed(0.73);

CREATE FUNCTION pg_temp.pid(n int) RETURNS uuid LANGUAGE sql IMMUTABLE
    AS $$ SELECT ('00000000-0000-4000-8000-' || lpad(n::text, 12, '0'))::uuid $$;
CREATE FUNCTION pg_temp.wid(n int) RETURNS uuid LANGUAGE sql IMMUTABLE
    AS $$ SELECT ('10000000-0000-4000-8000-' || lpad(n::text, 12, '0'))::uuid $$;
CREATE FUNCTION pg_temp.rid(w int, r int) RETURNS uuid LANGUAGE sql IMMUTABLE
    AS $$ SELECT ('20000000-0000-4000-8' || lpad(r::text, 3, '0') || '-' || lpad(w::text, 12, '0'))::uuid $$;
CREATE FUNCTION pg_temp.cid(n int) RETURNS uuid LANGUAGE sql IMMUTABLE
    AS $$ SELECT ('30000000-0000-4000-8000-' || lpad(n::text, 12, '0'))::uuid $$;

INSERT INTO principals (id, display_name)
SELECT pg_temp.pid(n), 'Bạn đọc ' || n FROM generate_series(1, 20000) n;

-- Work 0 is hot; works 1..200 are background. Owners are principals 20000 - w.
INSERT INTO works (id, owner_id) SELECT pg_temp.wid(w), pg_temp.pid(20000 - w) FROM generate_series(0, 200) w;
INSERT INTO revisions (id, work_id, title, document, content_hash, created_by)
SELECT pg_temp.rid(w, r), pg_temp.wid(w), 'Tác phẩm ' || w || ' bản ' || r,
       '{"schema_version": 1, "blocks": [{"lines": ["Dòng thơ mẫu."]}]}'::jsonb,
       md5(w || '-' || r) || md5(r || '-' || w), pg_temp.pid(20000 - w)
  FROM generate_series(0, 200) w, generate_series(1, 3) r
 WHERE w = 0 OR r = 1;
INSERT INTO publications (work_id, revision_id, generation, published_at)
SELECT pg_temp.wid(w), pg_temp.rid(w, CASE WHEN w = 0 THEN 3 ELSE 1 END), 1, now()
  FROM generate_series(0, 200) w;

-- Hot work endorsements: readers 1..15000.
INSERT INTO work_endorsements (work_id, actor_id, revision_id, version)
SELECT pg_temp.wid(0), pg_temp.pid(n),
       CASE WHEN n <= 6000 THEN pg_temp.rid(0, 3) WHEN n <= 11000 THEN pg_temp.rid(0, 2)
            WHEN n <= 13000 THEN pg_temp.rid(0, 1) ELSE NULL END,
       2 + (n % 5)
  FROM generate_series(1, 15000) n;
INSERT INTO work_endorsements (work_id, actor_id, revision_id, version)
SELECT pg_temp.wid(w), pg_temp.pid(n), pg_temp.rid(w, 1), 2
  FROM generate_series(1, 200) w, generate_series(1, 50) n;

-- Thread and comments of the hot work. Comments 1..500 are roots; 501..2000 reply to root
-- ((n - 501) % 500) + 1. States are drawn from random() under the fixed seed.
INSERT INTO discussion_threads (id, work_id) VALUES ('40000000-0000-4000-8000-000000000000', pg_temp.wid(0));
WITH drawn AS (
    SELECT n, random() AS r FROM generate_series(1, 2000) n
)
INSERT INTO comments (id, thread_id, kind, root_id, root_kind, reply_to_id, replied_to_reply,
                      author_id, body, review, removal, moderation_version, created_at)
SELECT pg_temp.cid(n), '40000000-0000-4000-8000-000000000000',
       CASE WHEN n <= 500 THEN 'root' ELSE 'reply' END,
       CASE WHEN n <= 500 THEN NULL ELSE pg_temp.cid(((n - 501) % 500) + 1) END,
       CASE WHEN n <= 500 THEN NULL ELSE 'root' END,
       NULL, false, pg_temp.pid(19000 + (n % 700)), 'Góp ý mẫu ' || n,
       CASE WHEN n = 1 OR r < 0.80 THEN 'approved' WHEN r < 0.90 THEN 'pending'
            WHEN r < 0.95 THEN 'rejected' ELSE 'approved' END,
       CASE WHEN n <> 1 AND r >= 0.95 THEN 'moderation' END,
       CASE WHEN n = 1 OR r < 0.80 THEN 2 WHEN r < 0.90 THEN 1 ELSE 2 END,
       now() - make_interval(secs => 2000 - n)
  FROM drawn;

-- Skewed comment endorsements: comment 1 gets 5 000 actors, comment k about 2000/k.
INSERT INTO comment_endorsements (comment_id, actor_id, selected, version)
SELECT pg_temp.cid(k), pg_temp.pid(a), (a % 5) <> 0, 2
  FROM generate_series(1, 2000) k,
       LATERAL generate_series(1, CASE WHEN k = 1 THEN 5000 ELSE greatest(1, 2000 / k) END) a;

ANALYZE;
