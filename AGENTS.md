# Agent instructions — Vink

Vink (Việt Ink) is a Vietnamese literary community. Start with [README.md](README.md),
the [documentation map](docs/README.md), then the requirements and sources nearest the task.
The user's current instructions take precedence over repository conventions.

## Repository reality and decisions

The repository holds product/architecture proposals, agent skills and a bootstrap skeleton:
a Cargo workspace with the Leptos SSR + Axum app and its hydration bundle (`apps/vink-web`,
`services/vink-app`), PostgreSQL migrations, an owned PostgreSQL container with per-owner
database/role provisioning (`services/postgres`: App `vink` and Identity `vink_identity`;
runtime and migrator credentials split) and a shared pool/migration package
(`packages/store-postgres`), a separate `services/vink-identity` process (own database,
migrations and draft private contract on a Unix socket; no auth authority, so it answers
`authority_not_configured`; the App consults it through its own client only for a presented
`Authorization: Bearer` credential and never maps a subject to a principal), a draft/reader slice with a
CodeMirror 6 body editor
(`apps/vink-web/editor`, pinned npm runtime), an adaptive 220/96 rail/mobile bar with local
appearance preferences and a truthful `/toi` utility, a finite v4 public reading stream with
loaded-list title/author search and URL-based reader return context, conditional public reads
(READ-01: strong body-hash ETag, `304`, `private, no-cache`; no server cache) with a browser-tab,
memory-only cache-first owner for those two reads (READ-02 Web pilot: ~120 s revalidation while
visible, last success kept on temporary failure, removed on 404; CMP/native not implemented), a quiet reader, a private
draft dashboard with loaded-list search/resume and uncertain-create reconciliation, a lifecycle-only worker,
`cargo xtask` lanes and CI, and the UI System foundation (`packages/ui-system`: one token source in
`shared/tokens.json` generating CSS/Rust/Kotlin with a drift check, the Leptos crate `vink-ui`, a
Compose Multiplatform `:ui` library and two previews; native Android/iOS targets compile only
when enabled and no native product app exists) with UIS-02 primitives on both renderers (text/layout,
actions, fields, icons from a licensed registry, avatar/byline with shared identity vectors) and
UIS-03 state/feedback/loading components (content region with lawful content × phase combinations,
state panel, notices, skeleton/arc/edge progress gated by host-injected activity, snackbar,
dialog, shared state vectors), and UIS-04 shell/navigation primitives
(pure `NavLayout`/`OverlayLayer` policy with shared vectors, adaptive scaffold, rail with a permanent
toggle, bottom bar, top bar; a diagnostic CMP navigation adapter) that the Web shell now renders.
UIS-05 adopts shared controls, identity, notices and dialog chrome on Explore/Reader/Desk/Editor/Me; `packages/vink-patterns` owns DTO-free Leptos/CMP compositions
consumed by the app and both galleries. Feedback and native screen recipes remain explicitly
labelled samples with no service, auth or save authority. UIS-06 adds an API admission map,
scoped style/dependency guards and opt-in Android debug/iOS simulator diagnostic hosts sharing
the CMP specimens with Desktop (`:diagnostics`); these are not native product apps. Target
readiness and runtime limits live in `docs/development/ui-system-qualification-evidence.md`.
[docs/development](docs/development/README.md) owns its layout, commands, evidence and limits.
EMO-01A adds a 16-asset Google Noto manifest in `packages/expressions`, generated Rust/CMP
lookup contracts, a pure Rust admission/generation policy consumed by the isolated Web route
`/thu-nghiem/bieu-cam`, and a self-hosted dotLottie Software renderer loaded only after activation.
Expression lifecycle Quint simulation/trace replay has its own `verify expressions` lane;
owner-local Kani 0.67.0 harnesses check that policy and the App's pure discussion core against
production source (`verify kani`, pinned verifier installed separately). CMP is contract/Unicode
fallback only; no native animation renderer, composer integration, durable stickers/reactions or
backend performance qualification is implied. [Expression scope](docs/development/expressions.md)
records provenance, licensing, model/proof bounds and remaining acceptance.
LVE-PR-01 adds the owner-local pure `domain::endorsement` seam for work revisions and original
root/flat replies, accepted-order fencing including same-state intents, exact receipts and checked
counts. Its Quint/replay shares `verify discussion`; its six Kani obligations are registered in the
existing App manifest/runner (19 App harnesses total). No SQL/HTTP/UI consumer exists yet; see
[policy](docs/architecture/literary-endorsement.md) and [registry/gaps](docs/development/endorsement-core.md).
No auth authority, login/session, publish/moderation workflow, AI provider or job exists;
production authentication is NOT_IMPLEMENTED. Vink owns discussion (owner decision 03/11/2026,
[ADR](docs/architecture/discussion-ownership.md)): the App has a pure two-tier discussion core
(structure, body, lifecycle, per-work comments mode, permissions, viewer visibility, idempotency,
paging), application use cases, a PostgreSQL adapter/migration with real-database tests, HTTP API
v1 discussion routes with contract fixtures (read, submit, remove own, author toggle, moderation
queue/decisions), DB-held moderator grants managed only by `vink-app moderator grant|revoke|list`
and rechecked at each decision's commit (moderators recuse from their own comments and works),
plus scoped Quint models with trace replay (`services/vink-app/behavior`: structure, comment
lifecycle/mode/moderation authority with submission receipts, and reactions). Reactions exist only
as pure decisions (desired state, generation fence, interaction limits, count projection; semantics
proposed, not product-accepted) with no table, store, route or UI. Production
writes answer `401` (no auth authority). The reader now has anonymous discussion SSR, browser paging and scoped draft/write-intent UI; real local writes are exercised only with the existing loopback LocalFixture, clearly labelled. Production write readiness, Google picker/sticker, reaction persistence/API/UI, ranking, off-page context lookup and native service integration remain unavailable. Inspect actual
files before claiming a capability; update this paragraph when implementation changes those facts.

