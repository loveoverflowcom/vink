"""p50/p95/p99 of repeated statements with pgbench (-M prepared, 4 clients, 10 s each) and the WAL
written by 2000 submit+approve cycles. Usage: python3 -I latency.py LABEL"""
import os
import re
import statistics
import subprocess
import sys
import glob

HERE = os.path.dirname(os.path.abspath(__file__))
CONN = ["-h", "127.0.0.1", "-p", "55432", "-U", "vink_test"]
PSQL = ["psql", "-X", "-q", "-At"] + CONN + ["vink_bench"]
HOT = "44444444-4444-4444-8444-444444444444"
P = "00000000-0000-4000-8000-000000000002"


def scalar(sql):
    return subprocess.run(PSQL + ["-c", sql], capture_output=True, text=True, check=True).stdout.strip()


queries = open(os.path.join(HERE, "queries.sql")).read()
statements = dict(re.findall(r"PREPARE (\w+)\([^)]*\) AS\n(.*?);\n", queries, re.S))
first20 = "{" + ",".join(scalar(
    f"SELECT id FROM comments WHERE thread_id = '{HOT}' AND kind = 'root' AND removal IS NULL "
    "AND review = 'approved' ORDER BY created_at, id LIMIT 20").split()) + "}"


def bind(name, *args):
    sql = statements[name]
    for i, arg in reversed(list(enumerate(args, 1))):
        sql = sql.replace(f"${i}", arg)
    return " ".join(sql.split()) + ";"


WORKLOADS = {
    "page.guest.20": [bind("roots", f"'{HOT}'::uuid", "NULL::uuid", "NULL::timestamptz", "NULL::uuid", "21"),
                      bind("previews", f"'{first20}'::uuid[]", "NULL::uuid", "4")],
    "own.list.P.20": [bind("own_list", f"'{HOT}'::uuid", f"'{P}'::uuid", "NULL::timestamptz", "NULL::uuid", "21")],
}

label = sys.argv[1]
for name, stmts in WORKLOADS.items():
    script = os.path.join(HERE, f"pgbench-{name}.sql")
    open(script, "w").write("\n".join(stmts) + "\n")
    prefix = os.path.join(HERE, f"log-{label}-{name}")
    for old in glob.glob(prefix + "*"):
        os.remove(old)
    subprocess.run(["pgbench", "-n", "-M", "prepared", "-c", "4", "-j", "4", "-T", "10",
                    "-f", script, "--log", "--log-prefix", prefix] + CONN + ["vink_bench"],
                   capture_output=True, text=True, check=True)
    lat = []
    for log in glob.glob(prefix + "*"):
        for line in open(log):
            lat.append(int(line.split()[2]) / 1000.0)
    lat.sort()
    q = lambda p: lat[min(len(lat) - 1, int(p * len(lat)))]
    print(f"{label:14} {name:16} n={len(lat):6} p50={q(0.50):.3f}ms p95={q(0.95):.3f}ms "
          f"p99={q(0.99):.3f}ms mean={statistics.mean(lat):.3f}ms")

# Write amplification: WAL for 2000 comments born pending then approved (one transaction each
# step in bulk; rolled back afterwards, WAL is still written).
wal = scalar(f"""
DO $$ BEGIN PERFORM 1; END $$;
SELECT 1;
""")
start = scalar("SELECT pg_current_wal_lsn()")
subprocess.run(PSQL, input=f"""
BEGIN;
CREATE TEMP TABLE w AS SELECT gen_random_uuid() AS id, g FROM generate_series(1, 2000) g;
INSERT INTO comments (id, thread_id, kind, replied_to_reply, author_id, body, review,
                      moderation_version, admission_policy, admission_reason)
SELECT id, '88888888-0000-4000-8000-000000000001', 'root', false,
       '00000000-0000-4000-8000-000000000003', 'WAL ' || g, 'pending', 1,
       'premoderation.v1', 'policy_premoderation' FROM w;
UPDATE comments SET review = 'approved', moderation_version = 2 WHERE id IN (SELECT id FROM w);
ROLLBACK;
""", capture_output=True, text=True, check=True)
end = scalar("SELECT pg_current_wal_lsn()")
diff = scalar(f"SELECT pg_wal_lsn_diff('{end}', '{start}')")
print(f"{label:14} wal.2000.submit+approve  {int(float(diff)) / 1024:.0f} KiB")
