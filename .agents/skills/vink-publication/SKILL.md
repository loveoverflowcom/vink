---
name: vink-publication
description: Implement or review Vink literary revisions, audiences, moderation decisions, author publish intent, publication pointers and tombstone deletion. Use for snapshot binding, stale callbacks, admin overrides and discussion access; not Cantos audio releases.
---

# Vink publication and moderation

Read [requirements](../../../prd/requirements.md): WR-03, PUB-01–PUB-07, AUD-01/02, MOD-01–MOD-15,
COM-01–COM-04 and COM-09–COM-11 (`ED-02` magazine curation and `WR-04` multi-chapter works are
`DEFERRED` since 10/11/2026). Resolve the relevant USER and O IDs in the [TOML registry](../../../decisions/README.md). Read the
[publication/moderation target](../../../docs/architecture/publication-moderation.md),
[decision log](../../../docs/architecture/decisions.md) and the owning registries:
[product-decisions-registry](../../../docs/development/product-decisions-registry.md),
[publication-core](../../../docs/development/publication-core.md),
[moderation-core](../../../docs/development/moderation-core.md) and
[publication-runtime](../../../docs/development/publication-runtime.md).
Use [vink-engineering](../vink-engineering/SKILL.md) for boundaries and verification.

Resolve each decision ID at the task checkout before sending a question to the owner. Use
[vink-shape](../vink-shape/SKILL.md) for an unresolved rule that changes the requested behavior.
Use [vink-specify](../vink-specify/SKILL.md) when the change affects a contract or registry.
Report the product decision, architecture status, implementation level and verification evidence
separately. For example, the pure-core behavior for pending comments at deletion is a baseline
for `O-04`. Check its current revision before treating that behavior as a product requirement.

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
- `Publication::record_review` accepts a `ReviewedRevision` from the moderation owner. Check actor
  authority and the decision version again in the transaction that writes the result.

## Decide and commit

1. Trace the public pointer, Reader projection, discovery results and cached reads separately.
   On `GET /api/v1/works/{id}/publication`, check access before comparing the ETag. With the
   pointer withdrawn, even a matching old validator gets `404` (`READ-01`).
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
the job as retryable or `NeedsAdmin` without publishing (`MOD-08`).

An author can file a work appeal against an evaluator or moderator rejection through the API.
The appeal binds the exact revision and decision version. An admin can uphold or reverse it. Recheck
current publish intent before restoring a pointer. Recording an appeal and delivering a
notification are separate events. See the [appeals contract and Web flows](../../../docs/development/moderation-runtime.md#6-phản-đối-quyết-định-tác-phẩm-mod-10).

## Reader and discussion access

Use the work's `WorkAccessPlan` and the [discussion visibility rules](../../../docs/architecture/discussion-ownership.md)
together. Check the actor, whether the work was ever public and what happened to its public revision.

- **Author switches a published work to private.** `GET /api/v1/works/{id}/reader` returns
  the `hidden` projection. The old Reader page serves the hidden notice and eligible comments
  (HTTP 200, `no-store`, `noindex` on SSR). Omit the author's name, title, body and tags.
  Comment writers may remove their own comments. Block new comments and replies.
- **Moderator or admin rejects the public revision.** Apply [O-17@r2](../../../decisions/signoffs/O-17.r2.toml)
  (`PRODUCT_CONFIRMED`). The Reader keeps the author's name and eligible comments with the
  review-withdrawn notice. Withhold the rejected text and metadata.
- **Deletion and removal.** In core, author deletion of a public or author-hidden work
  keeps a read-only tombstone (`PUB-04`, `O-03@r2`, `O-05@r2`). Admin removal of the whole
  work and deletion of a never-public work make the old URL and thread unavailable (`PUB-05`).
  Preserve comment records across work lifecycle changes.

Check the [runtime contract for the old URL](../../../docs/development/moderation-runtime.md#5-trang-ở-url-cũ-o-17r2)
for endpoint responses, and [publication core](../../../docs/development/publication-core.md) for
which deletion paths have adapters.

## Focused acceptance

Cover changed claims with exact requirement IDs and matching boundaries (`verify quick`,
`verify discussion` for M1/M3/evaluator replays, `verify postgres` for lock graph, ETag `404` and
receipts, `verify web` when UI is in scope). Report snapshot/generation scope, guards changed,
tested failure cases and remaining runtime gaps per the
[verification guide](../vink-engineering/references/verification.md).
