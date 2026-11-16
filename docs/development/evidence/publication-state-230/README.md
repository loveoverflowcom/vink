# Evidence #230

Owner, caller/integrity map, commands and acceptance:
[publication-state-read.md](../../publication-state-read.md).

- [before.json](before.json): PostgreSQL/app baseline, runtime `20ed13e` + new benchmark only.
- [after.json](after.json): same measurements at implementation `0147bda`.
- [runtime-sql.json](runtime-sql.json): selected actual PostgreSQL log statement sequences, 5 SELECT
  per owner read; after has an additional read-only REPEATABLE READ SET before owner SELECT.
  Parameters are disabled in the raw log. Log query examples are independent of EXPLAIN strings.
- [environment.json](environment.json): host/flags/source/seed/hashes and measurement limits.
- [decisions.json](decisions.json): actual CLI lookups USER-02/ADM-06/USER-09 with source and snapshot.
- [known-policy-rereview.json](known-policy-rereview.json) và [reproducer](known-policy-rereview.rs.txt):
  additional old-approval policy re-review flow FAIL; unchanged source mismatch, outside projection.
- [checks.json](checks.json): exact commands/status/verification boundaries and initial failures.
  Final [quick.log](quick.log), [postgres.log](postgres.log),
  [benchmark-before.log](benchmark-before.log), [benchmark-after.log](benchmark-after.log).
- [preservation.json](preservation.json): before/after #204 inventory digest and local cleanup boundary.

Each measurement case retains first/warm arrays, response bytes, stored/logical document bytes,
and EXPLAIN ANALYZE BUFFERS (full then metadata SELECT). These SELECT plans do not force JSONB
output/decoding. No claim of wire bytes, cold-cache timing, p95, production latency or browser QA.
Fresh SQLx databases have variable timestamp fractional precision; response byte deltas are kept.

Logs/data are outside Cargo target in `/tmp/vink-230-*`. The scratch cluster is stopped after
verification; existing #204 PostgreSQL and Chrome are preserved. No credentials, private
manuscripts, Chrome profiles or database directories are committed.
