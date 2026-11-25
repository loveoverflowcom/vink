---
name: vink-work-item
description: Deliver a bounded Vink feature, issue, bug fix or continuation from requirements through implementation and honest verification. Use for requested code or repository work; respect the specified branch and existing authorization for commits or PRs.
---

# Vink work item

## Define the result

Start with the user's request and any linked issue. Describe who takes the action, what they
should observe and which failure the change must prevent. For a continuation, retain the
integration owner, dependencies and acceptance criteria from the [task contract](../vink-prompt/references/prompt-template.md).
Check status, branch and starting SHA in the assigned worktree. [Repository workflow](../../../AGENTS.md#work-and-verification)

## Change the owning module

Follow the entrypoint in the [feature map](../../../docs/development/feature-map.md) through its
caller, module and existing assertions. Confirm the behavior in source. For a bug, reproduce it
at the affected boundary, then fix the owner. Use the [skill index](../README.md) for guidance
specific to that module.

Resolve decisions the task depends on in the [registry](../../../decisions/README.md) and the
owning PRD or ADR. Make reversible implementation choices within scope. If a missing rule changes
authority, visibility or data meaning, use [vink-shape](../vink-shape/SKILL.md) for that question
and continue independent work. If the implementation changes a documented behavior or reveals
drift, use [vink-specify](../vink-specify/SKILL.md) to update the owning record.

## Verify the result

Choose the smallest check that exercises the user's entrypoint and would catch the failure.
Use real PostgreSQL for transaction guarantees, a browser for editor or hydration behavior,
and static checks for skill metadata and documentation links. Fix in-scope failures and check
the final diff. When a prerequisite is missing, run the independent checks and identify the
specific acceptance criterion still awaiting evidence. [Verification guide](../vink-engineering/references/verification.md)

Report the observable result, command, source SHA, boundary and material gaps once. For a
requested commit, push or PR, follow [vink-git-workflow](../vink-git-workflow/SKILL.md)
within the authorization already given.
