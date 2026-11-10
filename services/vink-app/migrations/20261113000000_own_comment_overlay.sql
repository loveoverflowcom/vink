-- Private overlay of a writer's own unpublished comments (#120 DISC-02/03,
-- docs/architecture/discussion-reads.md). The overlay reads `thread_id = $1 AND author_id = $2
-- AND removal IS NULL AND review <> 'approved'` in `(created_at, id)` order; without this index
-- PostgreSQL scans the whole `comments` table for it (measured: 11.8 ms p50 and ~5 800 buffers per
-- page on a 113 400-comment table, 0.22 ms and ~200 buffers with it).
--
-- Partial: only present, unpublished comments are entries, so the index stays small (928 kB on
-- that table versus 8.6 MB unfiltered). A new comment is born pending and enters it; a decision
-- that publishes or removes it leaves it. Those updates were already non-HOT because `review` and
-- `removal` are in the predicate of `comments_pending_idx`; measured WAL for 2000 submit+approve
-- cycles grew by about 9%.
--
-- Lock budget: plain CREATE INDEX takes SHARE on `comments` (reads continue, writes wait) while it
-- builds over the pilot's table. Backout: migrations-rollback/20261113000000_own_comment_overlay.down.sql.

CREATE INDEX comments_own_unpublished_idx ON comments (author_id, thread_id, created_at, id)
    WHERE removal IS NULL AND review <> 'approved';