- [Vision](prd/vision.md) and [requirements](prd/requirements.md) own product intent and IDs.
- [Tech stack](docs/architecture/tech-stack.md): Vink owns literary content and, since 03/11/2026,
  discussion in PostgreSQL. Discourse is a design reference only: never a runtime dependency,
  comment store, auth source or plugin target. Leptos SSR/Axum and a background worker are proposed.
- [Decision log](docs/architecture/decisions.md) owns open D-01–D-12 decisions, INV-01–INV-08
  and S-01–S-05 spikes. Importing documents or passing a check does not accept a proposal.
- Distinguish proposed, implemented and verified. A scoped prototype may explore a decision
  without settling it; record its assumptions and the evidence still needed.
- [Archived input](docs/archive/initial/README.md) is historical context. Preserve it; current
  docs and explicit user decisions guide new work.

## Engineering boundaries

Use the [skill index](.agents/skills/README.md) to load only relevant guidance. Prefer a modular
monolith and concrete owners over copying Cantos's services, crates, toolchain or deployment.
The following boundaries guide work within the selected scope:

- Vink is independent of Cantos: PostgreSQL for Vink business data; no shared Turso store,
  password store, cookies or session tables. Audio adaptation needs its own consent and scope.
- Keep domain decisions independent of Axum, SQLx, Leptos and AI provider SDKs.
  Application code orchestrates; adapters perform I/O; UI renders facts and submits intents.
- Keep private working drafts, immutable submitted revisions, moderation decisions, publication
  and curation distinct. Recheck current authority and version at durable commit boundaries.
- Discussion is the App's `discussion` module: one writer/moderation owner, two tiers (root + flat
  replies), premoderation, work author toggles comments but never edits/removes others' comments.
  Do not add a Discourse adapter, sync outbox or parallel comment store. Secrets stay out of
  browser bundles and HTML.
- Preserve Vietnamese Unicode, IME composition, paragraph/poem structure and unsaved text.
  Material 3 Expressive should support a calm reader and editor; Vink has its own visual identity.
- AI results are untrusted input. Do not turn manuscript text into tool instructions or send
  private drafts to providers merely because autosave ran. Follow MOD-01–MOD-06 for moderation.

## Work and verification

Default integration branch: `develop`. For a new feature branch, use `codex/<outcome>` unless
the user specifies another workflow; an explicit request to commit directly to `develop` wins.
Check status first and preserve existing edits. Stage explicit paths belonging to the task.
Use authorization already granted in the conversation; a skill adds no requirement to ask again.
Do not infer permission to push, post messages or publish from a local editing task.

Discover manifests, scripts and CI before selecting checks. `cargo xtask verify
quick|discussion|expressions|kani|postgres|web` are implemented lanes, used by CI as well. `discussion` checks
the structural D2T-01, lifecycle D2T-02 (with receipts) and reaction D2T-INV-08 Quint models and replays their inputs through the real
pure decisions (the lane fails if a named replay test is not selected); locking, transactions and
I/O stay in `postgres`; `expressions` verifies its isolated playback model and Rust replay; `kani`
runs the owner-local manifests and requires every declared harness to verify. Every crate inherits
`unsafe_code = "forbid"`, and a `compile_fail` doctest needs a compiling lawful twin because stable
rustdoc ignores its error code. `specs/behavior` and a generic behavior lane do not exist. Do not
add a runner merely to validate documentation or skills.

For documentation/skill changes, check the diff, relative links, YAML metadata and referenced
files. For behavior changes, use the cheapest meaningful regression and then the actual
boundary needed by the claim: real PostgreSQL for transactions and a browser for hydration/editor interactions. Quint evidence is separate from code/I/O evidence.
Use [the verification guide](.agents/skills/vink-engineering/references/verification.md) for scope
and result reporting; never present an unavailable runtime check as passed.

Keep product documents and user-facing copy in Vietnamese unless requested otherwise. Agent
instructions may use English; preserve technical identifiers. Report the outcome, relevant
validation and remaining gaps in the user's language, with detail proportional to the change.
