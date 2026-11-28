# Reputation API v1 — draft, not implemented

[PRD §19](../../../../../prd/requirements.md#19-điểm-vink-reputation-v1) /
[domain](../../../../../docs/architecture/reputation.md) /
[registry](../../../../../docs/development/reputation.md). Q01..44 product direction; routes/DTO/error mapping OPEN.

| Capability | Contract |
| --- | --- |
| Current total | One ready integer Điểm Vink; true zero-fact baseline 1000, version/freshness/availability; PK projection, no raw components or voter data. |
| Top Reputation | Eligible contribution (not baseline-only, comment-only allowed), open snapshot current as_of or closed as-of-end rating, never peak/current substitute. |
| Top Growth | Fixed-unit signed net community credit Q41, exact tenths; >0 and >=3 distinct net-positive eligible actors after privacy/correction; no actor counts/IDs; independent calendar week/month. |

Top100 default20 max100 per page; canonical unrounded score desc PrincipalId asc. Calendar [start,end),
Asia/Ho_Chi_Minh, snapshot id/generation/as_of, formula/policy/metric version, coverage/correction revision.
First full week/month independently; no fake preactivation/partial period. Retain open + 52 closed weeks/12 months.
Open snapshot target 5–10m not SLA; closed reserve 50 internal, exact full-population refill/rebuild if incomplete
or ordering changes; no false-complete. Cursor bound snapshot/version/visibility, reject/restart stale.
Guest/member same public visibility, current authority before limit/cache; withhold unsafe stale projection,
suspended/deleted recipient omitted all periods. Privacy/deletion/correction may restate history; no raw admin evidence.
Q38 authorized CLI only; no public writes, source counts unchanged. Q30/Q42 gate/maturity private.
Distinguish disabled/unavailable/incomplete/expired/forbidden/deleted from true baseline or zero.
No N+1/global rank/source aggregation per request. Exact signoff/qualification pending; HTTP/runtime NOT_RUN.
