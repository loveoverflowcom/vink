---
name: vink-publication
description: Implement or review Vink literary revisions, audiences, moderation decisions, author publish intent, publication pointers and tombstone deletion. Use for snapshot binding, stale callbacks, admin overrides and discussion access; not Cantos audio releases.
---

# Vink publication and moderation

Read [requirements](../../../prd/requirements.md): WR-03, PUB-01–PUB-05, AUD-01/02, MOD-01–MOD-10,
COM-01–COM-04 and COM-09/10 (`ED-02` magazine curation and `WR-04` multi-chapter works are
`DEFERRED` since 10/11/2026). Resolve the relevant USER and O IDs in the [TOML registry](../../../decisions/README.md). Read the
[publication/moderation target](../../../docs/architecture/publication-moderation.md),
[decision log](../../../docs/architecture/decisions.md) and the owning registries:
[product-decisions-registry](../../../docs/development/product-decisions-registry.md),
[publication-core](../../../docs/development/publication-core.md),
[moderation-core](../../../docs/development/moderation-core.md) and
[publication-runtime](../../../docs/development/publication-runtime.md).
Use [vink-engineering](../vink-engineering/SKILL.md) for boundaries and verification.

When a task introduces or changes rules for audience, moderation, deletion/tombstone, restoration
or actor authority that are still `OPEN` (`O-04`, `O-08..O-10`, `O-17..O-19`, or a second review of an admin conclusion in `O-15`), route through
[vink-shape](../vink-shape/SKILL.md) first; when a decision or implementation changes current
contracts or registries, update the owning files via [vink-specify](../vink-specify/SKILL.md).
Keep the four status dimensions distinct: product decision (`PRODUCT_CONFIRMED` / `DEFERRED` /
`OPEN`), architecture status, implementation level (`core` / `persistence-api` / `worker-provider`
/ `web` / `cmp`), and verification evidence. A safe baseline in pure core (such as `O-04` on
pending comments at deletion or `O-17` on previous URLs) does not turn an `OPEN` rule into `PRODUCT_CONFIRMED`.

## Keep independent facts independent

Identify the actual owners of working draft/version, metadata snapshot, immutable revision/hash,
moderation decision/version, author audience/publish intent (`intent_generation`), public pointer
(`pointer_intent_generation`) and lifecycle (`Active`, `AuthorDeleted(DeletionProvenance)`,
`Removed`). Do not collapse them into one status field or let an evaluation result imply Published.
Publish requests name an exact snapshot and expected generation; never implement `publish(latest)`.

Preserve the canonical separation examples:

- `r1` remains public while `r2` is edited, pending evaluation or rejected (`PUB-02`, `INV-02`).
- An `Approved` decision on `r2` without active `Public` intent for `r2` never promotes `r2`
  (`AUD-02`, `PUB-01`, `INV-01`).
- Saving a private draft or snapshot (`SaveSnapshot`) never creates an evaluation job or sends text
  to an external provider (`MOD-05`, `INV-08`).
- `Publication::record_review` requires the `WorkModeration` capability witness in domain code AND
  a commit-time database guard when persisted.

## Decide and commit

1. Trace public-read and mutation entrypoints across SSR, HTTP API v1, discovery feeds and
   conditional reads (`READ-01` `If-None-Match` must answer `404`, never `304`, after a work leaves
   public visibility).
2. Express eligibility as a pure deterministic decision over explicit restored facts
   (`Publication::restore`, `WorkModeration::restore`). Return actionable blockers; UI previews
   never grant authority.
3. In PostgreSQL transactions (`persistence::publication`), follow the established lock order
   (`publications FOR UPDATE` → `work_publications FOR UPDATE` → `works FOR SHARE` →
   `principals FOR UPDATE` via `locked_standing` → command receipt) so publication writes serialize
   cleanly with draft/tag saves and discussion/endorsement reads.
4. Keep draft edits, metadata snapshots and submitted revisions separate. Reject stale generations
   without side effects; replay exact idempotent retries through command receipts after the
   principal standing fence.

## Moderation and AI

Evaluation tickets and results bind target, revision/hash, policy version, decision version and job
identity (`MOD-07`). Duplicate, expired, wrong-hash or late evaluator callbacks cannot overwrite a
newer decision, beat an admin override on the reviewed version (`MOD-09`, `INV-03`), or resurrect a
deleted/removed work (`PUB-03`, `INV-07`).

Follow `MOD-01` and `MOD-06..10`: literary quality is not a posting permission; model output is
untrusted data validated against a strict schema. Timeout, provider error or invalid output closes
the job as retryable or `NeedsAdmin` without publishing (`MOD-08`). Do not promise an SLA, staffed
queue or connected notification channel while appeals (`MOD-10`) and real providers are unimplemented.

## Discussion access and deletion lifecycle (PUB-04, PUB-05, ADM-07, ADM-08)

Vink owns two-tier discussion in PostgreSQL ([ADR](../../../docs/architecture/discussion-ownership.md));
there is no Discourse integration or remote topic sync. Discussion visibility composes with the
work's `WorkAccessPlan` (`PublicBody`, `AuthorTombstone`, `AuthorHidden`, `Unavailable`):

- **Case A — Author deletes a work that was public (`PUB-04`):** `DeletionProvenance::PublicTombstone`
  removes the work from discovery/search/hashtags/rankings and hides title/body/metadata, while the
  old URL serves the replacement notice (“Nội dung đã được tác giả xoá”) and keeps previously
  approved comments readable. Under `O-03@r2` (`PRODUCT_CONFIRMED`), new comments and replies are
  blocked (`TombstoneReadOnly`), pending moderation runs its course, while comment writers may still
  remove their own comments.
- **Case B — Author switches a previously public work to “Chỉ mình tôi” (`ADM-07`, `ADM-08`, `PUB-07`, `COM-11`):**
  In pure core, `PublicationFacts::hidden` serves `WorkAccessPlan::AuthorHidden` (“Nội dung đã được ẩn”)
  and keeps eligible comments readable (`HiddenReadOnly` under `O-03@r2`, blocking new comments/replies);
  subsequent deletion from this hidden page leaves `DeletionProvenance::PublicTombstone` with comments
  intact (`O-05@r2` `PRODUCT_CONFIRMED`). Since `#159` the hidden pointer is stored and loaded, but no
  route or SSR serves the hidden page yet, so after `POST .../set-private` the old URL still answers not found (`404`).
  The review-withdrawn page of `O-17@r2` (moderator/admin rejected the public revision) is served by
  `GET /api/v1/works/{id}/reader` and the Web reader ([moderation-runtime](../../../docs/development/moderation-runtime.md)).
- **Case C (Admin/moderation removal) and Case D (Never public, then deleted):**
  project `WorkAccessPlan::Unavailable` (`404`); neither the work nor its discussion thread is
  publicly readable (`PUB-05`). Comments are never cascade-deleted when a work changes state.

## Focused acceptance

Cover changed claims with exact requirement IDs and matching boundaries (`verify quick`,
`verify discussion` for M1/M3/evaluator replays, `verify postgres` for lock graph, ETag `404` and
receipts, `verify web` when UI is in scope). Report snapshot/generation scope, guards changed,
tested failure cases and remaining runtime gaps per the
[verification guide](../vink-engineering/references/verification.md).
