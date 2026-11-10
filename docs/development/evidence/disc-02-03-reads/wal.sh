#!/bin/sh
# WAL bytes of 2000 comments born pending then approved, after a CHECKPOINT each time (so both
# configurations pay the same full-page writes). Three repetitions per configuration.
set -e
export LC_ALL=en_US.UTF-8
P="psql -X -q -At -h 127.0.0.1 -p 55432 -U vink_test vink_bench"
measure() {
    $P -c "CHECKPOINT"
    start=$($P -c "SELECT pg_current_wal_lsn()")
    $P <<'SQL'
BEGIN;
CREATE TEMP TABLE w AS SELECT gen_random_uuid() AS id, g FROM generate_series(1, 2000) g;
INSERT INTO comments (id, thread_id, kind, replied_to_reply, author_id, body, review,
                      moderation_version, admission_policy, admission_reason)
SELECT id, '88888888-0000-4000-8000-000000000001', 'root', false,
       '00000000-0000-4000-8000-000000000003', 'WAL ' || g, 'pending', 1,
       'premoderation.v1', 'policy_premoderation' FROM w;
UPDATE comments SET review = 'approved', moderation_version = 2 WHERE id IN (SELECT id FROM w);
ROLLBACK;
SQL
    end=$($P -c "SELECT pg_current_wal_lsn()")
    $P -c "SELECT '$1: ' || pg_wal_lsn_diff('$end', '$start')::bigint / 1024 || ' KiB'"
}
for i in 1 2 3; do measure baseline; done
$P -c "CREATE INDEX bench_idx ON comments (author_id, thread_id, created_at, id) WHERE removal IS NULL AND review <> 'approved'"
for i in 1 2 3; do measure own_partial; done
$P -c "DROP INDEX bench_idx"
