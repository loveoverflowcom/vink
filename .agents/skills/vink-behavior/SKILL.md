---
name: vink-behavior
description: Analyze Vink state transitions, permissions, publication races and retry/recovery; author or review focused Quint models when the task warrants them. Use for semantic impact and counterexamples without assuming an existing model registry or verifier.
---

# Vink stateful behavior

Use [requirements](../../../prd/requirements.md) and
[the decision log](../../../docs/architecture/decisions.md) to locate the relevant rule,
INV ID and decision status. Resolve migrated IDs through the
[TOML registry](../../../decisions/README.md). Inspect the real implementation separately. The foundation
proposes Quint. The scoped two-tier creation model, the comment lifecycle/mode/moderation
authority model and their generated traces live in `services/vink-app/behavior/discussion`;
`verify discussion` checks both and replays their inputs through the real Rust decisions. The owner-local literary endorsement model/replay (LVE-PR-01) shares this runner;
its [registry](../../../docs/development/endorsement-core.md) records separate semantics and gaps. They
do not cover auth authority, locking or I/O; the D2T-INV registry and gaps are in
`docs/development/discussion-core.md`.
`verify kani` checks single discussion/taxonomy/endorsement decisions over whole input domains; it
is no sequence or race proof. There is no generic behavior lane.
`services/vink-app/behavior/target` holds target specifications for the owner's decisions of
10/11/2026 (`collections`, `comment_edit`, `discovery`): they run as a step of `verify discussion`, have generated vectors and negative controls,
and have **no Rust replay**, so they are model evidence only. Canonical M1 (64/64 steps),
M3 (41/41 steps) and M4 governance (`governance.qnt`, 105/105 steps, replacing `target/appeal.qnt`) live in
`behavior/publication` with full pure-core replay plus the 24-step `evaluator-vectors.json` script
([publication-core](../../../docs/development/publication-core.md),
[moderation-core](../../../docs/development/moderation-core.md)). When code implements one of the remaining target models, move the model to its owner directory
with a replay instead of keeping a second model of the same policy. When modeling or replay exposes an
unresolved product rule (`O-01..O-19`, resolved via the TOML registry) or a conflict between PRD and code, route the question through
[vink-shape](../vink-shape/SKILL.md) and record confirmed updates via
[vink-specify](../vink-specify/SKILL.md).
Other stateful code keeps its own boundary tests:
draft CAS in `tests/postgres.rs`, the desk save state machine in
`apps/vink-web/src/desk_state.rs`.

## Classify before choosing a change

For a changed state, guard, permission, retry, recovery step or behavioral oracle:

1. Locate the owning requirement and actual entrypoints. If models/scenarios already exist,
   search them by affected paths and behavior, and reuse stable IDs rather than adding a copy.
2. Distinguish the source of authority: an explicit user decision, a recorded accepted rule,
   a proposal awaiting selection, or an implementation fact. Running a model does not settle
   an unresolved rule.
3. Classify the task and choose the smallest authorized delta:

| Situation | Action |
| --- | --- |
| Code violates a selected requirement | Reproduce and fix code; preserve a correct oracle |
| A model contradicts its selected requirement | Correct the model with cited evidence; do not rewrite the rule to bless code |
| The user changes a requirement | Update its owner and affected tests/model/code within the requested scope |
| A rule has no model or no relevant scenario | Record the gap; add a focused scenario/model only when useful and in scope |
| A product decision is unresolved | Continue independent work, label prototype assumptions and request the decision when it affects correctness |
| Copy/layout/format change preserves outcomes | State no semantic impact; no model edit is needed |
| Review or prompt-only task | Report the classification and proposed delta without implementing it |

This is an impact check, not a requirement to model every change or install a toolchain.
Use [vink-engineering](../vink-engineering/SKILL.md) for implementation and evidence scope.

## When a model adds value

Prefer a small state space around a specific risk: pending revision versus public pointer,
human override versus late AI response, or publish/withdraw versus delayed remote events.
Select one owning model; do not create separate copies for browser, worker and API.

- State finite actors, revisions, versions, failure points and the assumptions being made.
  Model authorization as an explicit assumption or guard, not an unexplained trusted actor.
- Name each property and show a reachable state that exercises its antecedent. An unreachable
  publish action makes a privacy property vacuous.
- Check assertion sensitivity with a type-correct deliberate violation of the relevant guard.
  A syntax error or a run selecting zero scenarios is not a meaningful negative control.
- Safety and eventual progress are different claims. Liveness needs scheduling/time/fairness
  assumptions and operational ownership; a boolean such as `hasHumanQueue` proves no response.
- Preserve counterexamples, source/tool version, command, seed and bounds. Interpret the trace
  against the requirement before changing expectations or implementation.

Discover the installed/pinned Quint version and supported commands from actual repository
tooling. Record its status if unavailable; do not invent `cargo xtask verify behavior` support.

## Implementation evidence stays independent

If replaying model traces against code, call the actual production decision seam and feed it
events/inputs, not precomputed expected states. Compare exact observed outcomes to the model;
a second handwritten reducer merely compares two models. Label synthetic shell assumptions.

Retain real PostgreSQL concurrency tests and browser checks for
their own boundaries. Report model simulation, bounded checking and implementation conformance
separately, with limits. A PASS neither promotes a draft model nor establishes runtime acceptance.
