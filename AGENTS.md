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
appearance preferences and a truthful `/toi` utility, a finite v3 public reading stream with
loaded-list title/author search and URL-based reader return context, a quiet reader, a private
draft dashboard with loaded-list search/resume and uncertain-create reconciliation, a lifecycle-only worker,
`cargo xtask` lanes and CI, and the UI System foundation (`packages/ui-system`: one token source in
`shared/tokens.json` generating CSS/Rust/Kotlin with a drift check, the Leptos crate `vink-ui`, a
Compose Multiplatform `:ui` library and two previews; native Android/iOS targets compile only
when enabled and no native app exists) with UIS-02 primitives on both renderers (text/layout,
actions, fields, icons from a licensed registry, avatar/byline with shared identity vectors) and
UIS-03 state/feedback/loading components (content region with lawful content × phase combinations,
state panel, notices, skeleton/arc/edge progress gated by host-injected activity, snackbar,
dialog, shared state vectors), and UIS-04 shell/navigation primitives
(pure `NavLayout`/`OverlayLayer` policy with shared vectors, adaptive scaffold, rail with a permanent
toggle, bottom bar, top bar; a diagnostic CMP navigation adapter) that the Web shell now renders.
UIS-05 adopts shared controls, identity, notices and dialog chrome on Explore/Reader/Desk/Editor/Me; `packages/vink-patterns` owns DTO-free Leptos/CMP compositions
consumed by the app and both galleries. Feedback and native screen recipes remain explicitly
labelled samples with no service, auth or save authority.
[docs/development](docs/development/README.md) owns its layout, commands, evidence and limits.
No auth authority, login/session, publish/moderation workflow, Discourse plugin, AI provider, job
or executable behavior model exists; production authentication is NOT_IMPLEMENTED. Inspect actual
files before claiming a capability; update this paragraph when implementation changes those facts.

- [Vision](prd/vision.md) and [requirements](prd/requirements.md) own product intent and IDs.
- [Tech stack](docs/architecture/tech-stack.md) recommends Hybrid B: Vink owns literary content
  in PostgreSQL; Discourse owns discussion. Leptos SSR/Axum and a background worker are proposed.
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
- Keep domain decisions independent of Axum, SQLx, Leptos, Discourse and AI provider SDKs.
  Application code orchestrates; adapters perform I/O; UI renders facts and submits intents.
- Keep private working drafts, immutable submitted revisions, moderation decisions, publication
  and curation distinct. Recheck current authority and version at durable commit boundaries.
- In Hybrid work, integrate with Discourse through owned API/event contracts, not cross-database
  writes. Discourse credentials and server secrets must stay out of browser bundles and HTML.
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
quick|postgres|web` are the implemented lanes, used by CI as well; `specs/behavior` and a
behavior lane do not exist. Do not add a runner merely to validate documentation or skills.

For documentation/skill changes, check the diff, relative links, YAML metadata and referenced
files. For behavior changes, use the cheapest meaningful regression and then the actual
boundary needed by the claim: real PostgreSQL for transactions, pinned Discourse for its gates,
and a browser for hydration/editor interactions. Quint evidence is separate from code/I/O evidence.
Use [the verification guide](.agents/skills/vink-engineering/references/verification.md) for scope
and result reporting; never present an unavailable runtime check as passed.

Keep product documents and user-facing copy in Vietnamese unless requested otherwise. Agent
instructions may use English; preserve technical identifiers. Report the outcome, relevant
validation and remaining gaps in the user's language, with detail proportional to the change.
