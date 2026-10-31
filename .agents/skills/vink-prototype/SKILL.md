---
name: vink-prototype
description: >-
  Build and evaluate a bounded Vink experiment to resolve a concrete uncertainty,
  including S-01–S-05 spikes or competing UI/API approaches. Use when the user asks
  to try or compare implementations; keep prototype evidence separate from full
  feature acceptance and production readiness.
---

# Vink prototype

Build the smallest experiment that can change a decision. Read [AGENTS.md](../../../AGENTS.md),
the relevant S/D/INV items in the [decision log](../../../docs/architecture/decisions.md),
current source and [development setup](../../../docs/development/README.md). Preserve existing
edits and the user's branch/scope through [vink-work-item](../vink-work-item/SKILL.md).

## Define what the experiment can decide

State the uncertainty, baseline, proposed variant, observable success/failure and stopping
condition before coding. A single variant is enough when it resolves the question; compare
several only when there is a real choice. Use one representative Vietnamese text or caller
journey and a failure case that could invalidate the approach.

Write down assumptions and substitutions. A textarea, local identity fixture, fake adapter
or static UI can validate a limited seam; none validates a production editor, auth, remote
integration or publication workflow. Avoid numeric performance thresholds unless supplied,
already owned by the requirement, or clearly declared as provisional experiment criteria.

## Implement at the existing seam

Extend the current app/module instead of scaffolding a competing Vink. Isolate an alternative
behind a small local switch only if comparison needs it; do not add a permanent flag platform.
For a library/API experiment, start from a small usage example and the provider's owned contract.
Use synthetic text and fixture identities. Private drafts are not provider test data by default.

| Uncertainty | Useful starting seam | Evidence still needed beyond a small demo |
| --- | --- | --- |
| S-01 editor/SSR | [DocumentView](../../../apps/vink-web/src/document.rs), [Editor](../../../apps/vink-web/src/pages/desk.rs), [EditorSession](../../../apps/vink-web/src/desk_state.rs) | Real OS IME/device input, paste/undo, unsaved-text recovery, two tabs and schema round-trip |
| S-03 identity | `services/vink-app/src/identity.rs` | A selected real authority; linking, logout/ban, forged requests and session/CSRF behavior |
| S-04 durable changes | `services/vink-app/src/application/`, `persistence/`, PostgreSQL tests | Real transaction/crash/replay observations; local CAS does not prove remote fencing |
| S-02 Discourse | No adapter/plugin exists in the bootstrap; inspect current source first | Pinned release/plugin/config and enabled entrypoints including direct access |
| S-05 AI/operations | Worker is lifecycle-only in the bootstrap | Scoped provider/data authorization, untrusted-output handling and evaluation/queue evidence |

Use [vink-leptos-web](../vink-leptos-web/SKILL.md),
[vink-ui-design](../vink-ui-design/SKILL.md), or [vink-engineering](../vink-engineering/SKILL.md)
for the affected implementation. Do not weaken identity, privacy, validation or version
guards merely to make the demo pass. New migrations follow the existing migration rules.

## Observe and compare

Choose checks from [verification.md](../vink-engineering/references/verification.md) and commands
from the development guide. Exercise the real boundary that decides the question: PostgreSQL
for durability, a browser for hydration, a pinned service for integration. Keep variants on the
same fixture, viewport/theme or workload; record differences that prevent a fair comparison.
For visual claims, capture and open the images using available tools. The existing browser
smoke does not itself capture screenshots or verify native IME.

Keep command, input revision/diff, environment, observation and artifact location together.
A planned measurement is not a result; report unavailable checks as `NOT_RUN`/`UNSUPPORTED`
or preserve a runner's `FAIL` for missing dependencies. Continue independent parts without
replacing a missing real boundary with a fake success.

Stop when the declared question is answered or the next experiment needs new scope/access.
When a run fails, investigate a concrete hypothesis before retrying. Preserve useful evidence
and remove only temporary processes/files owned by the experiment; do not reset user changes
or delete their data to restore a baseline.

## Hand off the decision evidence

Report the finding, reproducible path, limits, and whether to keep, revise or drop the variant.
Separate completing this experiment from completing the whole S spike. Name the smallest next
step to production acceptance, including any guard or device coverage the prototype lacks.
Use the owning decision/development document for requested durable notes; do not create a
second status system. If implementation beyond the experiment is already requested, continue
within that scope. A prototype does not accept an open D decision, deploy or publish itself.
