# Agent instructions — Vink

Vink (Việt Ink) is a Vietnamese literary community. Start with [README.md](README.md),
the [documentation map](docs/README.md), then the requirements and sources nearest the task.
The user's current instructions take precedence over repository conventions.

## Repository reality and decisions

For DecisionDelta entries (10/11 USER, scoped CHOICE and O IDs; 11/11 ADM and O-17..19), D-IDs,
issue-scoped choices (`LVE-PR-01-CHOICE-02`, `TAG-PR-01-CHOICE-01`, `DISC-04-CHOICE-02`), development
choices (`TAG-PR-02..05`, `D-05-C1..C5`, `READ-01/02`, `DISC-WEB`) and
`VDR-*` records, resolve the relevant decision through [the TOML registry](decisions/README.md)
at the task checkout. Read the PRD and domain owner for detailed rules and acceptance. The
original DecisionDelta tables, the D-log and the development docs at their import commits preserve historical context. Use `vink-specify` to write
proposals; a proposal stays `PENDING` until a signoff quotes the owner's own words about that
exact revision (`VDR-DECISION-SIGNOFF@r1`). The
[rollout inventory](docs/development/decision-registry-rollout.md) identifies other owners and
the remaining migration work.

The repository holds product/architecture proposals, agent skills and a modular Rust workspace
([layout and commands](docs/development/README.md)). Inspect actual files and the owning
registries in `docs/development/` before claiming a capability; keep slice-by-slice detail in
those registries rather than duplicating long status narratives across skills.

- **Core app, database and web shell:** `services/vink-app` (domain/application/PostgreSQL/Axum),
  `apps/vink-web` (Leptos SSR + hydration, CodeMirror 6 editor in `apps/vink-web/editor`),
  `packages/store-postgres` (shared pool/migrator) and `services/postgres` (App `vink` and
  Identity `vink_identity` databases with split migrator/runtime roles). Public reads support
  strong body-hash `ETag`/`304` (`READ-01`) and a browser-tab memory-only cache-first pilot on Web
  (`READ-02`; CMP/native not implemented). `vink-worker` runs the discussion outbox and hourly
  retention.
- **UI System (`packages/ui-system`, `packages/vink-patterns`):** one token source in
  `shared/tokens.json` generating CSS/Rust/Kotlin (`cargo xtask ui-tokens`), Leptos `vink-ui`,
  Compose Multiplatform `:ui`, shared primitives/state/navigation vectors (UIS-02..05) and DTO-free
  patterns consumed by Web and diagnostic previews. Opt-in Android/iOS/Desktop `:diagnostics` hosts
  and Lossy WebP fixtures exist for qualification only; **no native product app exists**
  ([qualification evidence](docs/development/ui-system-qualification-evidence.md)).
- **Expressions (`packages/expressions`):** 16-asset Google Noto manifest, pure admission/generation
  policy on `/thu-nghiem/bieu-cam`, lazy dotLottie Software renderer on Web, Quint (`verify
  expressions`) and Kani (`verify kani`) coverage; CMP is contract/Unicode fallback only
  ([expression scope](docs/development/expressions.md)).
