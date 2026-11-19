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

1. Trace the requested journey or actual caller to its observable result. A sentence with the
   affected owner, AC and check suffices for a small fix. For multi-PR work, retain the integration
   owner, dependencies, closure scenario and any delivery constraints from the
   [task contract](../vink-prompt/references/prompt-template.md). Route prompt-only requests to
   [vink-prompt](../vink-prompt/SKILL.md).
2. Read current code/manifests and decision status. Resolve decisions the task depends on through
   the [registry](../../../decisions/README.md) for migrated IDs and the PRD/ADR for the rest.
   Determine what exists, what is proposed and what prerequisite actually affects this slice.
   Extend the existing skeleton (`apps/vink-web`, `services/vink-app`, `xtask`) rather than
   scaffolding a competing one.
   A merged PR is not proof of runtime acceptance; an absent database is not a reason to stop
   independent pure-code work.
3. Inspect Git status and pin the starting branch/SHA and requested base. Use the task worktree
   and branch rules in [AGENTS.md](../../../AGENTS.md#work-and-verification).
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

Use [the verification guide](../vink-engineering/references/verification.md) to select and report
checks on the final diff. Fix in-scope defects. A distinct outcome or explicitly accepted
limitation can be deferred with its owner, impact and dependency. If a prerequisite is missing,
complete independently verifiable work and name the remaining check.

Update the owning documentation via [vink-specify](../vink-specify/SKILL.md) if the change
implements a proposed module, command or behavior, or reveals drift between PRD, ADRs and
registries (including the affected feature-map entry when the journey or coverage changes).
Keep the four status dimensions (product decision, architecture decision, implementation level,
evidence) distinct. Report the scope that this slice completes. Preserve the PRD and existing
issue-tracking convention.

Use permission already granted in the conversation. When committing is requested, inspect the
staged diff, stage explicit task paths, commit in the repository's style and verify the resulting
commit/status. Push or PR creation follows the user's requested scope; no skill adds a new
confirmation step for an action already authorized. Posting comments, publishing a site or
calling a paid provider is not implied by a local implementation request.

Use the [shared handoff](../vink-engineering/references/verification.md#evidence-handoff-and-provenance)
once. Report the commit when relevant and the next concrete prerequisite for deferred acceptance.
