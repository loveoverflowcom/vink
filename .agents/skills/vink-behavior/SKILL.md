---
name: vink-behavior
description: >-
  Analyze Vink state transitions, permissions, publication races and retry/recovery;
  author or review focused Quint models when the task warrants them. Use for semantic
  impact and counterexamples without assuming an existing model registry or verifier.
---

# Vink stateful behavior

Use [requirements](../../../prd/requirements.md) and
[the decision log](../../../docs/architecture/decisions.md) to locate the relevant rule,
INV ID and decision status. Inspect the real implementation separately. The foundation
proposes Quint, but the repository ships no catalog, model or behavior lane. Current stateful
code is small and tested at its own boundary: draft CAS in `tests/postgres.rs`, the desk save
state machine in `apps/vink-web/src/desk_state.rs`.

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

Retain real PostgreSQL concurrency tests, pinned Discourse integration and browser checks for
their own boundaries. Report model simulation, bounded checking and implementation conformance
separately, with limits. A PASS neither promotes a draft model nor establishes runtime acceptance.
