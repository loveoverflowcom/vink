---
name: vink-architect
description: >-
  Design a substantial Vink change by comparing viable alternatives against
  current owners, product constraints and evidence, then define verifiable
  implementation slices. Use for architecture decisions or changes spanning
  boundaries; small fixes belong directly to vink-work-item.
---

# Vink architect

Produce a design that answers a concrete product or engineering question. Begin with
[AGENTS.md](../../../AGENTS.md), relevant [requirements](../../../prd/requirements.md),
[decisions](../../../docs/architecture/decisions.md), and actual code located through the
[development guide](../../../docs/development/README.md). Read
[vink-how](../vink-how/SKILL.md) or [vink-why](../vink-why/SKILL.md) only when the mechanism or
past rationale is unclear. Do not make every task run a fixed discovery ceremony.

## Frame the choice through usage

Describe the intended experience, caller or API usage first, including an important failure
case. Turn vague feedback into an observable problem before accepting its suggested fix.
For an API or library seam, a short proposed usage example or contract sketch can reveal the
design more clearly than a long component list; label it proposed and use the existing owner.
Do not publish invented endpoints as an implemented contract.

Identify the few decisions this task actually needs: content ownership, commit boundary,
browser/server separation, identity source, recovery, or operational cost. Record assumptions
that remain provisional. Routine reversible implementation choices do not need a new approval;
an unresolved product choice such as auth authority or automatic moderation cannot be silently
settled by a prototype. Continue independent work while a necessary choice is clarified.

## Compare viable alternatives

Use the current implementation or smallest extension as the baseline. Compare alternatives
only where they differ materially; do not force multiple designs for an obvious local fix.
For each viable option, name:

- The user-visible result and which existing module owns each new responsibility.
- Data and authority crossing each boundary; atomic writes, version checks and recovery.
- Compatibility or migration work, operational burden, and how to back out safely.
- Evidence already available, the largest uncertainty and the smallest discriminating check.

Preserve Vink's boundaries: independent PostgreSQL data, pure domain rules, server-owned
identity, separate draft/revision/publication/curation, and no browser secrets. A proposed
Discourse integration uses an owned API/event contract, never cross-database writes. Read
[vink-publication](../vink-publication/SKILL.md) for lifecycle design and
[vink-behavior](../vink-behavior/SKILL.md) only when state/race analysis adds value.

When evidence can settle the choice and implementation experiments are authorized, use
[vink-prototype](../vink-prototype/SKILL.md). For design-only work, specify the experiment and
its deciding observation without implying it ran. Delegation is optional only when authorized
and available; this skill does not require subagents, multiple models, cloud machines or
worktrees. Keep proposed collaborators' ownership separate if parallel work is requested.

Repeated workarounds, duplicate business rules or casts that bypass domain types are reasons
to revisit the owner or representation. Treat them as evidence to investigate, not automatic
proof that the whole architecture must be replaced.

## Make the recommendation executable

Explain the preferred option through a concrete before/after flow, its principal tradeoff,
and what evidence would change the recommendation. Distinguish a provisional recommendation
from an explicit user decision. Update the owning decision record when recording a decision
is in scope; passing a check or merging a prototype alone does not make a D item Accepted.

Split larger work into independently reviewable slices with an owner, observable acceptance,
dependency order and the verification boundary from
[verification.md](../vink-engineering/references/verification.md). Name shared contract/file
ownership when slices overlap. Include rollout/migration/recovery only where the change needs
them; do not promise production checks the environment cannot perform.

For a design request, return the recommendation, key evidence, open choices and slices. For a
request that includes implementation, continue via [vink-work-item](../vink-work-item/SKILL.md).
For a requested handoff, use [vink-prompt](../vink-prompt/SKILL.md). Planning alone does not
create PRs, launch agents or authorize external mutations.
