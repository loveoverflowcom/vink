-- Optional second scale, run after dataset.sql on another scratch database: one million more
-- comment endorsements (actors 5001..5500 on every one of the 2 000 comments, 4 in 5 selected),
-- to see the own-state and summary plans when the table no longer fits a few hundred pages.
INSERT INTO comment_endorsements (comment_id, actor_id, selected, version, accepted_order)
SELECT ('30000000-0000-4000-8000-' || lpad(k::text, 12, '0'))::uuid,
       ('00000000-0000-4000-8000-' || lpad(a::text, 12, '0'))::uuid, (a % 5) <> 0, 2, 3
  FROM generate_series(1, 2000) k, generate_series(5001, 5500) a;
ANALYZE;
