# VINK_PROMPT_INPUT_V1

For a direct, single-AC fix, one or two sentences can carry the caller, exact change and check.
Keep the repository, base, inspected revision, AC ID and grant identifiable. Omit fields that
would repeat those sentences. For an existing issue or plugin handoff, preserve its fields,
stable AC IDs, tracking and authorization constraints.

```text
VINK_PROMPT_INPUT_V1
REPOSITORY: loveoverflowcom/vink
BASE: [user's base, task integration base, or develop]
SNAPSHOT: [source SHA and any relevant dirty inputs or access gap]
TARGET_ENVIRONMENT: UNKNOWN until the executing agent inspects its machine
GOAL: [entry → action → observable result, or actual caller → output for backend/library/docs]
SCOPE: [affected owners/consumers and what this change completes; include explicit exclusions]
ACCEPTANCE: [AC-01 or existing ID — observable result, including recovery when relevant]
VERIFY: [AC ID — smallest check that exercises the required boundary]
AUTHORIZED_ACTIONS: [actions the user actually granted]
```

A copy fix can name the route, state and new text in AC-01, with a check of that rendered state
on the same line. A short result with the checked revision and any gap suffices for handoff.

Extend the same block when the work needs more context:

- **SCOPE for split work.** Keep any user-fixed number of PRs/chats. State what this slice
  completes and defers, the existing parent if any, integration owner, ordered dependencies
  and scenario that closes the integrated journey. If partial exposure is unsafe, identify
  how an existing control protects the affected entrypoints or sequence delivery safely.
- **SOURCES_AND_OWNERS and DECISIONS.** Link the authoritative requirement, actual caller,
  implementation owner and relevant skill. Carry applicable `registry_refs` (ID@revision,
  status, source and snapshot), `confirmed`, `open_product`, `open_technical` and `assumptions`.
  Give each material open choice its owner, affected requirements and dependent work.
- **LAYOUT_SPEC and UX_CRITERIA for UI.** Carry the selected spec and its revision. For a small
  edit, the affected region, before/after and check can fit on one line. If missing, the receiver
  presents the [scope-appropriate spec](../../vink-layout-spec/references/layout-spec.md) before
  a UI patch or preview. Preserve an explicit request to wait. For visual or interaction work,
  include the decisive [rubric rows](../../vink-ui-design/references/ux-quality.md), reference
  class/revision and approval status, measurable invariants, modalities, failure states and
  fix budget that apply. Check 100% and 200% text when affected. Omit UX_CRITERIA for a copy or
  single-property fix with an obvious check.
- **VERIFY and BLOCKERS.** Select an existing command or inspection from the
  [scoped workflows](../../../../docs/development/scoped-workflows.md). When using the diff planner, carry its
  scope, base/head, plan ID and selected commands. For PR review or unrelated local inputs, use a committed range. Record actual
  execution or applicable prior evidence beside each AC. Give the target, fixture
  and prerequisite when they affect the claim. If a dependency or decision blocks an AC,
  identify the dependent check, work that can proceed and the condition for resuming it.
- **HANDOFF.** Use the [verification guide](../../vink-engineering/references/verification.md#evidence-handoff-and-provenance)
  for evidence reuse, scoped claims and a human smoke card when applicable. For continuation,
  carry completed work, original evidence and the next concrete step.

Keep review-only, prompt-only and execution grants explicit. Preserve a commit or push grant
already given for the task. When the grant includes a branch, commit or PR, the receiver follows
[vink-git-workflow](../../vink-git-workflow/SKILL.md) unless the user fixed a branch name.

For Web/Preview/Test work, pin the host, real caller, resolved normal edges and artifact manifest.
See the [Web/Preview boundary](../../../../docs/development/web-preview-boundary.md).
