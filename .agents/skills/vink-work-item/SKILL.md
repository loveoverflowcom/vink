---
name: vink-work-item
description: Deliver a bounded Vink feature, issue, bug fix or continuation from requirements through implementation and honest verification. Use for requested code or repository work; respect the specified branch and existing authorization for commits or PRs.
---

# Vink work item

Start from [AGENTS.md](../../../AGENTS.md), the user's task, linked issue when relevant,
the nearest sources in [docs/README.md](../../../docs/README.md) and the implemented layout,
commands and limits in [docs/development](../../../docs/development/README.md). A formal work
queue or GitHub Project is not a prerequisite for doing useful work.

Use the [feature map](../../../docs/development/feature-map.md) to find a journey's entrypoints
and existing assertions, then confirm them in source. For an unclear mechanism, rationale or
prior handoff, select `vink-how`, `vink-why` or `vink-recall` from the
[skill index](../README.md); a small implementation does not need all three.

## Resolve scope and baseline

1. State one reviewable outcome, affected requirement IDs, Journey (`entry → action → observable result → recovery/return`, or real caller/output for backend/library/docs — never invent a UI journey), affected Touchpoints (caller, UI/API, durable owner, consumer), what this slice completes vs defers, and the smallest closure evidence for its ACs. One sentence suffices for a small fix; do not force an epic onto a self-contained task. For multi-PR journeys, name the existing parent, integration owner, dependencies and final closure scenario. If partial exposure is unsafe, use an existing rollout mechanism instead of building a feature-flag system; a hidden UI entry does not protect endpoints, jobs, migrations or data. Retain a PR/chat/count constraint the user supplied. A prompt-only request belongs to [vink-prompt](../vink-prompt/SKILL.md), not implementation.
2. Read current code/manifests and decision status. Resolve decisions the task depends on through
   the [registry](../../../decisions/README.md) for migrated IDs and the PRD/ADR for the rest.
   Determine what exists, what is proposed and what prerequisite actually affects this slice.
   Extend the existing skeleton (`apps/vink-web`, `services/vink-app`, `xtask`) rather than
   scaffolding a competing one.
   A merged PR is not proof of runtime acceptance; an absent database is not a reason to stop
   independent pure-code work.
3. Inspect Git status and pin the starting branch/SHA and requested base. Use the explicit
   user base, otherwise the task's integration base, otherwise `develop`. Preserve an active
   non-`develop` branch/worktree (such as an Antigravity worktree or user-specified branch) unless
   asked to create a new branch; when creating a new branch from `develop`, use `codex/<outcome>`
   by default. If the user requests a commit directly on develop, keep that workflow. Preserve
   existing edits; do not reset, stash or sweep unrelated files.
4. Continue with reasonable reversible choices. If an unresolved product/security decision
   (`OPEN` items resolved in `O-01..O-19`, or a change to authority, public visibility, lifecycle or data
   meaning) affects correctness, complete independent work and use
   [vink-shape](../vink-shape/SKILL.md) to classify and ask only the blocking product questions.
   A small bug fix or clear technical slice proceeds directly without shaping. A prototype
   records provisional assumptions; it cannot claim the production acceptance it lacks.

Do not require a new issue, planning hierarchy or accepted architecture for a small documentation
task. Do not silently select an auth provider, automatic moderation policy or withdrawal SLA
while the corresponding D decision is open.

## Implement with the relevant owners

| Work | Guidance |
| --- | --- |
| Clarify product scope, open rules in `O-01..O-19`, owner questions | [vink-shape](../vink-shape/SKILL.md) (only when rules are unclear/open) |
| Write or reconcile specs, PRD, decision records, registries | [vink-specify](../vink-specify/SKILL.md) |
| Domain, API, PostgreSQL, worker, integration | [vink-engineering](../vink-engineering/SKILL.md) |
| Revisions, audiences, moderation, publish/tombstone | [vink-publication](../vink-publication/SKILL.md) |
| State/guard/race impact or a useful Quint model | [vink-behavior](../vink-behavior/SKILL.md) |
| Leptos SSR/hydration/editor/browser state | [vink-leptos-web](../vink-leptos-web/SKILL.md) |
| Visual design, reading/writing UX, accessibility | [vink-ui-design](../vink-ui-design/SKILL.md) |
| Existing diff review | [vink-code-review](../vink-code-review/SKILL.md) |

Read only the references needed for the slice. Keep a claim → failure → owner → check table
when several durable behaviors change; a sentence is enough for a small edit. For a bug,
reproduce the behavior and fix its owning layer. For documents, preserve source intent,
status and provenance while updating links and placement.

## Verify and finish the authorized work

Use [the verification guide](../vink-engineering/references/verification.md) to pick the
`cargo xtask verify` lanes or [scoped commands](../../../docs/development/scoped-workflows.md) the
change needs; the commands are defined in
[docs/development](../../../docs/development/README.md), not in this skill. Do not install the
Cantos toolchain or invent gates. Finish focused checks and review the final diff. Keep pure, fake, PostgreSQL, model and browser
evidence distinct, preserve provenance when reusing evidence by delta applicability (never relabel an old run onto a new SHA), and inspect UI runtime/hydration errors without suppressing them. Fix in-scope defects before claiming completion; separate a follow-up only for a distinct outcome or an explicitly accepted limitation, recording owner, impact and dependency. If a dependency is unavailable, deliver the independently verified portion
and record exactly what remains unverified; keep runtime failures explicit rather than fake.

Update the owning documentation via [vink-specify](../vink-specify/SKILL.md) if the change
implements a proposed module, command or behavior, or reveals drift between PRD, ADRs and
registries (including the affected feature-map entry when the journey or coverage changes).
Keep the four status dimensions (product decision, architecture decision, implementation level,
evidence) distinct alongside the scoped claims (`IMPLEMENTED`, `VERIFIED`, `OUTCOME_COMPLETE` —
child completion does not imply whole-journey completion or deployment), and never silently rewrite
a PRD rule to bless a code deviation. Do not create a second status board or
require GitHub-only tracking, automatic GitHub labels or issue-state changes.

Use permission already granted in the conversation. When committing is requested, inspect the
staged diff, stage explicit task paths, commit in the repository's style and verify the resulting
commit/status. Push or PR creation follows the user's requested scope; no skill adds a new
confirmation step for an action already authorized. Posting comments, publishing a site or
calling a paid provider is not implied by a local implementation request.

Report using the shared evidence handoff (`AC/claim → connected source/caller → evidence → result → gap`) from
[the verification guide](../vink-engineering/references/verification.md), including branch/base or commit when relevant,
scoped completion claims, material gaps, next concrete prerequisite for deferred acceptance, and a
short human smoke card (~2 min, `NOT_RUN` until observed) when a main UI journey or interaction with
residual device risk changed. Keep the report proportional; do not make the user approve an abstract plan when a reviewable result can be made.
