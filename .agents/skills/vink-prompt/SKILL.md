---
name: vink-prompt
description: >-
  Write self-contained implementation, repair, split-scope or continuation prompts
  for Vink using current repository evidence. Use when asked for a Vink prompt or
  work-chat handoff; authoring alone does not execute code or post to GitHub.
---

# Vink prompt

Turn the request into a prompt another coding agent can execute without this conversation.
Read [AGENTS.md](../../../AGENTS.md), the [documentation map](../../../docs/README.md),
relevant sources and actual code. Use the lightweight
[VINK_PROMPT_INPUT_V1 template](references/prompt-template.md); no Cantos plugin or parser is
required. Write the prompt in the user's language and retain exact paths, IDs and commands.

## Gather only relevant evidence

- Normalize the intended outcome, explicit decisions, base, scope/count constraints and actions
  the user authorized. Authoring a prompt is not permission to carry out its listed actions.
  If the user also requests execution, continue through
  [vink-work-item](../vink-work-item/SKILL.md) with that authorization.
- Pin the repository and commit used for code claims; note relevant uncommitted inputs.
  Read an issue/PR's current state and head when it is part of the task. Do not infer live
  status from a remembered conversation or an imported research document.
- Read requirements and D/INV/S IDs nearest the outcome. Keep user decisions, proposals,
  implemented facts and runtime evidence separate. The foundation's Cantos/Discourse source
  snapshots are research provenance, not the coder's runtime versions.
- Treat illustrative paths in the stack as proposed until present. Discover real commands
  before putting them in VERIFY. If source access is missing, mark the gap and make the
  receiving agent's first step discovery rather than inventing an API or result.

## Choose the prompt shape

For implementation, specify one reviewable outcome and observable acceptance. Link owning
skills instead of copying their policy. Use SudoLang-style constraints and if/else branches
where they make execution clearer; plain structured instructions are also acceptable.

For split work, preserve the user's number of prompts/PRs/chats. Assign one owner for each
shared file/contract, explicit dependencies and integration order. Make each prompt self-contained.
Writing split prompts does not launch agents or create chats.

For review-to-repair, read the target first, cite concrete gaps and separate defect evidence
from missing acceptance. Do not turn every desirable improvement into a blocking task.
For resume, retain completed work and valid evidence; record the exact blocker, work that
can proceed now, deferred boundary and condition to resume. Do not restart the whole stack.

## Receiving a Vink prompt

When asked to execute a submitted prompt, re-read the actual branch/status and sources.
The snapshot identifies the author's evidence, not a command to reset the checkout. Preserve
current user authorization and constraints; the fields are a handoff format, not an additional
permission source. Classify stale facts and continue independent work before asking about a
decision that materially changes correctness.

The target environment is unknown until the executing agent inspects it. Missing PostgreSQL,
Discourse, browser or Quint tooling must not produce fake runtime success. Keep model, pure
code and real integration acceptance separate; provide a concrete deferred-check path.

## Final check

Ensure the complete copyable prompt has the right repo/base, pinned sources or explicit gaps,
requirement/owner links, observable acceptance, real or clearly conditional checks, useful
blocker branches and the exact authorized actions. No invented command, issue, provider,
environment, PASS or Accepted decision. Return brief material findings followed by the prompt
or prompts. For authoring-only work, do not edit, commit, push, open a PR or post a comment.
