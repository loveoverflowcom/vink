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
do not inherit Cantos's CSR/Trunk setup or assume these packages already exist.

## Discover and place the change

Find actual manifests, Leptos version/features, route and component owners, server/browser
build targets, CSS tokens and supported commands. Library APIs depend on the pinned version;
check that version's official docs when uncertain. Example directories in the foundation are
not instructions to scaffold a workspace for a small edit.

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

Treat WR-01–WR-03 and S-01 as the acceptance boundary. Choose the canonical document/schema
version through the scoped design; derived HTML/text must not become competing sources.
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

Use the [verification guide](../vink-engineering/references/verification.md). Select relevant
checks for server HTML/privacy, real browser hydration/navigation, resource races and editor
save/reload/conflicts. Test representative Vietnamese poetry/prose, Word/Docs paste, undo and
mobile text scaling. Distinguish simulated input from a real named OS IME run.

For visual changes, drive and open captures of relevant states with viewport/theme/locale;
inspect focus and keyboard separately. A string render or screenshot does not prove editor
interaction. Report rendering/build mode, actual commands, browser/backend used, covered async
races and unrun acceptance. A skill-only edit needs no application run.
