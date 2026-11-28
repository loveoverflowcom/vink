-- The statements PgStore issues (services/vink-app/src/persistence/endorsement.rs), with the
-- dataset's IDs as literals (a custom plan, as for the first executions of a prepared
-- statement): the hot work, its hot root, and a default discussion page of 80 comment IDs
-- (20 roots + 60 replies, mixed states, hot root included). Run after dataset.sql with
--   psql -X -v ON_ERROR_STOP=1 -f queries.sql
\set work '10000000-0000-4000-8000-000000000000'
\set hot '30000000-0000-4000-8000-000000000001'
\set actor '00000000-0000-4000-8000-000000000042'
SELECT '{' || string_agg('30000000-0000-4000-8000-' || lpad(n::text, 12, '0'), ',') || '}' AS page
  FROM (SELECT generate_series(1, 20) AS n UNION ALL SELECT 500 + generate_series(1, 60)) ids \gset

\echo '== 1. summary: work + effective count'
EXPLAIN (ANALYZE, BUFFERS, COSTS OFF, TIMING OFF)
SELECT w.id, w.owner_id, p.revision_id AS published,
       (SELECT count(*) FROM work_endorsements e
         WHERE e.work_id = w.id AND e.revision_id = p.revision_id AND e.selected) AS effective,
       NULL::boolean AS selected, NULL::bigint AS version, NULL::bigint AS accepted_order
  FROM works w LEFT JOIN publications p ON p.work_id = w.id
 WHERE w.id = :'work';

\echo '== 2. summary: 80 comments of the work with counts'
EXPLAIN (ANALYZE, BUFFERS, COSTS OFF, TIMING OFF)
SELECT c.id, c.root_id, c.reply_to_id, c.author_id, c.review, c.removal, c.moderation_version,
       t.id AS thread_id, t.work_id,
       (SELECT count(*) FROM comment_endorsements e
         WHERE e.comment_id = c.id AND e.selected) AS endorsements,
       NULL::boolean AS selected, NULL::bigint AS version, NULL::bigint AS accepted_order
  FROM comments c JOIN discussion_threads t ON t.id = c.thread_id
 WHERE c.id = ANY(:'page'::uuid[]) AND t.work_id = :'work';

\echo '== 3. own state: work + the actor row'
EXPLAIN (ANALYZE, BUFFERS, COSTS OFF, TIMING OFF)
SELECT w.id, w.owner_id, p.revision_id AS published, NULL::bigint AS effective,
       e.selected, e.version, e.accepted_order
  FROM works w
  LEFT JOIN publications p ON p.work_id = w.id
  LEFT JOIN work_endorsements e ON e.work_id = w.id
        AND e.revision_id = p.revision_id AND e.actor_id = :'actor'
 WHERE w.id = :'work';

\echo '== 4. own state: 80 comments with the actor rows'
EXPLAIN (ANALYZE, BUFFERS, COSTS OFF, TIMING OFF)
SELECT c.id, c.root_id, c.reply_to_id, c.author_id, c.review, c.removal, c.moderation_version,
       t.id AS thread_id, t.work_id,
       NULL::bigint AS endorsements, e.selected, e.version, e.accepted_order
  FROM comments c
  JOIN discussion_threads t ON t.id = c.thread_id
  LEFT JOIN comment_endorsements e ON e.comment_id = c.id AND e.actor_id = :'actor'
 WHERE c.id = ANY(:'page'::uuid[]) AND t.work_id = :'work';

\echo '== 5. write: effective count of the hot work after a write'
EXPLAIN (ANALYZE, BUFFERS, COSTS OFF, TIMING OFF)
SELECT count(*) FROM work_endorsements
 WHERE work_id = :'work' AND revision_id = '20000000-0000-4000-8003-000000000000' AND selected;

\echo '== 6. write: selected count of the hot root after a write'
EXPLAIN (ANALYZE, BUFFERS, COSTS OFF, TIMING OFF)
SELECT count(*) FROM comment_endorsements WHERE comment_id = :'hot' AND selected;

\echo '== 7. write: the actor row under lock'
EXPLAIN (ANALYZE, BUFFERS, COSTS OFF, TIMING OFF)
SELECT selected, version, accepted_order FROM comment_endorsements
 WHERE comment_id = :'hot' AND actor_id = :'actor' FOR UPDATE;
