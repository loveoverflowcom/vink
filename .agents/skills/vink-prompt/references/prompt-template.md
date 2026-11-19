# VINK_PROMPT_INPUT_V1

Use this copyable shape, shortening fields for a small task. This is an agent handoff convention,
not an executable schema or a requirement to build a parser. Replace bracketed descriptions
with observed facts or `UNKNOWN`; preserve the user's constraints and decisions.

```text
VINK_PROMPT_INPUT_V1
REPOSITORY: loveoverflowcom/vink
GOAL: [one observable outcome and why it matters; journey: entry → action → observable result → recovery/return (or real caller/output for backend/library/docs; one sentence suffices for a small fix)]
SCOPE:
  completes: [what this task completes; touchpoints: affected caller, UI/API, durable owner, consumer]
  deferred: [explicit exclusions; for multi-PR journeys only: existing parent, integration owner, dependencies, safe-exposure/rollout rule if partial exposure is unsafe, and final closure scenario]
  delivery_constraints: [user-fixed PR/chat counts if any]
BASE: [explicit user base, task integration base, or develop]
SNAPSHOT: [source branch/SHA; relevant dirty inputs; evidence gaps]
TARGET_ENVIRONMENT: UNKNOWN until the executing agent inspects its machine
SOURCES_AND_OWNERS:
  - AGENTS.md and .agents/skills/README.md
  - [existing product/architecture files, requirement/D/INV/S IDs and actual code paths]
  - [only the relevant vink-* skills]
DECISIONS:
  registry_refs: [when relevant, decision ID@revision, status, source and snapshot hash]
  confirmed:
    - [owner-confirmed rule + authoritative source/ID]
  open_product:
    - [question requiring owner decision; decision_owner: product; affected_requirements: [...]; blocks: [only the dependent slice]]
  open_technical:
    - [technical choice the executing agent resolves from code/evidence; decision_owner: agent; affected_requirements: [...]; blocks: [...]]
  assumptions:
    - [reversible assumption or technical baseline used to proceed + condition to revisit]
LAYOUT_SPEC: [UI only: selected spec/content or accessible path + revision; if missing,
  present the full/short vink-layout-spec (including Interaction field when applicable)
  before UI code/scripts/previews; retain the user's chosen direction and any explicit request to wait; no default approval gate]
UX_CRITERIA: [UI only: the few rubric rows that decide this task, the visual reference class
  (concept / style anchor / regression baseline) with revision and approved|candidate|unknown,
  spec invariants with unit and tolerance; `no approved reference` when none exists]
ACCEPTANCE:
  - [AC-01: observable result, failure case and its authoritative requirement; keep stable IDs and avoid repeating the same acceptance prose across GOAL, SCOPE, ACCEPTANCE and VERIFY]
  - [UI/Interaction: modalities (touch/key/a11y), trigger → feedback → commit/cancel, focus retention/restoration, reduced-motion behavior, Vietnamese IME preservation]
  - [UI: separate goals at 100% and at 200% text; 200% stays legible with hierarchy, not merely non-crashing]
VERIFY:
  - [smallest closure evidence sufficient for the ACs from docs/development (README.md, scoped-workflows.md) and verification.md: cargo xtask verify quick|discussion|expressions|kani|postgres|web, verify unit|web --scenario|preview, --plan, doctor; use ./tools/local-budget run -- on macOS where supported]
  - [UI boundary: JVM unit/offscreen for logic; native host/browser for touch gesture feel, 60fps, insets, screen reader]
  - [UI result table: Functional, Geometry, Visual, Accessibility/Responsive, Interaction, each with scenario/target and PASS/FAIL/NOT_RUN/UNSUPPORTED; opened images named; fix-round budget and stop condition]
BLOCKERS:
  if a runtime dependency is unavailable:
    complete independent work; preserve explicit unavailable/pending runtime behavior
    record the deferred check, prerequisite and resume condition
  if an unresolved decision changes correctness:
    complete unaffected work and ask the specific decision with its source
AUTHORIZED_ACTIONS:
  - [only actions actually granted by the user; local execution scope and remote actions]
HANDOFF:
  report changed behavior, files, scoped completion claims (IMPLEMENTED / VERIFIED / OUTCOME_COMPLETE alongside the four status dimensions), writer/reviewer mapping (AC/claim → connected source/caller → evidence → result → gap), short human smoke card if a main UI journey or residual device risk changed, and material remaining gaps
```

If a task is intentionally review-only or prompt-only, state that scope explicitly and do not
turn future actions described in the prompt into actions performed by the authoring session.
If the user authorized execution and a commit, retain that grant without inventing a new approval.

Keep small tasks lightweight: one sentence suffices for Journey/Touchpoints/Closure on a small
fix, and never force an epic/milestone onto a self-contained task or invent a UI journey for
backend, library or documentation work. For evidence provenance, reuse and human smoke cards,
follow [verification.md](../../vink-engineering/references/verification.md).

For split prompts, include each slice's owned/shared files, contract dependency and integration
order inside each prompt. For continuation, include completed work and valid evidence plus the
concrete next step; do not label deferred integration as PASS.

For UI, use the [shared layout convention](../../vink-layout-spec/references/layout-spec.md)
and include comparison against the chosen spec in ACCEPTANCE/VERIFY. Pass only the decisive
criteria from [UX quality](../../vink-ui-design/references/ux-quality.md) in `UX_CRITERIA`
instead of asking the receiving agent to load every UI reference; omit the field for a copy or
one-property fix whose check is obvious. Never write a candidate reference as approved, and do
not let a round budget or a missing device turn into PASS. Omit LAYOUT_SPEC for
tasks that do not change UI; do not invent a layout stage for a backend or documentation edit.
