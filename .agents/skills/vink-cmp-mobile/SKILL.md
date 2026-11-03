---
name: vink-cmp-mobile
description: >-
  Build, fix or review Vink Kotlin Compose Multiplatform UI for Android/iOS,
  shared primitives/patterns, native diagnostic hosts and Desktop previews.
  Use for Compose layout, controlled input, theme, navigation, lifecycle and
  native UI evidence; browser routes and CSS belong to vink-leptos-web.
---

# Vink CMP mobile

Use this beside [vink-ui-design](../vink-ui-design/SKILL.md) for visual criteria,
[vink-ui-fix](../vink-ui-fix/SKILL.md) for a bounded repair and
[vink-code-review](../vink-code-review/SKILL.md) for review-only work. Before changing
UI, show the selected [LayoutSpec](../vink-layout-spec/SKILL.md), including the
affected narrow/large-text state. Continue work already authorized by the user.

## Choose the actual owner

Read [owners and evidence](references/owners-and-evidence.md) for source-set paths
and target-specific checks, then inspect the current manifests and call sites.
The [development guide](../../../docs/development/README.md) owns commands;
[target readiness](../../../docs/development/ui-system-qualification-evidence.md)
owns recorded results and gaps.

Vink currently has a shared `:ui` library, a `:patterns` library, common diagnostic
specimens, a JVM Desktop preview and opt-in Android/iOS diagnostic hosts. There
is **no native product app**, account/session, draft persistence or service-backed
native editor. Recipes marked `[Mẫu]` remain samples. A task to fix a component
does not authorize scaffolding a native product or copying Web/Cantos services.

- Primitive/state/theme/navigation API: `packages/ui-system/cmp/ui`.
- DTO-free literary compositions: `packages/vink-patterns/cmp`.
- Sample fixtures, replay, inspector and recipes: CMP `:diagnostics`.
- OS lifecycle, insets, keyboard and launch adapters: the corresponding native host.
- Browser resources, routes, CodeMirror, DOM and CSS: [vink-leptos-web](../vink-leptos-web/SKILL.md).

Search the [API admission map](../../../packages/ui-system/shared/api-coverage.json),
[catalog](../../../packages/ui-system/shared/preview-catalog.json) and existing
`Vink*` components before creating controls. Follow
[EXTENDING](../../../packages/ui-system/EXTENDING.md) when changing a public API:
declare semantics, keep the Leptos counterpart or an explicit unsupported reason,
and register/replay the affected scenario. Shared API does not imply identical
platform pixels or native acceptance.

## Compose and state boundaries

Keep `commonMain` independent of platform APIs, HTTP, SQL, auth and service DTOs.
Inject platform facts/effects from `androidMain`, `iosMain` or the host; use
`expect`/`actual` only where an actual shared port needs it. Avoid a dependency or
platform abstraction without a consumer. Check the pinned Gradle artifacts and
official documentation before adopting an API; Android availability does not prove
iOS or common availability.

Use immutable view values and typed intents with one authority at the correct
lifetime. Pure Kotlin policies in `VinkState.kt`/`VinkNavigation.kt` own their
invariants; renderers display facts and invoke caller callbacks. Keep fetch,
retry, permission and uncertain-write reconciliation at the consumer. A sample
reducer cannot confirm that a server saved or published anything.

Scope coroutines/listeners to their actual owner, dispose observers, preserve
cancellation and ignore stale completions after owner/session changes. Recomposition
is not permission to issue requests. Keep a stable composition call site and keys
through theme, size and navigation changes; do not recreate state just to redraw.

Fields remain controlled `TextFieldValue` values. Preserve selection, composition,
undo where supported, Vietnamese NFC/NFD, paragraphs and poem stanzas. Do not send
partial IME composition or replace current input with a late acknowledgment. A
native product editor, autosave or durable recovery requires its own explicit scope
and owner; the current text-field sample does not implement them.

## Vink visual and platform policy

Use generated `VinkTokens.kt`, `VinkTheme`/`VinkScope` and `LocalVink`; values come
from [tokens.json](../../../packages/ui-system/shared/tokens.json). Keep Vink's
Lavender identity, semantic roles, typography and Vietnamese copy. Do not edit
generated Kotlin or introduce a second palette. Host-injected system dark/motion,
text scale and activity flow down; an inactive ancestor or reduced motion must
stop loops without losing the state label.

Use the pinned stable Material3 surface, not an assumed Expressive alpha API.
Inspect real text measurement, dp bounds, density and OS font scale; reflow rather
than fixing heights that clip Vietnamese marks. Test light/dark, narrow windows
and 200% text for affected states. Preserve accessible names, headings, roles,
disabled/pending reasons and visible focus. Touch targets, safe drawing/IME insets,
Back/overlay order and focus return need evidence on the host that owns them.

Fonts are host-supplied `VinkFonts`. Desktop loads the pinned TTF derivatives of the
self-hosted WOFF2 faces (one provenance owner in the web font asset directory);
native currently uses system fonts until a licensed native font pipeline exists.
Report actual decode/fallback, not parity inferred from the Web font files.

## Verify the claimed boundary

Choose the cheapest meaningful common/Compose regression, then the renderer/host
needed by the change. Use [verification](../vink-engineering/references/verification.md)
and the linked development commands. Do not create a runner just to validate this
skill, or copy Cantos command/module names into Vink.

For screenshots, use [vink-ui-capture](../vink-ui-capture/SKILL.md). Preserve catalog/
scenario, source SHA/dirty fingerprint, fixture, renderer/host, requested/effective
theme, logical dp versus image pixels, density, OS scale versus preview multiplier,
insets, fonts and motion. Open cited images. Off-screen JVM, Android device/emulator
and iOS simulator/device are distinct evidence. Compile proves no runtime; a static
semantics tree proves no TalkBack/VoiceOver walkthrough or OS IME behavior.

Report actual PASS/FAIL/NOT_RUN/UNSUPPORTED by command and target, preserve failed
runner results and state the missing prerequisite. Device absence does not block
independent source/common/Desktop work. Review-only requests return findings;
publication, external messages and merge follow the user's actual authorization.