- **Literary endorsement (`domain::endorsement`, LVE #73):** pure revision/comment endorsement core
  with accepted-order fencing, exact receipts, checked counts, Quint (`verify discussion`) and Kani
  harnesses ([policy](docs/architecture/literary-endorsement.md),
  [core registry](docs/development/endorsement-core.md)); PostgreSQL/HTTP API v1 and Web reader
  adapter (“Đánh giá cao” / “Hữu ích”, Pen Flight R2 A · Mềm, no optimistic state) are implemented
  ([runtime](docs/development/literary-endorsement-runtime.md)). No native consumer, voter list or
  live ranking exists.
- **Work taxonomy and discovery (`domain::taxonomy`, `domain::discovery`, #95–#100):** curated
  Vietnamese form/theme catalog, immutable slug/search-key rules, versioned draft tags, metadata
  snapshots bound to publications, operator CLI `vink-app tag …` with audit, public discovery
  `GET /api/v1/works` (`q`, repeatable `tag`, `cursor`) and Web discovery/editor consumers
  ([decision](docs/architecture/work-taxonomy.md), [core](docs/development/taxonomy-core.md),
  [storage](docs/development/taxonomy-postgres.md), [API](docs/development/taxonomy-api.md),
  [discovery API](docs/development/discovery-api.md), [discovery UI](docs/development/discovery-ui.md),
  [editor](docs/development/taxonomy-editor.md), [LayoutSpec](docs/design/tag-selector.md)).
- **Product decisions of 10/11/2026 (`PRODUCT-DECISIONS-20261110`,
  [DecisionDelta](docs/architecture/product-decisions-20261110.md),
  [registry](docs/development/product-decisions-registry.md)):** short posts with personal
  collections (chapters, `Tạp Chí` and age policy are `DEFERRED`), two audiences (“Chỉ mình tôi” /
  “Công khai”), AI evaluator as default moderation for works and comments with admin override and
  appeals to admin, author deletion of a public work leaving a tombstone that keeps valid comments,
  comment editing with an “Đã sửa” mark, hashtags, Recommended feed, Top 100, and username +
  password as primary sign-in (PRD §14: `AUD`, `PUB-04/05`, `MOD-07..10`, `COM-09/10`, `COL`,
  `HTG`, `RANK`, `TOP`, `ACC-03`). Runtime implementation is partial:
  - Pure publication/tombstone core + application planning (`#150`, `#157`,
    [publication-core](docs/development/publication-core.md)) and pure evaluator/admin moderation
    core + `ScriptedEvaluator` (`#151`, [moderation-core](docs/development/moderation-core.md)).
  - Admin moderation governance (`ADMIN-MODERATION-20261111`, DecisionDelta §7: MOD-09..15,
    PUB-06/07, COM-11): pure core `domain::moderation::{authority, case, appeal, automation,
    investigation}`; `verify discussion` replays 64/64 M1, 53/53 M3, 105/105 M4 governance and 24
    `evaluator-vectors.json` steps. Since `#159`
    ([moderation-runtime](docs/development/moderation-runtime.md)) work revisions have
    persistence-api + worker: admin seats (`vink-app admin`), work grants and groups, review cases
    and pause/enable under `/api/v1/moderation/*`, decision history, and a `vink-worker`
    evaluation consumer (off by default; only a local/test scripted evaluator, never production;
    without a verdict every job goes to people). `O-17@r2` (`PRODUCT_CONFIRMED`) adds the
    review-withdrawn page at the old URL (`GET /api/v1/works/{id}/reader`, Web reader). The follow-up
    also serves the author-hidden page, persists work appeals and adds Web case/appeal/automation
    screens plus author review status. Investigations, comment cases and grant/group management UI
    remain unimplemented.
  - Author publication persistence + HTTP API (`#158`,
    [publication-runtime](docs/development/publication-runtime.md)) implements snapshot capture,
    `request-public` (queues evaluation or human review according to automation mode), `set-private` and owner state read,
    with pre-#158 pointers backfilled as `legacy_bootstrap`. Bàn viết consumes these routes in its
    “Công khai” region (`PUB-AUTHOR-WEB-01`,
    [owner](docs/development/publication-author-web.md)): choose audience, save → snapshot →
    request-public of exactly the saved version, withdraw, reconcile lost answers; no CMP consumer.
  - Remaining target models (`collections`, `comment_edit`, `discovery` in
    `services/vink-app/behavior/target`) are **model evidence only**; no UI consumer, delete/remove
    route, real AI provider, collection, hashtag, ranking or comment edit exists in
    runtime. Items marked `DESIGN_PROPOSED` or `OPEN` (`O-01..O-19`, except the registry's current
    `O-17@r2`) are not owner product decisions, even when a safe baseline is coded in pure core.
- **Authentication and principal standing (D-05 Provisional, not Accepted;
  [ADR](docs/architecture/auth-authority.md), [runbook](docs/development/auth-operations.md),
  [Auth Web](docs/development/auth-web.md)):** `services/vink-identity` and `services/vink-app`
  implement native email-code sign-in (with `mail_deliveries` outbox and Web `/toi` consumer) and,
  since `#152`, native **backend** username/password credentials (Argon2id, O-12 policy in
  `services/vink-identity/src/password.rs`, `account_passwords`, throttling and App password
  registration/sign-in with the `__Host-vink_session` cookie). Native accounts with verified
  email can add a password on the same subject using a fresh code and read back enrollment
  ([App auth contract](services/vink-app/contracts/http/v1/README.md#sign-in-d-05)). Web/CMP UI for
  username/password, account recovery (`O-11`) and production sign-in unlock (`VINK_AUTH_SIGN_IN`
  defaults to `disabled` in production) remain unimplemented. Every protected App write takes the
  shared standing fence (`principal::require_active`) at commit.
- **Discussion ([ownership ADR](docs/architecture/discussion-ownership.md),
  [core](docs/development/discussion-core.md), [admission](docs/architecture/discussion-admission.md),
  [post-review](docs/development/discussion-post-review.md),
  [outbox](docs/architecture/discussion-outbox.md), [reads](docs/architecture/discussion-reads.md),
  [Web reads](docs/development/discussion-web-reads.md)):** Vink owns two-tier discussion in
  PostgreSQL (root + flat replies). `premoderation.v1` is the only effective admission policy today
  (with DB-held moderator grants via `vink-app moderator …`, durable `comment.submitted` outbox
  deliveries consumed by `vink-worker`, `admission_shadow.v1` audit and operator post-review
  tickets); the 10/11/2026 AI evaluator admission and comment-edit gates are not yet wired to
  runtime. Reader Web consumes the actor-free public projection (`GET …/discussion/public`) and
  authenticated own overlay (`GET …/discussion/me`). Reactions exist only as pure decisions.

Keep these **four status dimensions** separate in docs, reviews and handoffs:

1. **Product decision:** `PRODUCT_CONFIRMED`, `DEFERRED`, or `OPEN` ([PRD](prd/vision.md),
   [requirements](prd/requirements.md), [DecisionDelta](docs/architecture/product-decisions-20261110.md)).
2. **Architecture decision:** `DESIGN_PROPOSED`, `Provisional`, or `Accepted`
   ([decision log](docs/architecture/decisions.md), ADRs). Passing a check or merging code does not
   make a proposal `Accepted`.
3. **Implementation level:** `NOT_IMPLEMENTED`, `core`, `persistence-api`, `worker-provider`,
   `web`, or `cmp` (recorded in `docs/development/*` registries). A pure core baseline for an `OPEN`
   item (such as `O-04` or `O-18`) does not settle the product rule.
4. **Verification evidence:** exact command/lane, pinned SHA and boundary (`model`, `unit/kani`,
   `postgres`, `web`, `cmp`).

[Vision](prd/vision.md) and [requirements](prd/requirements.md) own the product spec and requirement IDs.
[Tech stack](docs/architecture/tech-stack.md): Vink owns literary content and, since 03/11/2026,
discussion in PostgreSQL; Discourse is a design reference only, never a runtime dependency, comment
store, auth source or plugin target. The [decision log](docs/architecture/decisions.md) owns
D-01, INV-01–INV-08 and S-01–S-05 and points D-02–D-19 to their registry records (imported with the
source status); importing documents, passing a check or an owner-confirmed
product direction does not accept a proposal or implement behavior. A scoped prototype may explore a
decision without settling it; record its assumptions and the evidence still needed.

When a request introduces or hits an unclear product/domain rule, use
[`vink-shape`](.agents/skills/vink-shape/SKILL.md) to classify and elicit only the needed owner
decisions; when recording decisions or reconciling drift between code and docs, use
[`vink-specify`](.agents/skills/vink-specify/SKILL.md). For UI and interaction UX tasks
(gestures, motion, focus, IME, CMP libraries), follow the conditional routing in the
[skill index](.agents/skills/README.md) and
[Interaction UX](.agents/skills/vink-ui-design/references/interaction-ux.md); review and report
UI changes with the rubric, reference classes, bounded fix rounds and five-dimension result table
in [UX quality](.agents/skills/vink-ui-design/references/ux-quality.md).
[Archived input](docs/archive/initial/README.md) is historical context only.

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
  replies), premoderation (the implemented policy; the confirmed target is a decision by the AI
  evaluator before anything is public, and edits of one's own comment through the same gate, both
  unimplemented), work author toggles comments but never edits/removes others' comments.
  Do not add a Discourse adapter, sync outbox or parallel comment store. Secrets stay out of
  browser bundles and HTML.
- Preserve Vietnamese Unicode, IME composition, paragraph/poem structure and unsaved text.
  Material 3 Expressive should support a calm reader and editor; Vink has its own visual identity.
- AI results are untrusted input. Do not turn manuscript text into tool instructions or send
  private drafts to providers merely because autosave ran. Follow MOD-01–MOD-15 for moderation:
  an evaluator result carries no authority, a failure or timeout never approves, an admin
  decision on a revision beats a late evaluator result, and no real provider, admin channel or
  auto-approval is enabled.

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
runs the owner-local manifests and requires every declared harness to verify. The `discussion` lane
also runs the target specifications of `behavior/target` (typecheck, simulation, scenarios, vector
drift, negative controls) and canonical M1/M3/M4 governance in `behavior/publication`. The remaining target actions have no Rust replay. Every crate inherits
`unsafe_code = "forbid"`, and a `compile_fail` doctest needs a compiling lawful twin because stable
rustdoc ignores its error code. `specs/behavior` and a generic behavior lane do not exist. Do not
add a runner merely to validate documentation or skills. Keep `AGENTS.md` under 24,000 bytes so all
agent runtimes (Codex, Claude Code, Antigravity) load it without truncation.

For documentation/skill changes, check the diff, relative links, YAML metadata and referenced
files. For behavior changes, use the cheapest meaningful regression and then the actual
boundary needed by the claim: real PostgreSQL for transactions and a browser for hydration/editor interactions. Quint evidence is separate from code/I/O evidence.
Use [the verification guide](.agents/skills/vink-engineering/references/verification.md) for scope
and result reporting; never present an unavailable runtime check as passed.

Keep product documents and user-facing copy in Vietnamese unless requested otherwise. Agent
instructions may use English; preserve technical identifiers. Report the outcome, relevant
validation and remaining gaps in the user's language, with detail proportional to the change.
