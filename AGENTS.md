# Agent instructions — Vink

Vink (Việt Ink) is a Vietnamese literary community. Start with [README.md](README.md),
the [documentation map](docs/README.md), then the requirements and sources nearest the task.
The user's current instructions take precedence over repository conventions.

## Repository reality and decisions

This repository currently contains product/architecture proposals and agent skills. No app,
Cargo workspace, Discourse plugin, executable behavior model or build/test runner exists yet.
Inspect actual files before claiming a capability or running a documented candidate command.
Update this paragraph when implementation establishes those capabilities.

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

Discover manifests, scripts and CI before selecting checks. `cargo xtask` and `specs/behavior`
in the foundation are proposals, not commands or registries to assume already exist. Do not add
an application or a runner merely to validate documentation or skills.

For documentation/skill changes, check the diff, relative links, YAML metadata and referenced
files. For behavior changes, use the cheapest meaningful regression and then the actual
boundary needed by the claim: real PostgreSQL for transactions, pinned Discourse for its gates,
and a browser for hydration/editor interactions. Quint evidence is separate from code/I/O evidence.
Use [the verification guide](.agents/skills/vink-engineering/references/verification.md) for scope
and result reporting; never present an unavailable runtime check as passed.

Keep product documents and user-facing copy in Vietnamese unless requested otherwise. Agent
instructions may use English; preserve technical identifiers. Report the outcome, relevant
validation and remaining gaps in the user's language, with detail proportional to the change.
