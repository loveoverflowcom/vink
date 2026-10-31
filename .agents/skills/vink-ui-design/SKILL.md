---
name: vink-ui-design
description: >-
  Design Vink reading, writing, workshop and magazine experiences with layout
  alternatives and a visible spec before UI generation. Use for Material 3
  Expressive, tokens, Vietnamese typography, copy and accessibility; not backend policy.
---

# Vink UI design

Read the [vision](../../../prd/vision.md), UX/WR/ED requirements in
[requirements](../../../prd/requirements.md), and actual design/component owners.
Use [vink-leptos-web](../vink-leptos-web/SKILL.md) for renderer implementation and
[vink-publication](../vink-publication/SKILL.md) for the meaning of publication states.
For an audit-only request, inspect and report; fixing requires scope already granted by the user.

## Design for the literary task

Before styling, name the reader/writer's goal, primary content, primary action and state.
Use [vink-layout-spec](../vink-layout-spec/SKILL.md) as the shared layout convention:
brief → alternatives when needed → recommendation → selected spec → authorized UI
generation → comparison against that spec. Publish the spec in a progress message or
linked document before the first UI code edit, generation script or preview call.
For a new screen or substantial redesign without a chosen direction, compare 2–3
different structures with the same inputs, then recommend and detail one. Respect the
user's requested count and an already selected direction. For a small fix, use the
short regional spec; audit-only work observes and reports without creating a new design.
This adds no approval gate: continue when prototype/implementation is authorized;
stop at sketches or wait for selection only when requested. The spec owns any later
structural/CTA changes as well; explain and update it before changing the UI again.

The three spaces are experience scopes, not a mandate for three apps or fixed tabs.

| Surface | Expression | Main concern |
| --- | --- | --- |
| Bàn viết / editor | Low | Stable text, quiet tools, reliable save/revision state |
| Long-form reader | Low | Vietnamese type, reading rhythm, poetry, calm navigation |
| Xưởng Viết | Low–medium | Author feedback intent and constructive, contextual responses |
| Tạp Chí / discovery | Medium | Editorial hierarchy, a small useful collection and visible selection context |
| Moderation / author status | Restrained | Exact snapshot, reason, next action and current authority |

Apply M3 Expressive through hierarchy, shape, color, size, containment and purposeful motion.
Keep text areas quiet; concentrate emphasis on the action or content that matters. Reuse or
extend actual components and semantic tokens. The token owner is
`apps/vink-web/style/vink.css`: warm paper surfaces, ink-indigo primary, seal-vermilion
accent, light/dark schemes, radius/space/motion scales and `vk-*` components. Add a token
there before adding a one-off color. Fonts are local stacks with Vietnamese coverage; adding a
web font needs an asset decision. Cantos's lavender branding and player patterns are not defaults.

## Vietnamese text and states

Use real Vietnamese samples with diacritics, long titles, author names, dialogue and poetry.
Check the selected font's glyph coverage, clipping, line height and fallback at target sizes.
Preserve authored paragraphs, line breaks and stanza gaps at narrow widths and larger text.
Do not make a poem look like a chapter list or wrap every paragraph in a separate card.

Use concise Vietnamese labels and accessible names. Follow an existing copy owner when one
exists; a future multilingual system is not required just to ship Vietnamese UI. Keep essential
status/blocker information visible; optional help may use an accessible disclosure. Never put
the only explanation for a disabled action in a hover-only tooltip.

Separate local edits, saving, saved, conflict, waiting for moderation, policy rejection,
publication and magazine selection. Explain what the person can do next. Do not label an
AI suggestion as a final human decision or an approved revision as already public. Render
facts from their owner instead of duplicating authority checks in components.

Discovery must work with a small corpus: finite collections, a useful empty state and a clear
way to participate. Do not manufacture users, reads, reviews or engagement. Feedback intent
is context, not a star-score ranking or an exemption from community policy.

## Accessibility and evidence

Check semantic headings/landmarks, named controls, visible focus, keyboard reachability,
dialog focus restoration and alternatives to gestures/drag. Use text/icon/state as well as
color; measure contrast for actual themes. Allow text enlargement and reduced motion without
losing information or waiting for animation to complete an action.

For a visual claim, capture and open the relevant state at a specified viewport, theme,
locale and text scale (run the app with `cargo xtask dev`; seed sample data first). Recompute
contrast for any changed color pair in both schemes. Compare before/after under the same scenario when available. Check
loading, empty, error, pending and success states only where they apply. Interactive behavior,
keyboard operation, a named screen reader and motion each require their own observation;
an image cannot establish them.

Report the changed flow/components, inspected artifacts and target conditions, concrete
findings and material coverage gaps. For an audit, tie each finding to an observed problem
and its user impact. A documentation/skill change alone needs link/metadata review, not screenshots.

## Design workflows and references

For a component-by-component audit use [vink-ui-inspector](../vink-ui-inspector/SKILL.md);
for a scoped UI repair or implementation handoff use [vink-ui-fix](../vink-ui-fix/SKILL.md).
These workflows complement, rather than replace, the existing Leptos and publication owners.

Use the [layout template](../vink-layout-spec/references/layout-spec.md) for the selected
direction and [examples](../vink-layout-spec/references/examples.md) when useful. LayoutSpec
owns the notation; this skill owns design choices. Reuse the existing spec instead of
maintaining a second tree in a preview or handoff.

Read the applicable references: [product context](references/product-context.md),
[design system](references/design-system.md), [surfaces and states](references/surfaces-and-states.md),
[workflow and evidence](references/chat-workflow.md), and [source map](references/source-map.md).
The bundled context is a dated snapshot; current repository files and explicit decisions take precedence.
