---
name: vink-engineering
description: >-
  Implement or refactor Vink domain logic, Axum APIs, PostgreSQL persistence, the Vink-owned
  two-tier discussion module and moderation workers. Use for backend, contracts, concurrency
  and verification work; discover actual code and runners before applying the proposed stack.
---

# Vink engineering

Read [AGENTS.md](../../../AGENTS.md), the [documentation map](../../../docs/README.md),
the task's requirement IDs and actual code before editing. The skeleton's layout, commands and
limits are in [docs/development](../../../docs/development/README.md); the
[tech stack](../../../docs/architecture/tech-stack.md) remains a conditional recommendation.
Do not copy Cantos runtime assumptions or create infrastructure to satisfy this skill.

## Where code lives

| Responsibility | Owner today |
| --- | --- |
| Pure rules (document v1, title, draft version, revision hash) | `services/vink-app/src/domain/` — no Axum, SQLx, Leptos or tokio imports |
| Discussion decisions (two tiers, body, lifecycle, comments mode, permissions, visibility, idempotency, paging) | `services/vink-app/src/domain/discussion.rs` + `domain/discussion/`; [ADR](../../../docs/architecture/discussion-ownership.md), [registry](../../../docs/development/discussion-core.md) |
| Literary endorsement decisions (LVE #73: work revision/root/reply, accepted order, receipts) | `services/vink-app/src/domain/endorsement.rs` + `domain/endorsement/`; [decision record](../../../docs/architecture/literary-endorsement.md), [registry](../../../docs/development/endorsement-core.md); use cases, routes and `PgStore` adapter: [runtime](../../../docs/development/literary-endorsement-runtime.md) |
| Use cases, actor checks, storage ports | `services/vink-app/src/application/` |
| SQL, transactions, constraint classification | `services/vink-app/src/persistence/` (`postgres.rs`, `discussion.rs`) and `migrations/` |
| Pools, migration lifecycle, schema state, connection-error classes | `packages/store-postgres/` (caller supplies its `Migrator`; no business SQL) |
| PostgreSQL container, owner databases, migrator/runtime roles | `services/postgres/` (`provision.sh`, runbook) |
| Authentication facts (no authority yet), Identity migrations, private contract | `services/vink-identity/` and `contracts/rpc/v1/`; never imported by `vink-app` |
| HTTP API v1, health, Origin check, SSR host | `services/vink-app/src/http/` |
| Canonical API contract and fixtures | `services/vink-app/contracts/http/v1/` |
| Principal resolution seam; App's Identity client | `services/vink-app/src/identity/` (async resolver: principal / anonymous / unavailable) |
| Shared config, logging, shutdown; worker | `config.rs`, `runtime.rs`, `src/bin/vink-worker.rs` |
| PostgreSQL tests (`#[sqlx::test]`, HTTP through the router) | `services/vink-app/tests/postgres.rs` |
| Browser-side views and DTOs | `apps/vink-web/` (see [vink-leptos-web](../vink-leptos-web/SKILL.md)) |

`verify quick` enforces the import boundaries for `domain/` and `application/` and that
`vink-web` does not depend on the service or SQLx. Add a crate only for a real consumer.

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
  An API change updates the provider contract and fixtures first, then each consumer's own
  representation; both sides' fixture tests must pass.
- Add a crate, service, dependency or reusable abstraction for a concrete consumer. Domain code
  imports neither HTTP/UI frameworks nor database/provider clients. Avoid a global Vink–Cantos
  contracts package; each API provider owns its schema and consumers own their representations.

## PostgreSQL and external effects

Use the ownership and transaction rules in
[tech stack](../../../docs/architecture/tech-stack.md), especially content, jobs and discussion.
For a persistent change, identify the constraint or conditional write that enforces it under
concurrency; a read-then-write check outside a transaction does not provide that guarantee.
`PgStore::save_draft` is the reference pattern: owner and version sit in the UPDATE predicate,
and the follow-up read only classifies an unwritten save. Add a new migration file for schema
changes; never edit an applied one. Seed data stays in `dev_seed.rs`, not in migrations.

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
available. Derive acting identity from the server session/mapping; never trust a user ID, role
or display name sent by the browser.

Discussion is owned by Vink (decision 03/11/2026); Discourse is only a design reference. Do not
add a Discourse adapter, sync outbox or a second comment store. The adapter loads facts under
locks and calls the pure decision (`decide_submission`, lifecycle transitions); the database
independently enforces two tiers with composite keys and immutable placement. A work author
toggles comments for that work but never edits, removes or moderates others' comments.

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
and material gaps. A fake adapter, pure test, real database, model run and browser run
are different evidence. Name any unrun layer and the concrete prerequisite for checking it.
