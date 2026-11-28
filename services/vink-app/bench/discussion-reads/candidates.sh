#!/bin/sh
# Measures each candidate index alone against the baseline: create, analyze, explain, size, drop.
set -e
cd "$(dirname "$0")"
export LC_ALL=en_US.UTF-8
PSQL="psql -X -q -At -h 127.0.0.1 -p 55432 -U vink_test vink_bench"

run_candidate() {
    name="$1"
    ddl="$2"
    echo "=== candidate $name: $ddl"
    $PSQL -c "$ddl"
    $PSQL -c "ANALYZE comments"
    $PSQL -c "SELECT 'index size: ' || pg_size_pretty(pg_relation_size('bench_idx'))"
    python3 -I explain.py "$name"
    $PSQL -c "DROP INDEX bench_idx"
}

run_candidate own_partial "CREATE INDEX bench_idx ON comments (author_id, thread_id, created_at, id) WHERE removal IS NULL AND review <> 'approved'"
run_candidate own_full "CREATE INDEX bench_idx ON comments (author_id, thread_id, created_at, id)"
run_candidate public_replies "CREATE INDEX bench_idx ON comments (root_id, created_at, id) WHERE kind = 'reply' AND review = 'approved' AND removal IS NULL"
echo "=== sizes of existing comment indexes"
$PSQL -c "SELECT indexrelname || ': ' || pg_size_pretty(pg_relation_size(indexrelid)) FROM pg_stat_user_indexes WHERE relname = 'comments' ORDER BY 1"
