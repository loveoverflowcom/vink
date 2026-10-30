---
name: vink-engineering
description: >-
  Implement or refactor Vink domain logic, Axum APIs, PostgreSQL persistence,
  moderation workers and Discourse adapters. Use for backend, contracts, concurrency
  and verification work; discover actual code and runners before applying the proposed stack.
---

# Vink engineering

Read [AGENTS.md](../../../AGENTS.md), the [documentation map](../../../docs/README.md),
the task's requirement IDs and actual code before editing. The repository starts with documents;
the [tech stack](../../../docs/architecture/tech-stack.md) is a conditional recommendation.
Do not copy Cantos runtime assumptions or create infrastructure to satisfy this skill.

## Find the owner and the claim

State the observable rule being changed, its owner and the failure it prevents. Trace callers
through domain, application, adapter and UI. For state, permission, lifecycle or retry changes,
use [vink-behavior](../vink-behavior/SKILL.md) to classify the impact; that lookup need not create
a model. Proposed rules remain proposed unless the user or a recorded decision selects them.

For a small change, a sentence is enough. For several durable claims, use a short working table:

| Claim / requirement | Failure mode | Owner | Check and boundary |
| --- | --- | --- | --- |

## Implementation method

- Keep validation, publication eligibility, callback admission and state transitions callable
  from deterministic tests without a server, database or AI provider. Thin adapters need no
  artificial domain layer.
- Make IDs, validated snapshots and outcomes distinct where mixing them could corrupt data or
  permissions. Use fallible construction at JSON, SQL and AI-output boundaries as well as in
  ordinary code. A private constructor is ineffective if deserialization bypasses it.
- Settled snapshots and decisions retain identity and history. Working drafts use versioned
  saves; a correction creates a new snapshot or decision rather than rewriting its evidence.
- Use closed error/outcome variants. Preserve conflict, unavailable, rejected and unknown-result
  distinctions through HTTP and UI; avoid panics and unchecked assumptions on request paths.
- Add a crate, service, dependency or reusable abstraction for a concrete consumer. Domain code
  imports neither HTTP/UI frameworks nor database/provider clients. Avoid a global Vink–Cantos
  contracts package; each API provider owns its schema and consumers own their representations.

## PostgreSQL and external effects

Use the ownership and transaction rules in
[tech stack](../../../docs/architecture/tech-stack.md), especially content, jobs and Discourse.
For a persistent change, identify the constraint or conditional write that enforces it under
concurrency; a read-then-write check outside a transaction does not provide that guarantee.

Publication, audit and outbox changes commit together. Re-read authority, revision/hash,
decision version and expected generation at the write boundary. Use optimistic concurrency
for drafts and appropriate locking/CAS for shared transitions. Preserve applied migrations;
add a migration with an upgrade/recovery path when schema work is in scope.

Claim jobs in short transactions. Network calls run outside database locks. Late results must
match the current lease/version and subject state before taking effect. Define attempts,
deadlines, retryable errors and the recovery path after a worker crash. Keep provider output
untrusted and bounded; an AI outage cannot authorize publication.

An outbox is local durability, not remote exactly-once execution. Reconcile uncertain creates,
bind events to stable subject IDs and generations, and describe the remote fencing actually
available. Use Discourse API/plugin contracts, never direct writes to its core database.
Derive acting identity from the server session/mapping; do not trust a browser's `Api-Username`.

Public reads, search and caches need current access checks as well as immutable content keys.
Account-scoped responses and drafts must not enter shared public caches. Keep keys, cookies,
raw manuscripts and staff-only webhook payloads out of logs and client-visible errors.
Measure a query before adding an index/cache; test that withdrawn/private content stays hidden.

## Verify and hand off

Follow [verification.md](references/verification.md) to select checks proportional to the claim.
For a bug, reproduce it and add a regression that checks the exact outcome through the relevant
owner. Run focused checks while editing and the affected final checks once the change settles.
Do not demand runtime tests for a documentation-only change.

Report what changed, the owner/requirements affected, checks actually run and their scope,
and material gaps. A fake adapter, pure test, real database, pinned Discourse and browser run
are different evidence. Name any unrun layer and the concrete prerequisite for checking it.
