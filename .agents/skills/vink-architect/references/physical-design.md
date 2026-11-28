# Physical design for Vink Rust code

Use this reference when a review, refactor or new slice changes how Rust code is split into
modules, crates or packages, what they export, or what they depend on. It adapts the physical
design ideas of John Lakos, *Large-Scale C++ Software Design* (1996, chapters 3–7) to Vink's
Rust workspace. It is a way to decide and to show evidence, not a target layout: do not force
every service into the same folders, add a trait per struct or split a crate to make a diagram
look cleaner.

## What to look at

Judge two things separately:

- **Logical design**: what an abstraction means, which behavior it promises and which invariant
  it owns.
- **Physical design**: where that code lives, what it must know, and what has to be understood,
  rebuilt and re-tested when it changes.

A module tree is not evidence of a good physical design. Evidence is the real dependency graph:
the `use crate::…` edges inside a crate, the `Cargo.toml` edges between crates, and the
transitive closure a consumer inherits (if A uses B and B needs C, A pays for C as well). Source
dependencies and runtime calls are different graphs; name which one a claim is about.

## Principles, with when they apply

Each entry gives the signal, the options and the limit. Pick the smallest change that removes the
named knowledge or dependency, and keep transaction semantics and invariants intact.

| Principle | Signal | Options | Limit |
| --- | --- | --- | --- |
| **Levelization** | A low module names a higher one (persistence returns transport errors, a policy crate formats UI copy) or two modules need each other | Move the misplaced decision; cut the edge; merge the pair when neither means anything alone | No cycles is necessary, not sufficient: a long chain still drags its closure along |
| **Escalation** | Coordination is spread across peers, or a transport handler makes application decisions (timing equalization, hashing order, capability checks) | Lift the coordination into one use-case owner above the peers | A use case that needs one atomic commit keeps that commit in one transaction owner; never split it into several commits for neat layers |
| **Demotion** | Several consumers need the *same semantics* (a policy value, an ID type, a retry decision) and each re-derives it | Move the shared part into a lower module or a dependency-free crate | Same names or two call sites are not enough; the demoted part must not pull its consumers' context down with it |
| **Insulation** | A caller receives an implementation (a store, a pool, a row type) when it only needs a capability | Return outcomes instead of the implementation; facade methods; `pub(crate)`; convert representations at the boundary | Encapsulation (what callers may touch) and rebuild impact (what recompiles) are different claims; inside one crate a visibility change rarely changes build time |
| **Minimality** | `pub` items with no external consumer; wrappers and re-exports that only rename | Narrow to `pub(crate)` or private; delete unused API; keep ergonomic batch/transaction operations that save callers round trips | Do not starve a widely used abstraction of the operations its users would otherwise rebuild badly |
| **Hierarchical testing** | A risky decision can only be reached through the whole system | Give pure decisions explicit inputs and unit tests at their owner; test transactions against real PostgreSQL; test wire mapping at the transport | Mocks cannot prove PostgreSQL locking or atomicity; do not export internals only for tests |
| **Reuse discipline** | A shared package grows to remove a few repeated lines | Accept small syntactic duplication; share only rules that must never diverge (security, time windows, counting, idempotency) | A small crate is fine when it is a stable, dependency-light rule; size alone never decides keep or delete |
| **Physical cost** | Rebuild fan-out, link time, verification lanes or binary size grow with a change | Remove unused dependency edges; keep heavy dependencies out of widely depended-on crates | Claim a build or performance improvement only with a measurement taken the same way before and after |

Further judgment calls:

- Two runtime objects that know each other do not force two modules to depend on each other.
  Pass an ID, a handle or let a higher owner hold the relationship.
- A value that differs between configurations is data (a policy struct, a constant), not a
  reason for a trait. A trait earns its place when behavior varies or when it insulates a real
  dependency, even with one implementation.
- Public API is a maintenance commitment. For each `pub` item ask which consumer needs it, which
  external types appear in its signature and whether it leaks a representation. Check consumers
  and compatibility before narrowing an existing API.
- Test a boundary by imagining a different representation behind it (a stored value becomes
  computed, a table splits, a provider changes). If callers would have to change, the
  representation has become the contract.
- A contract states valid inputs, outcomes, errors, side effects and edge cases (expiry at the
  exact instant, retries keeping an event ID, whether a failed proof is spent). A refactor keeps
  the promised behavior; calling a behavior incidental needs evidence.
- Splitting purely to test has diminishing returns. A private helper fully exercised through
  its owner can stay private; test complex internals in the module that owns them.
- `main.rs` assembles: configuration, pools, transports, workers and lifecycle. Decisions belong
  to modules that tests can call.

## Rust and Vink translation

- Inside a crate, enforce levels with module visibility and, when the levels matter, a small
  owner-local test over `crate::` references (see `services/vink-identity/src/lib.rs`, module
  `levels`). Prefer this to new tooling.
- Between crates Cargo already forbids cycles; it does not keep a crate from depending on more
  than it needs. Remove dependency edges that no code uses.
- A binary and integration tests are separate crates, so a service library exposes modules as
  `pub`; narrow the items inside them instead.
- Keep wire contracts (`serde` DTOs, error codes) free of storage derives such as
  `sqlx::FromRow`, and keep storage free of transport error choices. Convert at the boundary
  that owns the mapping.
- A shared policy crate holds values and validation; copy and formatting stay with the UI
  owner, counting and authority with the service owner.

## Measuring

Use the numbers to find places to look, never as a score. Lakos's cumulative component dependency
(CCD) sums, over every component, the size of the set made of the component and everything it
transitively needs. Adding a component, even one that improves insulation, usually raises CCD.
Keep the component definition fixed when comparing.

```bash
# Crates that rebuild when a crate changes (its reverse closure inside the workspace).
cargo tree --workspace -e normal --invert vink-work-view-policy --prefix none | sort -u
# Direct fan-out of one crate.
cargo tree -p vink-identity -e normal --depth 1
# Raw module edges inside a crate; this also counts comments and test modules, so read the hits.
grep -rhoE 'crate::[a-z_]+' services/vink-identity/src | sort | uniq -c
```

Time builds only with the same profile, a warm or cold cache stated explicitly and no other
heavy job on the machine. Otherwise report that build time was not measured.

## Recorded decisions

- **Identity (`services/vink-identity`)**: one crate. The `levels` test checks explicit
  `crate::module` paths in the listed library files. Review new modules and other path forms
  separately. The binary uses `accounts` for assembly and maintenance. For RPC calls,
  `authority` gates access to the store by method; `http` maps
  `outcome` values to the contract and never sees the store. Persistence returns outcomes and
  `StoreError`, never `ContractError`. Rejected: new crates, a ports trait per store, splitting
  `delivery` (its pure retry policy is already unit-tested next to the outbox), and moving
  `mail::Purpose` (accounts reaches mail through `delivery` anyway, so the closure would not
  shrink). Module CCD rose from 49 to 57 with the new `outcome` module while `http` lost its
  direct edge to the store: the gain is insulation, not a smaller closure.
- **Work views (`packages/work-view-policy`)**: kept as a dependency-free crate because the App
  counting rule and the Web explanation share the 24-hour window (`VIEW_WINDOW_HOURS`) and the
  Web timer and copy share the threshold. Vietnamese copy moved to `apps/vink-web`; the App no
  longer re-exports the browser-only threshold.

## Report

For each physical change, answer:

1. Which part had to know too much about which other part before?
2. Which knowledge or dependency is gone now?
3. Which behavior became easier to verify, and at which level?
4. What new cost appeared (a module, an indirection, a larger closure), and why is it worth it?
