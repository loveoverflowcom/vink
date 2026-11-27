# Reputation API v1 — contract draft, not implemented by this change

[PRD §19](../../../../../prd/requirements.md#19-điểm-vink-reputation-v1) /
[domain](../../../../../docs/architecture/reputation.md) /
[registry](../../../../../docs/development/reputation.md). Route names, DTO shape, page limits and
error mapping OPEN until audit of actual HTTP v1 owner; do not generate FE bindings yet.

| Read capability | Candidate contract |
| --- | --- |
| Current total | One ready integer rating (scale/rounding OPEN), formula version, availability/freshness. Never raw component counts or eligible voter identities. |
| High reputation | Bounded, stable ordered query of eligible authors by current total. Whether to expose period snapshot of high rating is OPEN. |
| Fast growth | Bounded weekly/monthly calendar period queries, typed period boundaries and version/readiness; signed net eligible new contribution metric minus withdrawals of old interactions in the withdrawal period, not naive score delta. Support current and retained closed periods; no fake historical coverage. |

Public guest and authenticated readers of eligible data have the same public visibility.
Check current identity/standing/content privacy fences at read; invalidate or withhold unsafe
stale projections and caches including historical growth. A fully suspended recipient is omitted
from public total and all public statistics (including closed periods) until restored and eligible;
no score reset. No raw component or private signals. Period availability/coverage/retention and
version/correction contract OPEN; distinguish unavailable/expired from zero growth.
Unready/disabled/forbidden/deleted differ from legitimate baseline; no unsafe stale fallback.
Use indexed projection reads with deterministic tie/cursor candidate, no per-author source
aggregation or N+1; profile integration vs dedicated API OPEN. API read only; no arbitrary
score write endpoint. No UI, no production activation until formal product/architecture
and qualification gates. Actual endpoint/DTO tests and performance evidence NOT_RUN.
