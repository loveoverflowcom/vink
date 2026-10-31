---
name: vink-why
description: >-
  Investigate why a Vink design or behavior was chosen using decision records,
  scoped Git history and relevant PR or issue context. Use for rationale,
  rejected alternatives or reconsidering a decision; distinguish recorded intent
  from an explanation inferred from code.
---

# Vink why

Answer a specific design question with provenance. Read [AGENTS.md](../../../AGENTS.md),
the relevant [decision log](../../../docs/architecture/decisions.md) entries and the current
code before reconstructing history. Use [vink-how](../vink-how/SKILL.md) when the current
mechanism itself is unclear; a plausible implementation does not establish its author's intent.

## Gather the narrow evidence chain

1. Identify the choice, current owner, branch/SHA and what would change if it were revisited.
   Read nearby comments, tests, the [development guide](../../../docs/development/README.md)
   and relevant requirement or D/INV/S IDs. Distinguish input constraints, proposals, bootstrap
   assumptions and decisions explicitly made by the user.
2. Follow only the history needed to locate the introduction or change: `git log -- <path>`,
   `git log -S '<literal>' -- <path>`, `git log -G '<pattern>' -- <path>`, then `git show`
   for the candidate commit. Use blame to locate changes, not to assign fault. Read the patch
   and surrounding code; a commit title or current comment can be incomplete.
3. If a linked PR, issue or relevant earlier chat contains the rationale, read that source
   through available tools. Prefer the service connector and inspect its current capabilities.
   Use [vink-recall](../vink-recall/SKILL.md) for conversation context. Missing access or shallow
   history is an evidence limit, not a reason to search unrelated accounts or invent discussion.
4. Stop when the documented reason and material tradeoff are clear, or when scoped sources
   cannot answer. Do not fetch all history, inspect every chat or require external services
   for a question already answered locally. If history must be fetched, keep it scoped and
   preserve the checkout; never reset to an old commit just to read it.

Vink's foundation imports and Cantos snapshots in
[sources](../../../docs/references/sources.md) are historical inputs. Their presence or a merged
bootstrap commit does not accept D-02–D-12, complete S-01–S-05 or prove a production capability.
The [bootstrap choices](../../../docs/development/README.md) record conditions for revisiting
temporary decisions; compare those conditions to the current task.

## Return the reason and its limits

Separate **recorded rationale**, **inference from code/history** and **unknown**. For each
material reason, cite the decision section, commit/path or exact PR/comment that supports it.
If sources disagree, explain their dates and scopes instead of silently choosing the story
that best fits the current code. Absence of a recorded reason is not proof that none existed.

Explain the constraint, the tradeoff it caused and what evidence would justify changing the
choice. Keep advice for today's task distinct from the historical account. For example,
manual saving may be a documented bootstrap boundary while autosave remains product intent;
this does not mean autosave was permanently rejected.

Return the answer in the user's language, with a short timeline only when sequence matters.
Investigation alone does not edit a decision record. If the user also asks for redesign or
implementation, carry the findings into [vink-architect](../vink-architect/SKILL.md) or
[vink-work-item](../vink-work-item/SKILL.md) within the already authorized scope.
