---
name: vink-how
description: >-
  Trace and explain how an existing Vink feature works through UI, application,
  persistence and tests. Use for codebase questions, onboarding or understanding
  a flow before changing it; use vink-why for historical design rationale.
---

# Vink how

Explain the mechanism that exists in this checkout. Start with [AGENTS.md](../../../AGENTS.md),
the [development guide](../../../docs/development/README.md) and the relevant entry in the
[feature map](../../../docs/development/feature-map.md). Confirm the map against source;
product requirements and architecture proposals describe intent, not executed behavior.

## Trace one concrete journey

- Restate the question as an observable action and result, such as saving an edited draft
  while another tab saves first. Identify the actor, route or command, starting state and
  success/failure the user wants explained. Do not expand into a whole-repository survey.
- Pin branch/SHA and relevant local changes. Locate the entrypoint with `rg`, then follow
  calls and data rather than inferring behavior from names. Read the caller and callee at
  each boundary that affects the answer.
- For web flows, distinguish server HTML, hydration and later browser requests. Trace the
  server's principal resolver, application port, PostgreSQL operation and response handling
  only as far as the question requires. SSR public reads through `PublicReads` do not make
  an HTTP request back to the same server.
- Identify where the actual guarantee lives: document validation, owner/version predicate,
  database constraint, or UI state transition. A disabled button is not server authorization;
  the local principal fixture is not production authentication.
- Follow one relevant rejection or recovery path as well as success. For a save conflict,
  distinguish the stored version, the submitted version and text still held by the browser.
  For public reading, identify the publication snapshot separately from the working draft.

Use the [API contract](../../../services/vink-app/contracts/http/v1/README.md) for transport
semantics and [vink-engineering](../vink-engineering/SKILL.md) or
[vink-leptos-web](../vink-leptos-web/SKILL.md) only when their domain guidance is needed.
If the route or module is absent, explain the nearest implemented seam and link the proposed
requirement; do not invent a call chain for moderation, Discourse or another future feature.

## Explain with inspectable evidence

Lead with what happens for the user, then explain the few boundaries that make it happen.
Link exact files/symbols; include verified line numbers when useful. A short sequence diagram
helps when server/browser or asynchronous boundaries would otherwise be confusing. Teach the
terms through this flow rather than adding a generic framework tutorial.

Keep these claims distinct:

- Source shows a path or guard exists.
- A named test asserts a behavior, but has not necessarily run in this session.
- A command actually ran on stated inputs and produced an observed result.
- A document proposes behavior which the current implementation does not yet provide.

Reading tests is sufficient for a code explanation; run a focused check when the question
needs a runtime result, using the [verification guide](../vink-engineering/references/verification.md).
For intent that code cannot establish, use [vink-why](../vink-why/SKILL.md). End with any material
unknown and the smallest check that would resolve it. An explanation request alone does not
authorize edits; if implementation was also requested, continue with
[vink-work-item](../vink-work-item/SKILL.md) without asking again.
