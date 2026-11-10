"""Runs EXPLAIN (ANALYZE, BUFFERS) on the production discussion statements over the bench dataset.

Usage: python3 -I explain.py LABEL  -> prints a table and writes explain-LABEL.json beside it.
Each workload runs 7 times in one session after one warm-up; the median execution time and the
buffers/rows of the median run are reported. Planning time is listed separately.
"""
import json
import os
import statistics
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
PSQL = ["psql", "-X", "-q", "-At", "-h", "127.0.0.1", "-p", "55432", "-U", "vink_test", "vink_bench"]
HOT = "44444444-4444-4444-8444-444444444444"
P = "00000000-0000-4000-8000-000000000002"
A = "00000000-0000-4000-8000-000000000001"
RUNS = 7


def scalar(sql):
    out = subprocess.run(PSQL + ["-c", sql], capture_output=True, text=True, check=True)
    return out.stdout.strip()


def ids(sql):
    return "{" + ",".join(scalar(sql).split()) + "}"


hot_root = scalar(
    f"SELECT root_id FROM comments WHERE thread_id = '{HOT}' AND kind = 'reply' "
    "GROUP BY root_id ORDER BY count(*) DESC, root_id LIMIT 1")
mid = scalar(
    f"SELECT created_at || '|' || id FROM comments WHERE thread_id = '{HOT}' AND kind = 'root' "
    "ORDER BY created_at, id OFFSET 1000 LIMIT 1").split("|")
mid_reply = scalar(
    f"SELECT created_at || '|' || id FROM comments WHERE root_id = '{hot_root}' "
    "ORDER BY created_at, id OFFSET 150 LIMIT 1").split("|")
first20 = ids(
    f"SELECT id FROM comments WHERE thread_id = '{HOT}' AND kind = 'root' AND removal IS NULL "
    "AND review = 'approved' ORDER BY created_at, id LIMIT 20")
first50 = ids(
    f"SELECT id FROM comments WHERE thread_id = '{HOT}' AND kind = 'root' AND removal IS NULL "
    "AND review = 'approved' ORDER BY created_at, id LIMIT 50")
hot50 = ids(
    f"SELECT root_id FROM comments WHERE thread_id = '{HOT}' AND kind = 'reply' "
    "GROUP BY root_id ORDER BY count(*) DESC, root_id LIMIT 50")
p20 = ids(f"SELECT id FROM comments WHERE author_id = '{P}' ORDER BY created_at, id LIMIT 20")

WORKLOADS = [
    ("roots.guest.20", f"EXECUTE roots('{HOT}', NULL, NULL, NULL, 21)"),
    ("roots.guest.50", f"EXECUTE roots('{HOT}', NULL, NULL, NULL, 51)"),
    ("roots.guest.20.deep", f"EXECUTE roots('{HOT}', NULL, '{mid[0]}', '{mid[1]}', 21)"),
    ("roots.writer.20", f"EXECUTE roots('{HOT}', '{P}', NULL, NULL, 21)"),
    ("previews.guest.20roots", f"EXECUTE previews('{first20}', NULL, 4)"),
    ("previews.guest.50roots", f"EXECUTE previews('{first50}', NULL, 4)"),
    ("previews.writer.50hot", f"EXECUTE previews('{hot50}', '{P}', 4)"),
    ("replies.hot.20", f"EXECUTE replies('{hot_root}', NULL, NULL, NULL, 21)"),
    ("replies.hot.20.deep", f"EXECUTE replies('{hot_root}', NULL, '{mid_reply[0]}', '{mid_reply[1]}', 21)"),
    ("own.list.P.20", f"EXECUTE own_list('{HOT}', '{P}', NULL, NULL, 21)"),
    ("own.list.A.20", f"EXECUTE own_list('{HOT}', '{A}', NULL, NULL, 21)"),
    ("own.exact.P.20", f"EXECUTE own_exact('{HOT}', '{P}', '{p20}')"),
]


def walk(node, acc):
    acc["hit"] += node.get("Shared Hit Blocks", 0) if node is acc["root"] else 0
    return acc


def run(stmt):
    script = [f"\\i {HERE}/queries.sql", "SET plan_cache_mode = force_custom_plan;", stmt + ";"]
    for _ in range(RUNS):
        script.append(f"EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON) {stmt};")
    out = subprocess.run(PSQL, input="\n".join(script), capture_output=True, text=True, check=True)
    plans = []
    buf = ""
    for line in out.stdout.splitlines():
        buf += line
        if buf.strip().startswith("[") and buf.strip().endswith("]"):
            try:
                plans.append(json.loads(buf)[0])
                buf = ""
            except json.JSONDecodeError:
                pass
        elif not buf.strip().startswith("["):
            buf = ""
    plans.sort(key=lambda p: p["Execution Time"])
    median = plans[len(plans) // 2]
    top = median["Plan"]
    return {
        "exec_ms": round(statistics.median(p["Execution Time"] for p in plans), 3),
        "plan_ms": round(statistics.median(p["Planning Time"] for p in plans), 3),
        "rows": top.get("Actual Rows"),
        "shared_hit": top.get("Shared Hit Blocks", 0),
        "shared_read": top.get("Shared Read Blocks", 0),
        "plan": median["Plan"],
    }


label = sys.argv[1]
results = {}
print(f"{'workload':28} {'exec ms':>9} {'plan ms':>8} {'rows':>5} {'hit':>7} {'read':>5}")
for name, stmt in WORKLOADS:
    r = run(stmt)
    results[name] = r
    print(f"{name:28} {r['exec_ms']:9.3f} {r['plan_ms']:8.3f} {r['rows']:5} {r['shared_hit']:7} {r['shared_read']:5}")
with open(os.path.join(HERE, f"explain-{label}.json"), "w") as f:
    json.dump(results, f, indent=1)
