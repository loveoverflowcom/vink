# Reputation — registry và bàn giao #600

Base đọc qua GitHub connector: `91191186c1ac5ec6f4032dc0292af025eb2c0458`. Không có local checkout, cargo, DB hay benchmark trong lượt
ghi tài liệu này. Không xác nhận production. Implementation: chưa kiểm toán đầy đủ owner/callers,
UNKNOWN; epic này chưa triển khai runtime. Evidence: source/document inspection only; NOT_RUN
cho decision CLI, Quint, Kani, PostgreSQL, HTTP và performance.

- [PRD §19](../../prd/requirements.md#19-điểm-vink-reputation-v1) sở hữu requirements REP-01..10.
- [VDR-REPUTATION@r1](../../decisions/proposals/VDR-REPUTATION.r1.toml): lựa chọn nghiệp vụ Q01..15
  đã có lời chủ sản phẩm; formal registry PENDING. Không active marker hoặc signoff tự tạo.
- [VDR-REPUTATION-CONTRACT@r1](../../decisions/proposals/VDR-REPUTATION-CONTRACT.r1.toml): PENDING,
  kiến trúc DESIGN_PROPOSED; [domain](../architecture/reputation.md) và
  [API](../../services/vink-app/contracts/http/v1/reputation.md).
- Source authority audit còn phải hoàn thành tại HEAD: domain/persistence/endorsement.rs,
  publication/moderation/discussion, account_deletion.rs, principal standing, outbox.rs,
  bin/vink-worker.rs, profile read owners. Follow chỉ là nguồn bị loại khỏi công thức.
  Đối chiếu #73, #284, #297, #305, #365; không suy đủ nguồn từ notifications.

## Slices đề xuất (6 PR, chưa tạo child issues)

| Slice | Owner/phạm vi | Gate/evidence |
| --- | --- | --- |
| REP-S01 | Shape, science, SSOT | Q01..15 + OPEN lifecycle; candidate simulations; formula/eligibility/privacy decisions; CLI validate/propose/query/signoff đúng revision; không kích hoạt. |
| REP-S02 | Pure Rust Domain | checked facts/entitlement/growth/policy versions; golden/property vectors, Quint replay và Kani finite invariants; không claim proof float. |
| REP-S03 | DB & accepted source events | additive migrations, transactional sources, no-op/retry/reversal/lifecycle, Q13 remove vs growth, Q14 actor standing/deletion isolation, privacy fence, tests PG race/rollback. |
| REP-S04 | Worker/current projection/recovery | lease/ack, batch, reconcile, replay/rebuild/version switch, backfill checkpoint, lag/invalidation. |
| REP-S05 | Period aggregation & statistics | current/as-of close top rating + weekly/monthly growth (current/closed), signed old-interaction withdrawal and same-period re-like compensation, independent week/month boundaries, event-time/coverage, 52 closed weeks/12 closed months + open periods, expiry/purge/replay, retention-independent current facts, version/privacy, comment-only eligibility, anti-gaming, bounded indexed reads. |
| REP-S06 | API & qualification | real auth/guest, read guard and cache, bounded queries, end-to-end PG/HTTP, isolation and benchmark, rollout/runbook. |

Sequential dependencies S01 -> S02 -> S03 -> S04 -> S05 -> S06; product gates before exposure.
S01 is not complete just because this draft is opened. Design and Leptos Web/CMP are separate epic.
No merge/deploy/acceptance checkbox from document-only evidence.
Review delta lượt 3–5: Q07..15 đã có lời chọn; cập nhật draft #601 trước signoff/merge, chưa chạy CLI.
Các revision r1 vẫn là payload draft chưa phát hành; giữ lịch sử commit, không ghi signoff hoặc active marker.

## Review queue

- Product: comment edit/tombstone eligibility, verified invalidation/closed-period correction and
  account deletion growth attribution; eligibility at author/actor level; repeated toggles/nonlinear
  attribution, initial coverage, thresholds beyond Q11 and version policy. Q13 selects lifecycle remove
  outside growth; Q14 preserves valid endorsements of suspended actors; Q15 selects period retention
  (raw source/ledger/fence retention still open, subject to DEL/privacy).
- Science: formula/weights/baseline, anti-abuse and new-author fairness, simulation.
- Technical: event completeness and ownership, persistence and versions, retention/purge,
  late arrival/cutover, backfill, indexes and API freshness. Benchmark hot principals and
  high-frequency events; p50/p95/p99 reads and source writes, WAL/locks, storage, worker lag.
  Epic proposed p95 write delta <=5%, lag <2m are study targets, not SLA.
- Local next: cargo xtask decision query/validate/diff, review exact SHA; inspect commands
  and scoped workflows on actual checkout. No registry hash or signoff claimed here.
