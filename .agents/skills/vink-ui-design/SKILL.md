---
name: vink-ui-design
description: Design Vink reading, writing, discovery, collection and workshop experiences with layout alternatives and a visible spec before UI generation. Use for Material 3 Expressive, tokens, Vietnamese typography, copy and accessibility; not backend policy.
---

# Vink UI design

Read the [vision](../../../prd/vision.md), UX/WR/AUD/PUB/COL/HTG/RANK/TOP requirements in
[requirements](../../../prd/requirements.md), the relevant decisions in the
[TOML registry](../../../decisions/README.md) and actual
design/component owners. Use [vink-leptos-web](../vink-leptos-web/SKILL.md) or
[vink-cmp-mobile](../vink-cmp-mobile/SKILL.md) for renderer implementation and
[vink-publication](../vink-publication/SKILL.md) for the meaning of publication states.
When a screen or flow hits an unresolved product rule (`OPEN` items in `O-01..O-19` or new authority/state
questions), clarify it with [vink-shape](../vink-shape/SKILL.md); when a design settles or updates
a documented UX/state contract, record it with [vink-specify](../vink-specify/SKILL.md).
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

Since 10/11/2026 (`PRODUCT-DECISIONS-20261110`), Vink focuses on short-form literary posts and
personal collections; multi-chapter works (`WR-04`), `Tạp Chí` / magazine curation (`ED-02`) and
age policy (`D-11`) are `DEFERRED` (do not include them in default navigation or flows unless the
user explicitly asks to explore them).

| Surface | Expression | Main concern |
| --- | --- | --- |
| Bàn viết / editor | Low | Stable text, quiet tools, reliable save/snapshot/audience state |
| Reader & tombstone | Low | Vietnamese type, reading rhythm, poetry, calm navigation, truthful tombstone (`PUB-04`) |
| Xưởng Viết & discussion | Low–medium | Author feedback intent, two-tier comments, pending/edited marks (`COM-09/10`), endorsement |
| Khám phá / Bộ sưu tập | Medium | Finite discovery (`Đề xuất`, `Top 100`, tags/hashtags) and personal collections (`COL-01..06`) |
| Moderation / author status | Restrained | Separate audience, per-revision review outcome, current public pointer and honest appeal state |

You may design **target UX ahead of backend/runtime completion**. Whenever a design includes
capabilities not yet wired end-to-end in the current checkout, the spec must explicitly separate:
(1) **Target UX vs current runtime behavior**, (2) **Mock/sample fixtures vs live API/store data**,
and (3) **Runtime prerequisites** (migrations, endpoints, workers, auth) required before full
acceptance.

Apply M3 Expressive through hierarchy, shape, color, size, containment and purposeful motion.
Keep text areas quiet; concentrate emphasis on the action or content that matters. Reuse or
extend actual components and semantic tokens. Token values have one authoring place,
[`packages/ui-system/shared/tokens.json`](../../../packages/ui-system/shared/tokens.json):
Vink's Lavender palette in light/dark (system default, `data-theme` override), type roles,
radius/space/density/motion scales and contrast pairs. `cargo xtask ui-tokens` generates the
CSS custom properties, Rust and Kotlin accessors; never edit generated files. Web `vk-*`
components stay in `apps/vink-web/style/vink.css`; the
[UI system](../../../packages/ui-system/README.md) owns renderers and previews, and the
[design baseline](../../../docs/design/README.md) records sources and measured contrast.
Add a role to `tokens.json` before adding a one-off color. Be Vietnam Pro (UI) and Literata (reading)
are self-hosted in `apps/vink-web/assets/fonts` with provenance; a new face, weight or
script needs the same license, size and coverage record. Vink's Lavender is its own palette:
do not import Cantos's tokens, Lavender Stage branding or player patterns.

## Vietnamese text and states

Use real Vietnamese samples with diacritics, long titles, author names, dialogue and poetry.
Check the selected font's glyph coverage, clipping, line height and fallback at target sizes.
Preserve authored paragraphs, line breaks and stanza gaps at narrow widths and larger text.
Do not make a poem look like a chapter list or wrap every paragraph in a separate card.

Use concise Vietnamese labels and accessible names. Follow an existing copy owner when one
exists; a future multilingual system is not required just to ship Vietnamese UI. Keep essential
status/blocker information visible; optional help may use an accessible disclosure. Never put
the only explanation for a disabled action in a hover-only tooltip.

Separate local edits, saving, saved, conflict, audience (“Chỉ mình tôi” / “Công khai”), waiting
for evaluation, retryable/needs-admin hold, policy rejection, current public pointer and tombstone
deletion. Explain what the person can do next without promising an SLA or connected admin notification
channel (`MOD-08`, `MOD-10`). Render facts from their owner instead of duplicating authority checks.

Discovery must work with a small corpus: finite lists, truthful `Top 100` / `Đề xuất` states, a
useful empty state and a clear way to participate. Do not manufacture users, reads, reviews or
engagement. Feedback intent is context, not a star-score ranking or an exemption from policy.

## Accessibility, evidence and references

Check semantic headings/landmarks, named controls, visible focus, keyboard reachability,
dialog focus restoration and alternatives to gestures/drag. Use text/icon/state as well as
color; measure contrast for actual themes. Allow text enlargement and reduced motion without
losing information or waiting for animation to complete an action.

For a visual claim, capture and open the relevant state at a specified viewport, theme,
locale and text scale (`cargo xtask dev`; seed sample data first). Recompute contrast for any
changed color pair in both schemes. Compare before/after under the same scenario when available.
Check loading, empty, error, pending and success states only where they apply. Interactive
behavior, keyboard operation, a named screen reader and motion each require their own
observation; an image cannot establish them.

Report the changed flow/components, inspected artifacts and target conditions, concrete findings
and material coverage gaps. For an audit, tie each finding to an observed problem and its user
impact. A documentation/skill change alone needs link/metadata review, not screenshots.

For a component-by-component audit use [vink-ui-inspector](../vink-ui-inspector/SKILL.md);
for a scoped UI repair or implementation handoff use [vink-ui-fix](../vink-ui-fix/SKILL.md).
Use the [layout template](../vink-layout-spec/references/layout-spec.md) and
[examples](../vink-layout-spec/references/examples.md); LayoutSpec owns the notation, this skill
owns design choices, and a preview or handoff reuses the existing spec instead of keeping a second
tree. Read the applicable references:
[product context](references/product-context.md), [design system](references/design-system.md),
[surfaces and states](references/surfaces-and-states.md), [workflow and evidence](references/chat-workflow.md),
and [source map](references/source-map.md). The bundled context is a dated snapshot; current
repository files and explicit decisions take precedence.
