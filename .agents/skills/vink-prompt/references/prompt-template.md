# VINK_PROMPT_INPUT_V1

Use this copyable shape, shortening fields for a small task. This is an agent handoff convention,
not an executable schema or a requirement to build a parser. Replace bracketed descriptions
with observed facts or `UNKNOWN`; preserve the user's constraints and decisions.

```text
VINK_PROMPT_INPUT_V1
REPOSITORY: loveoverflowcom/vink
GOAL: [one observable outcome and why it matters]
SCOPE: [affected capability; explicit exclusions and any user-fixed PR/chat counts]
BASE: [explicit user base, task integration base, or develop]
SNAPSHOT: [source branch/SHA; relevant dirty inputs; evidence gaps]
TARGET_ENVIRONMENT: UNKNOWN until the executing agent inspects its machine
SOURCES_AND_OWNERS:
  - AGENTS.md and .agents/skills/README.md
  - [existing product/architecture files, requirement/D/INV/S IDs and actual code paths]
  - [only the relevant vink-* skills]
DECISIONS:
  - [what the user selected; what remains proposed or needs a decision]
LAYOUT_SPEC: [UI only: selected spec/content or accessible path + revision; if missing,
  present the full/short vink-layout-spec before UI code/scripts/previews; retain the
  user's chosen direction and any explicit request to wait; no default approval gate]
ACCEPTANCE:
  - [observable result, failure case and its authoritative requirement]
VERIFY:
  - [cargo xtask verify quick|postgres|web (docs/development), exact claim and boundary]
BLOCKERS:
  if a runtime dependency is unavailable:
    complete independent work; preserve explicit unavailable/pending runtime behavior
    record the deferred check, prerequisite and resume condition
  if an unresolved decision changes correctness:
    complete unaffected work and ask the specific decision with its source
AUTHORIZED_ACTIONS:
  - [only actions actually granted by the user; local execution scope and remote actions]
HANDOFF:
  report changed behavior, files, evidence actually obtained and material remaining gaps
```

If a task is intentionally review-only or prompt-only, state that scope explicitly and do not
turn future actions described in the prompt into actions performed by the authoring session.
If the user authorized execution and a commit, retain that grant without inventing a new approval.

For split prompts, include each slice's owned/shared files, contract dependency and integration
order inside each prompt. For continuation, include completed work and valid evidence plus the
concrete next step; do not label deferred integration as PASS.

For UI, use the [shared layout convention](../../vink-layout-spec/references/layout-spec.md)
and include comparison against the chosen spec in ACCEPTANCE/VERIFY. Omit LAYOUT_SPEC for
tasks that do not change UI; do not invent a layout stage for a backend or documentation edit.
