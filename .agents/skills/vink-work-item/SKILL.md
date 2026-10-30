---
name: vink-work-item
description: >-
  Deliver a bounded Vink feature, issue, bug fix or continuation from requirements
  through implementation and honest verification. Use for requested code or repository
  work; respect the specified branch and existing authorization for commits or PRs.
---

# Vink work item

Start from [AGENTS.md](../../../AGENTS.md), the user's task, linked issue when relevant,
and the nearest sources in [docs/README.md](../../../docs/README.md). A formal work queue,
GitHub Project and full application stack are not prerequisites for doing useful work.

## Resolve scope and baseline

1. State one reviewable outcome, affected requirement IDs and acceptance. Retain a PR/chat/count
   constraint the user supplied. A prompt-only request belongs to
   [vink-prompt](../vink-prompt/SKILL.md), not implementation.
2. Read current code/manifests and decision status. Determine what exists, what is proposed
   and what prerequisite actually affects this slice. A merged PR is not proof of runtime
   acceptance; an absent database is not a reason to stop independent pure-code work.
3. Inspect Git status and pin the starting branch/SHA and requested base. Use the explicit
   user base, otherwise the task's integration base, otherwise `develop`. When creating a new
   branch, use `codex/<outcome>` by default. If the user requests a commit directly on develop,
   keep that workflow. Preserve existing edits; do not reset, stash or sweep unrelated files.
4. Continue with reasonable reversible choices. If an unresolved product/security decision
   changes correctness, complete independent work and ask the focused question. A prototype
   records provisional assumptions; it cannot claim the production acceptance it lacks.

Do not require a new issue, planning hierarchy or accepted architecture for a small documentation
task. Do not silently select an auth provider, automatic moderation policy or withdrawal SLA
while the corresponding D decision is open.

## Implement with the relevant owners

| Work | Guidance |
| --- | --- |
| Domain, API, PostgreSQL, worker, integration | [vink-engineering](../vink-engineering/SKILL.md) |
| Revisions, moderation, publish/withdraw, curation | [vink-publication](../vink-publication/SKILL.md) |
| State/guard/race impact or a useful Quint model | [vink-behavior](../vink-behavior/SKILL.md) |
| Leptos SSR/hydration/editor/browser state | [vink-leptos-web](../vink-leptos-web/SKILL.md) |
| Visual design, reading/writing UX, accessibility | [vink-ui-design](../vink-ui-design/SKILL.md) |
| Existing diff review | [vink-code-review](../vink-code-review/SKILL.md) |

Read only the references needed for the slice. Keep a claim → failure → owner → check table
when several durable behaviors change; a sentence is enough for a small edit. For a bug,
reproduce the behavior and fix its owning layer. For documents, preserve source intent,
status and provenance while updating links and placement.

## Verify and finish the authorized work

Use [the verification guide](../vink-engineering/references/verification.md). Discover actual
commands; do not run hypothetical xtask gates or install the Cantos toolchain. Finish focused
checks and review the final diff. Keep pure, fake, PostgreSQL, Discourse, model and browser
evidence distinct. If a dependency is unavailable, deliver the independently verified portion
and record exactly what remains unverified; keep runtime failures explicit rather than fake.

Update the owning documentation if the change implements a proposed module, command or
behavior. Keep acceptance claims tied to evidence. Do not create a second status board or
require GitHub-only tracking when the project has not selected it.

Use permission already granted in the conversation. When committing is requested, inspect the
staged diff, stage explicit task paths, commit in the repository's style and verify the resulting
commit/status. Push or PR creation follows the user's requested scope; no skill adds a new
confirmation step for an action already authorized. Posting comments, publishing a site or
calling a paid provider is not implied by a local implementation request.

Report the outcome, branch/base or commit when relevant, requirement coverage, checks and
material gaps. Include the next concrete prerequisite for deferred acceptance. Keep the report
proportional; do not make the user approve an abstract plan when a reviewable result can be made.
