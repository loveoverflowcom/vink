---
name: vink-leptos-web
description: Build or change Vink Leptos routes, browser state and the writing editor in standalone CSR or SSR/hydration. Use for public and private rendering, Vietnamese input, autosave and revision conflicts.
---

# Vink Leptos web

## Choose the rendering path

Trace the affected route through [the shared router](../../../apps/vink-web/src/lib.rs) and
identify how its first document and data arrive.

| Path | Where the route gets its data | What to check |
| --- | --- | --- |
| Standalone CSR (`csr`, `mount()`) | A static document boots the router. The browser reads same-origin `/api/v1` from the API-only App. | Direct entry, loading or API failure, then the visible result after mount. Use `cargo xtask build spa` for the bundle and explicitly select the `standalone` browser scenario. [Build, API boundary and rollout](../../../docs/development/edge-frontend.md) |
| SSR and hydration (`ssr`, then `hydrate`) | Axum provides `PublicReads` and `SiteOrigin`. Public routes using `SsrMode::Async` can set status, content and metadata in the initial HTML. The browser hydrates that DOM and makes later reads over HTTP. | Inspect the first response when changing server status, headers, metadata or no-JS content. Exercise hydration and client navigation for interactive changes. [Build and scenario catalog](../../../docs/development/scoped-workflows.md#scenario-web-và-setup) |

A shared route, component or browser adapter may affect both paths. Put shared DOM and fetch
code behind `feature = "browser"`, server code behind `feature = "ssr"`, and mode-specific
boot code behind `csr` or `hydrate`. Keep the featureless build working. Match the APIs to
the versions in [the manifest](../../../apps/vink-web/Cargo.toml) and the workspace lockfile.

## Find the owner

| Concern | Owner |
| --- | --- |
| Route, shell and page | `apps/vink-web/src/lib.rs`, `src/shell.rs`, `src/pages/` |
| Public reads, browser requests and response types | `src/api.rs`, `src/server.rs`, `src/contract.rs`. Reuse the browser fetch wrapper for futures used by a `Resource`. |
| Draft save state, tags and leave intent | `src/desk_state.rs`, `src/tag_state.rs`, `src/navigation.rs` |
| Canonical document and CodeMirror 6 bridge | `src/document.rs`, `src/document/markdown.rs`, `src/editor.rs` and the TypeScript runtime under `apps/vink-web/editor/` |
| Styling and shared components | `packages/ui-system/shared/tokens.json`, `vink-ui` and `vink-patterns`. [Component owners](../vink-ui-design/references/source-map.md#owner-map-cho-bàn-giao-code) · [Tailwind syntax, tokens and cascade](references/tailwind.md) |

## Place the change

Select the host before editing a route or manifest. Web hosts real reader/writer operations; example fixtures belong to Preview.
See the [Web/Preview boundary](../../../docs/development/web-preview-boundary.md).

For UI changes, consume the selected [LayoutSpec](../vink-layout-spec/SKILL.md).
If absent, complete the appropriate full/short spec and show it before editing
components/CSS/JS or generating a preview. Reuse a still-current spec already shown;
do not reopen the user's selected direction. Map regions to actual Leptos/component
owners and tokens. Explain structural/CTA changes in the spec before implementation,
then compare the result against it. Pure transport/state changes with no UI change
do not need a layout exercise. Continue within existing implementation authorization.

Apply the interaction UX standards from [Interaction UX](../vink-ui-design/references/interaction-ux.md):
preserve keyboard reachability, visible focus rings, dialog focus containment and restoration; never
steal focus when hydration completes, when an async resource resolves or when an action finishes.
Respect `prefers-reduced-motion` via generated `tokens.css` (instant transitions, stopped loops).
Preserve Vietnamese IME composition (Telex/VNI) and selection in text inputs and the CodeMirror 6
editor (`editor/`); do not remount input nodes across state updates. Keep scroll positions stable
across re-renders.

Keep presentation separate from application and transport: components render typed states
and dispatch intents; resources/actions coordinate requests; adapters own URLs, DTOs and
error classification. Use the App-provided context for SSR reads.
Browser code must not import server persistence or secrets; use actual feature/build boundaries
for server-only and DOM-only code. Share transport-independent rules only where there is a
real consumer and no private data crosses into the browser build.

## Public and private surfaces

- Render the Reader from its authorized projection, including replacement notices. Apply its
  access rules to SSR metadata, CSR data, later reads and cached content.
- Use browser-only reads (such as `LocalResource`) for account-scoped data.
  Private drafts, staff fields and credentials cannot appear in public HTML, serialized
  hydration state, error pages, shared caches or browser configuration. Sanitize document
  rendering and imported rich text; do not trust editor-generated HTML as safe server output.
- Public content caches need revision identity plus current access/invalidation policy. Keep
  account-scoped data out of shared caches; clear scoped state on logout or account change.
- Resolve the acting principal on the server. The browser never supplies a trusted user ID,
  role or display name for discussion commands. Apply the selected session/CSRF/Origin policy.
- Use [vink-publication](../vink-publication/SKILL.md) for gates. A button's enabled state is
  a projection, not permission to publish, moderate or act as another user.

## Editor and async state

The desk edits the canonical `{format, source}` document (`default` or `markdown`) in one
CodeMirror 6 `EditorView`. Preserve buffer and selection when reconfiguring the Markdown
extension. A server-confirmed format conversion applies the converted source and moves the
cursor to the beginning.

For a new draft, the author triggers the first “Gửi”. Subsequent text edits can autosave.
Tags have a separate version and require an explicit save. Use `EditorSession` and read
the body through `BodySource::snapshot`. Route exits through `NavigationGuard` so the
author can handle pending edits before leaving. [Editor data and save behavior](../../../docs/development/public-read-freshness.md#luồng-và-lifetime-hiện-hành)

The JS editor bridge owns mount, subscriptions, selection and teardown. Preserve the editor
instance across ordinary reactive updates; dispose listeners and timers when its owner ends.
Use stable domain IDs for dynamic rows instead of array positions.

Preserve Vietnamese composition, undo/redo, selection, line breaks and poem stanzas across
save/render/edit. Do not submit partial composition or overwrite in-progress typing with a
server echo. Validate schema and sanitize at the server boundary as well as in the editor.

Autosave distinguishes local edits, saving, server-confirmed saved, failed, conflict and
uncertain outcome. Bind a request to account, document, submitted draft version and request ID.
A late acknowledgment of version 7 cannot mark newly typed version 8 as saved. A conflict or
timeout preserves the user's text while the author reconciles versions or retries a safe write.
Two tabs cannot silently overwrite one another. A local recovery copy is not server persistence;
if implemented, scope it to the account and consider shared-device cleanup.

Represent loading, empty, unavailable, validation failure, pending moderation and successful
results explicitly. Key requests to their subject/account; cancel or ignore stale replies after
navigation, withdrawal or logout. Each mutation defines duplicate-submit and retry behavior;
an unkeyed write with an unknown remote result requires reconciliation, not blind replay.

## Verification and handoff

Find the journey in the [scenario catalog](../../../docs/development/scoped-workflows.md#scenario-web-và-setup).
Follow the selected phase through [the dispatcher](../../../apps/vink-web/e2e/browser-smoke.mjs)
to the function that exercises the behavior. Extend that assertion alongside its fixture.
For example, the `editor` scenario calls [the autosave tests](../../../apps/vink-web/e2e/editor-autosave.mjs),
and `standalone` calls [the CSR journey](../../../apps/vink-web/e2e/auth-password-smoke.mjs).
Run `cargo xtask verify web --scenario <name>` for the affected path. If a shared change
affects both renderers, select `standalone` and the relevant SSR scenario. The default Web
suite omits `standalone`.

For editor changes (WR-01–WR-03, S-01), exercise the relevant failure with Vietnamese prose
or verse. Check composition, paste, undo, late save responses or leaving with unsaved text as
the change requires. Report synthetic Chrome input and named OS IME runs separately.
[Verification guide](../vink-engineering/references/verification.md)

For visual changes, drive and open captures of relevant states with viewport/theme/locale;
inspect focus and keyboard separately. A string render or screenshot does not prove editor
interaction. Take the criteria, the 100% versus 200% goals and the five-dimension result table
from [UX quality](../vink-ui-design/references/ux-quality.md). Browser measurements (DOM rects,
computed styles, `document.fonts`) answer only the spec invariants they were taken for and are
recorded per the capture [evidence contract](../vink-ui-capture/references/evidence-contract.md);
the preview's 200% is a root font-size multiplier, not browser zoom, and a full-page capture that
resizes the viewport does not show how a viewport-anchored dialog or sheet sits on a real screen. Report rendering/build mode, actual commands, browser/backend used, covered async
races and unrun acceptance. A skill-only edit needs no application run.
