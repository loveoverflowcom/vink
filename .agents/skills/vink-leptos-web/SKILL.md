---
name: vink-leptos-web
description: >-
  Build or change Vink Leptos SSR/hydration, routes, resources, actions, browser
  state and the JavaScript writing editor. Use for Vietnamese input, autosave,
  revision conflicts and public/private rendering; discover actual package commands first.
---

# Vink Leptos web

Read [AGENTS.md](../../../AGENTS.md), the relevant
[requirements](../../../prd/requirements.md) and
[frontend/editor proposal](../../../docs/architecture/tech-stack.md).
Use [vink-ui-design](../vink-ui-design/SKILL.md) for visual criteria and
[vink-engineering](../vink-engineering/SKILL.md) for durable decisions.
The target is Leptos SSR with a bounded JavaScript editor integration, subject to S-01;
do not inherit Cantos's CSR/Trunk setup.

## The implemented web app

| Concern | Owner |
| --- | --- |
| Shell, routes, hydration entry (`hydrate()`) | `apps/vink-web/src/lib.rs` |
| Pages: home, reader, not-found; Bàn viết | `src/pages/mod.rs`, `src/pages/desk.rs` |
| The web app's own API v1 types and error classification | `src/contract.rs` (checked against the provider fixtures) |
| Data access: SSR through `server::PublicReads`, browser through `/api/v1` | `src/api.rs`, `src/server.rs` |
| Save-state machine (pure, unit-tested) | `src/desk_state.rs` |
| Textarea ↔ document v1, reading view | `src/document.rs` |
| Design tokens and components | `style/vink.css` |
| Browser smoke over the DevTools Protocol | `e2e/browser-smoke.mjs` (run by `verify web`) |

Leptos is pinned at 0.8 (see `Cargo.lock`); check that version's docs when unsure of an API.
`ssr` and `hydrate` are separate builds of one crate: never enable both, gate server-only code
with `#[cfg(feature = "ssr")]` and DOM/fetch code with `#[cfg(feature = "hydrate")]`, and keep
the build without either feature compiling. `vink-app` provides `PublicReads` and `SiteOrigin`
through context; this crate never imports the service. Public pages use `Resource` with
`SsrMode::Async` so status and metadata are final before streaming; account-scoped pages use
`LocalResource`, which stays pending on the server, so private data never enters HTML or the
hydration payload. `Resource` needs `Send` futures: wrap browser fetches in `SendWrapper`. Build
commands (`cargo xtask build`, wasm-bindgen version pin) are in
[docs/development](../../../docs/development/README.md).

## Place the change

For UI changes, consume the selected [LayoutSpec](../vink-layout-spec/SKILL.md).
If absent, complete the appropriate full/short spec and show it before editing
components/CSS/JS or generating a preview. Reuse a still-current spec already shown;
do not reopen the user's selected direction. Map regions to actual Leptos/component
owners and tokens. Explain structural/CTA changes in the spec before implementation,
then compare the result against it. Pure transport/state changes with no UI change
do not need a layout exercise. Continue within existing implementation authorization.

Keep presentation separate from application and transport: components render typed states
and dispatch intents; resources/actions coordinate requests; adapters own URLs, DTOs and
error classification. SSR can call application services directly through the composition root.
Browser code must not import server persistence or secrets; use actual feature/build boundaries
for server-only and DOM-only code. Share transport-independent rules only where there is a
real consumer and no private data crosses into the browser build.

## Public and private surfaces

- Public reader HTML, canonical URL and metadata come from an authorized publication snapshot.
  Hydration and later API reloads must enforce the same access policy.
- Private drafts, staff fields and credentials cannot appear in public HTML, serialized
  hydration state, error pages, shared caches or browser configuration. Sanitize document
  rendering and imported rich text; do not trust editor-generated HTML as safe server output.
- Public content caches need revision identity plus current access/invalidation policy. Keep
  account-scoped data out of shared caches; clear scoped state on logout or account change.
- Resolve acting Discourse identity on the server. The browser neither supplies a trusted
  `Api-Username` nor receives the API key. Apply the selected session/CSRF/Origin policy.
- Use [vink-publication](../vink-publication/SKILL.md) for gates. A button's enabled state is
  a projection, not permission to publish, moderate or act as another user.

## Editor and async state

Treat WR-01–WR-03 and S-01 as the acceptance boundary. The desk is a textarea smoke over
document v1 with manual save; it does not complete S-01. Reuse `EditorSession` for autosave or
a JavaScript editor instead of adding a second save-state model. Choose the canonical
document/schema version through the scoped design; derived HTML/text must not become competing
sources.
The JS editor bridge owns mount, subscriptions, selection and teardown. Preserve the editor
instance across ordinary reactive updates; dispose listeners and timers when its owner ends.
Use stable domain IDs for dynamic rows instead of array positions.

Preserve Vietnamese composition, undo/redo, selection, line breaks and poem stanzas across
save/render/edit. Do not submit partial composition or overwrite in-progress typing with a
server echo. Validate schema and sanitize at the server boundary as well as in the editor.

Autosave distinguishes local edits, saving, server-confirmed saved, failed, conflict and
uncertain outcome. Bind a request to account, document, submitted draft version and request ID.
A late acknowledgment of version 7 cannot mark newly typed version 8 as saved. A conflict or
timeout preserves the user's text and offers the relevant retry/reconcile/export action.
Two tabs cannot silently overwrite one another. A local recovery copy is not server persistence;
if implemented, scope it to the account and consider shared-device cleanup.

Represent loading, empty, unavailable, validation failure, pending moderation and successful
results explicitly. Key requests to their subject/account; cancel or ignore stale replies after
navigation, withdrawal or logout. Each mutation defines duplicate-submit and retry behavior;
an unkeyed write with an unknown remote result requires reconciliation, not blind replay.

## Verification and handoff

Use the [verification guide](../vink-engineering/references/verification.md). `verify web`
covers server HTML/privacy, hydration, client navigation, save/reload/conflict and restart
with programmatic input; extend `e2e/browser-smoke.mjs` for new interactions. Select relevant
checks for server HTML/privacy, real browser hydration/navigation, resource races and editor
save/reload/conflicts. Test representative Vietnamese poetry/prose, Word/Docs paste, undo and
mobile text scaling. Distinguish simulated input from a real named OS IME run.

For visual changes, drive and open captures of relevant states with viewport/theme/locale;
inspect focus and keyboard separately. A string render or screenshot does not prove editor
interaction. Report rendering/build mode, actual commands, browser/backend used, covered async
races and unrun acceptance. A skill-only edit needs no application run.
