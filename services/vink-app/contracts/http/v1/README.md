# Vink HTTP API v1 (bootstrap)

**Owner:** `vink-app` (`services/vink-app/src/http/contract.rs`). **Status:** implemented for the
skeleton; not a stable public API. Consumers keep their own types: the web app's live in
`apps/vink-web/src/contract.rs`. Fixtures in [fixtures/](fixtures/) are checked on both sides:
the provider serializes its responses and compares them field-for-field; the consumer decodes
every fixture and must keep every field.

Only the endpoints below exist. Publish/withdraw and account/profile endpoints do not; the
D-05/S-03 spike adds [Sign-in](#sign-in-d-05s-03-spike) only. Discussion
(comments) is Vink-owned (owner decision 03/11/2026; no Discourse) and has the endpoints in
[Discussion](#discussion-d2t-02b) and [Comment moderation](#comment-moderation-d2t-02c). Literary
endorsement has the endpoints in [Literary endorsement](#literary-endorsement-lve-73); no web or
native consumer uses them yet. Work tags have the endpoints in [Work tags](#work-tags-tag-pr-03):
public catalog reads and the author's private draft tags; no route changes the catalog. The web
desk (TAG-PR-05) consumes the catalog list, suggestions and draft tags; slug resolution has no
consumer yet and no native consumer exists.

## Conventions

- JSON, UTF-8. Identifiers are UUID strings; timestamps are RFC 3339 in UTC.
- `document` uses schema version 1: `{"schema_version": 1, "blocks": [{"lines": ["…"]}]}`.
  A block is a paragraph or stanza; each line is non-blank, without line breaks or control
  characters (tab allowed). The server stores lines NFC-normalized. At most 200 000
  characters; titles at most 200 characters, single line, trimmed.
- Unknown request fields are rejected.
- Unsafe methods (`POST`, `PUT`, `DELETE`) require an `Origin` header equal to the server's
  `VINK_PUBLIC_ORIGIN`; otherwise `403 forbidden` before any use case runs.
- Draft endpoints act for the principal resolved **by the server**. No authority is
  configured in production yet (D-05), so they answer `401` there. Nothing in the request
  body or a header such as `x-user-id` selects the actor. Draft responses carry
  `Cache-Control: no-store`.
- **Session carrier (D-05/S-03 spike):** when the App is configured with the Identity process
  (`VINK_IDENTITY_SOCKET`), a request presents a session credential either as the
  `__Host-vink_session` cookie (Web; set only by [Sign-in](#sign-in-d-05s-03-spike)) or as
  `Authorization: Bearer <credential>` (non-browser clients; an `Authorization` header, even a
  malformed one, takes precedence; two session cookies are no credential). The App asks Identity
  (private contract `vink.identity.v1`) to verify it on every request, then reads its own
  `principal_bindings` and `principal_suspensions`. Without an authority, a rejected, expired or
  revoked credential, an authenticated subject without a principal, or a suspended principal, the
  answer is `401`. Only when a credential is presented and Identity or the App database cannot
  decide (down, timeout, malformed answer, untrusted server) is it `503 dependency_unavailable`.
  No credential is always `401`, never `503`.

## Endpoints

| Method and path | Success | Errors | Fixture |
| --- | --- | --- | --- |
| `GET /api/v1/works?q=&tag=&tag=&limit=&cursor=` | `200` one page of current publications, newest first (default and home page: the first 12); `304` on a matching `If-None-Match` | `422` (`q`, `tag`, `limit`, `cursor`, `query`), `503` | [default](fixtures/works.list.json), [search](fixtures/works.search.json), [unmatched filters](fixtures/works.search.unmatched.json) |
| `GET /api/v1/works/{work_id}/publication` | `200` the public snapshot (never the draft) with its public `tags`; `304` on a matching `If-None-Match` | `404`, `503` | [publication.get.json](fixtures/publication.get.json) |
| `GET /api/v1/drafts` | `200` the actor's drafts | `401`, `503` | [drafts.list.json](fixtures/drafts.list.json) |
| `POST /api/v1/drafts` | `201` a new empty draft at version 1 | `401`, `403`, `503` | [draft.get.json](fixtures/draft.get.json) (same shape) |
| `GET /api/v1/drafts/{work_id}` | `200` the actor's draft | `401`, `404`, `503` | [draft.get.json](fixtures/draft.get.json) |
| `PUT /api/v1/drafts/{work_id}` | `200` new version | `401`, `403`, `404`, `409`, `422`, `503` | [request](fixtures/draft.save.request.json), [response](fixtures/draft.save.response.json) |

### Public discovery (TAG-PR-04)

`GET /api/v1/works` searches the whole public library; there is no other discovery route. No
actor is resolved (an Identity outage never hides it) and only publications are read: a working
draft, private draft tags, an unpublished or a withdrawn work never matches, appears or changes a
page. Choices, plans and measurements: [discovery](../../../../../docs/development/discovery-api.md).

- **Parameters** (all optional; a blank value is absent, as an empty native form field submits it):
  - `q`: text, at most 100 characters after NFC. It matches a **word prefix** of the public title
    or of the author's current display name after folding (NFC/NFD, `đ`/`Đ`, marks and case:
    `hoi uc` finds “Hồi ức tuổi thơ”, `ma` finds “Má” and “Mẫu”, `oi` finds neither). It is a
    literal: `%` and `_` are separators, never wildcards. A NUL character or text without a letter
    or digit gives `422` (`field: q`).
  - `tag`: a tag slug, repeatable (`tag=ký-ức&tag=thơ`), at most 4 distinct after NFC/lowercase
    (8 raw values). **Every** selected tag must be on the work's public metadata snapshot (AND).
    An alias or former slug filters its tag; a slug that names no tag, or a retired tag, matches
    no work and says so in `filters` (not an error). A malformed slug is `422` (`field: tag`).
  - `limit`: `1..=24`, default 12. `cursor`: the previous page's `next_cursor`, unchanged.
  - Anything else, or `q`/`limit`/`cursor` twice, is `422` with `field: query`. Validation order:
    `query` (while reading the URL), then `q`, `tag`, `limit`, `cursor`, before any database read.
- **Answer.** `works`: cards (`work_id`, `title`, `author_display_name`, `published_at`, `tags`).
  `tags` on a card or a publication are the **active** tags of the bound snapshot, form first, with
  current labels (`tag_id`, `facet`, `label`, `slug`); a retired tag stays in the snapshot but is
  not shown; a publication without a snapshot shows `[]`. `filters.q` echoes the query exactly as
  received (or `null`); `filters.tags` has one entry per distinct requested slug, in request order:
  `requested` (normalized), `resolution` (`canonical`, `redirect`, `retired`, `not_found`) and the
  catalog `tag` (with `description` and, when retired, the `successor` hint; `null` for
  `not_found`). A client moves a `redirect` URL to `tag.slug`. `next_cursor` is present only when
  another matching work was read after this page; `null` is the end. There is **no total count**.
- **Order and pages.** Newest `published_at` first, then `work_id` ascending. A page is one
  database snapshot; the list across pages is not: a work published after page 1 is not inserted
  into later pages, a withdrawn one disappears, a republished one moves to the top (it may be
  missed by a client already past it). Nothing is repeated or skipped for works that did not
  change. A cursor carries its position and a digest of the folded query used for matching and the
  canonical tag identities. Another spelling with the same folded key or an alias of the same tag
  continues the search. A different scope or an older cursor version answers `422`
  (`field: cursor`). Restart from the first page in that case. A forged cursor still moves only
  within the public results.
- **Caching.** As READ-01: strong `ETag` over the exact body (cards, labels, filters, cursor),
  `Cache-Control: private, no-cache`, `304` only after the search succeeded. Each distinct URL is
  its own representation. A label rename, a retirement, a withdrawal or a republish changes the
  body; a private draft-tag edit cannot. Errors are `no-store`; an outage is `503`, never an empty
  page, a zero or a `304`.
- **Not provided:** total counts, sort choices, full-text search of manuscripts, ranking,
  personalization, tag search by `q` (use `GET /api/v1/tags/suggestions`), snapshot-consistent
  paging across pages.

`PUT` writes only if the stored version equals `expected_version` and the actor owns the work;
the check and the write are one statement. Otherwise nothing is written. A draft that does not
exist and a draft owned by someone else both answer `404`. A malformed `work_id` is `404`.

`POST` is not idempotent: a retried request after a lost response may create a second empty
draft. Clients disable the button while a request is pending.

### Conditional public reads (READ-01)

Only the two public reads above are conditional; `HEAD` answers like `GET` without content.
Rationale, evidence and rollback: [public conditional reads](../../../../../docs/development/public-conditional-reads.md).

- **Validator.** `ETag` is a **strong** tag: `"` + the first 128 bits of SHA-256 of the exact
  response body, as 32 lowercase hex digits + `"`. It covers every byte the client gets (title,
  byline, document, revision, order and membership). Responses are not content-coded; the tag is
  opaque and its derivation may change. No request header selects a variant, so there is no
  `Vary`.
- **Order.** The resource is resolved first: `404`, `503` and `500` are returned as usual whatever
  `If-None-Match` says. Only an existing representation is compared, never a guess.
- **`If-None-Match`.** Parsed as RFC 9110 list (`*`, or entity tags with optional `W/`, OWS and
  empty elements, several field lines) with weak comparison. A match, or `*`, is `304` with no
  content and the same `ETag` and `Cache-Control`; otherwise `200` with the body. A malformed field
  (unquoted tag, `*` inside a list, illegal byte), more than 32 tags or more than 4096 bytes is
  ignored: the answer is the full `200`, never a `304`.
- **Cache policy.** `200`/`304`: `Cache-Control: private, no-cache` — a browser may store the bytes
  but must revalidate before each reuse; no shared cache. Errors of these routes: `no-store`.
  Drafts and discussion stay `no-store` and have no validator.
- **Consistency.** Each response is read from one database snapshot and serialized once. The tag
  is computed from those bytes. A `304` still reads the snapshot and serializes the response body
  before comparing validators.
- **Withdrawal.** A request whose read starts after a withdrawal commits gets `404`, not `304`.
  Copies a client already received are not recalled by this endpoint.
- **Consumers.** The web app sends no `If-None-Match` itself and treats a raw `304` as an
  unexpected status; the browser's HTTP cache adds the validator and hands the application the
  stored `200`. A client that sends `If-None-Match` itself must handle `304`.

## Discussion (D2T-02b)

Two tiers: roots, and ONE flat list of replies per root. `reply_to` is context, never a third
level. New comments are **pending** until a moderator approves them through
[Comment moderation](#comment-moderation-d2t-02c). The initial status is decided at commit by
the versioned admission of #120; its only effective policy, `premoderation.v1`, always answers
`pending`, and `status` echoes the stored state rather than a constant. The policy and reason
codes are stored for audit and are not part of any response. Writes act for
the server-resolved principal (`401` without one), require the same `Origin` as other unsafe
methods, and never accept an actor, root, role or display name from the client. Every
discussion response carries `Cache-Control: no-store`: it contains viewer-specific state.

| Method and path | Success | Errors | Fixture |
| --- | --- | --- | --- |
| `GET /api/v1/works/{work_id}/discussion?cursor=&limit=` | `200` root page of a public work | `404`, `422` (`cursor`, `limit`, `query`), `503` | [discussion.get.json](fixtures/discussion.get.json) |
| `GET /api/v1/works/{work_id}/discussion/roots/{root_id}/replies?cursor=&limit=` | `200` replies of one root | `404`, `422`, `503` | [discussion.replies.json](fixtures/discussion.replies.json) |
| `POST /api/v1/works/{work_id}/discussion/comments` | `201` new pending comment; `200` with `replayed: true` for a retry | `401`, `403` (`forbidden`, `principal_suspended`), `404`, `409` (`comments_closed`, `target_unavailable`), `422` (`body`, `command_key`, `target_id`, `idempotency_key_reused`), `503` | [request](fixtures/discussion.comment.submit.request.json), [response](fixtures/discussion.comment.submit.response.json) |
| `DELETE /api/v1/works/{work_id}/discussion/comments/{comment_id}` | `200` the writer removed their own comment (repeatable) | `401`, `403` (`forbidden`, `not_comment_writer`), `404`, `503` | [discussion.comment.remove.response.json](fixtures/discussion.comment.remove.response.json) |
| `PUT /api/v1/works/{work_id}/discussion/settings` | `200` the work's author opened/closed comments | `401`, `403`, `404`, `409` (`version_conflict`), `422`, `503` | [request](fixtures/discussion.settings.request.json), [response](fixtures/discussion.settings.response.json) |

- **Reading.** Anyone may read the discussion of a work with a current publication; otherwise
  `404`. Others see only approved comments. A signed-in writer also sees their own pending
  (`own_pending`) and rejected (`own_rejected`) comments. `can_remove` is true only on the
  viewer's own comments: the work's author gets no power over other people's comments.
  A hidden root that still has public replies is an `anchor` without text or author.
  `rejection_reason` is the moderator's reason and is non-null only on the viewer's own
  `own_rejected` comment; a comment removed by moderation disappears for everyone, its writer
  included (no notification exists yet).
  `reply_to` is `null` on roots; on replies it is `{"kind": "root"}`, a public
  `{"kind": "reply", "comment_id", "author_display_name"}`, or `{"kind": "unavailable"}` when the
  context was hidden or removed (never shown as “deleted by its writer”).
- **Paging.** Roots and replies are ordered `created_at ASC, comment_id ASC`. `limit` is 1–50
  (default 20). Cursors are opaque; pass `next_cursor` back unchanged. Each root previews up to
  three replies; `preview_next_cursor` continues that root's replies endpoint after the preview.
  Paging is not a snapshot: a comment approved later can appear before an issued cursor.
- **Submitting.** `target_id` is `null` for a root, or a root/reply ID of the same work to reply.
  `command_key` (8–128 of `A-Z a-z 0-9 - _`) is the idempotency key, scoped to the actor: after a
  timeout, retry with the **same** key and body; the same key with a different payload is
  `422 idempotency_key_reused`. `body` is NFC-normalized, trimmed, at most 4000 characters and
  200 lines; emoji are kept; control and bidi override characters are refused. A malformed
  `target_id` is `422`, a missing, foreign or non-public target is `409 target_unavailable`.
- **Removing.** Only a comment's writer removes it; the text is deleted. For a comment the actor
  can see but did not write the answer is `403 not_comment_writer`; for one they cannot see,
  `404`. Repeating a removal returns `200` again.
- **Settings.** Only the work's author changes `comments_open`; anyone else gets `404`.
  `comments_open` is the desired state and `expected_generation` the generation the client last
  saw; the generation is compared first, so a stale tab cannot undo a newer choice:
  - `expected_generation` equal to the stored one: **accepted**, `200` with `generation` + 1,
    also when `comments_open` already holds. That advance fences every older request still in
    flight (a request for the current state is a real intent, not a no-op).
  - another generation and `comments_open` already holds: `200` with the stored settings,
    nothing written (a retry after a lost response, or a duplicate).
  - another generation and the other state: `409 version_conflict` with the stored generation in
    `current_version`; nothing written. Show the stored state; never resend the old intent.

  Closing stops new comments and replies; approved comments stay readable. Comments start open
  on a public work.
- **Not provided:** editing (blocked by D-08), reports, appeals, reactions, ranking,
  notifications or rate limits.

## Comment moderation (D2T-02c)

Authority is an **active moderator grant** in Vink's database. Only the operator command
`vink-app moderator grant|revoke|list` creates or revokes one; no endpoint does, and owning a
work never implies one. Each decision rechecks the grant in its own transaction, so a revocation
that commits first wins. A moderator **recuses** from comments they wrote and from comments on
works they own (`403 moderator_recused`). Without a grant the answer is `403 not_moderator`
before any comment lookup, so non-moderators learn nothing about pending comments.

| Method and path | Success | Errors | Fixture |
| --- | --- | --- | --- |
| `GET /api/v1/moderation/comments?cursor=&limit=` | `200` pending comments, oldest first, with `pending_total` | `401`, `403` (`not_moderator`), `422`, `503` | [moderation.queue.json](fixtures/moderation.queue.json) |
| `POST /api/v1/moderation/comments/{comment_id}/decisions` | `200` the comment's new (or unchanged) review state | `401`, `403` (`forbidden`, `not_moderator`, `moderator_recused`), `404`, `409` (`version_conflict`, `comment_removed`), `422` (`expected_version`, `reason`, `body`), `503` | [request](fixtures/moderation.decision.request.json), [response](fixtures/moderation.decision.response.json) |

- `action` is `approve`, `reject` or `remove`. `reason` (1–500 characters, NFC, newlines
  allowed) is **required** for `reject` and `remove` and is what the writer reads (MOD-04).
- `expected_version` is the queue item's `version`. Another decision since then is
  `409 version_conflict` with `current_version`; repeating the current decision is `200` without
  a change. A removed comment accepts no decision (`409 comment_removed`): removal is terminal.
- `can_decide` in the queue is false where this moderator must recuse. `work_title` is the
  current public snapshot's title, or `null` when the work is not public.
- Every decision is recorded append-only with the moderator, the grant that authorized it and
  the reason. **Not provided:** reports, appeals, reopening a removed comment, queue age alerts.

## Literary endorsement (LVE #73)

One positive signal per actor and target: “Đánh giá cao” on one published **revision** of a work,
“Hữu ích” on a root comment or a flat reply. Semantics are owned by the
[decision record](../../../../../docs/architecture/literary-endorsement.md) (`decide_endorsement`),
data flow and operations by [the runtime guide](../../../../../docs/development/literary-endorsement-runtime.md).
Every endorsement response, success or error, carries `Cache-Control: no-store`. Writes act for
the server-resolved principal (`401` without one, so production answers `401` until D-05), need
the same `Origin` as other unsafe methods, and never accept an actor, count, role, root or work
from the body: unknown fields are `422`.

| Method and path | Success | Errors | Fixture |
| --- | --- | --- | --- |
| `GET /api/v1/works/{work_id}/endorsements?comment_ids=` | `200` public summaries of the current revision and of the listed comments | `404`, `422` (`comment_ids`, `query`), `503` | [endorsements.get.json](fixtures/endorsements.get.json) |
| `GET /api/v1/works/{work_id}/endorsements/me?comment_ids=` | `200` the actor's own states | `401`, `404`, `422`, `503` | [endorsements.me.get.json](fixtures/endorsements.me.get.json) |
| `PUT /api/v1/works/{work_id}/endorsement/me` | `200` accepted or replayed | `401`, `403` (`forbidden`, `self_endorsement`, `endorsement_not_permitted`), `404`, `409` (`version_conflict`, `revision_changed`), `422` (incl. `idempotency_key_reused`), `503`, `500` | [request](fixtures/endorsement.work.put.request.json), [response](fixtures/endorsement.work.put.response.json) |
| `PUT /api/v1/works/{work_id}/discussion/comments/{comment_id}/endorsement/me` | `200` accepted or replayed | `401`, `403`, `404`, `409` (`version_conflict`), `422`, `503`, `500` | [request](fixtures/endorsement.comment.put.request.json), [response](fixtures/endorsement.comment.put.response.json) |

- **Command.** `endorsed` is the desired selection; `expected_order` the `order` the client last
  saw for that target (`1` before any action); `command_key` a UUID the client chooses, scoped to
  the actor across **all** targets. A work command also names `revision_id`, the revision the page
  rendered: each revision is its own target, and the server checks it belongs to the work.
- **Outcomes.**
  - `expected_order` equal to the stored order and the target eligible: **accepted** (`outcome:
    "accepted"`), `order` + 1 — also when the selection already held (that advance fences every
    older command). `version` + 1 only when `selected` changes.
  - The same `command_key` and the same body again (a retry after a lost response): `200`
    `outcome: "replayed"`, nothing written. `committed` is what that command committed (history);
    `endorsement` is the **current** state — show that one.
  - The same key with another body: `422 idempotency_key_reused` (`field: command_key`).
  - Another order: `409 version_conflict` with `current_version` (the current order) and
    `endorsement` (the current state). Show it; never resend the old intent with the new order.
  - Selecting a revision that is no longer the public one: `409 revision_changed` with
    `current_revision_id`. Nothing written; that revision's selections stay stored, uncounted.
  - `403 self_endorsement`: the work's author selecting their work, or a writer their comment.
    `403 endorsement_not_permitted`: the actor has no current capability (no such mechanism exists
    yet besides having a principal).
  - `404`: missing, of another work, or not visible (draft, withdrawn, pending, rejected, removed,
    hidden root) — except **removing the actor's own retained record**, which stays possible while
    the target is hidden or withdrawn (no count is returned then). Replies under a hidden root stay
    selectable while they are public. Closing comments does not affect endorsements.
- **Result.** `work_id`, `revision_id` (works) or `comment_id` name the target; `endorsement` is
  `{selected, version, order}`; `summary` is the target's public count read in the same transaction
  (`unavailable` when the target is hidden). A client applies a result only when its `order` is
  greater than the one it shows for that (session, target), so a late answer never moves the UI back.
- **Ordering scope.** The order serializes one actor's commands on one target; the key namespace
  is the actor's. Nothing orders different targets or actors, and two devices of one actor are not
  told which intent was “last”: the first accepted at an order wins, the other gets `409`.
- **Reads.** `comment_ids` is a comma-separated list of comment IDs of that work, at most 100
  (duplicates count; each distinct ID is answered once, in request order). Unknown query parameters
  are `422`. A work that is not public is `404` for both reads. `work` describes the **current**
  publication's revision (`revision_id`). A missing, foreign or hidden comment is `{"status":
  "unavailable", "count": null}` in summaries and `{"state": "unavailable", "version": null,
  "order": null}` in own state: never zero, never a stored token. Own state is `not_selected` with
  version/order `1` before any action, `401` for a guest. `can_endorse`/`blocked_reason`
  (`own_content`, `not_permitted`) are advisory: each write decides again on the facts it locks.
  Summaries never resolve the actor, so an identity outage cannot hide them. Each read is one
  snapshot; summaries and own state are separate snapshots.
- **Not provided:** voter lists or attribution, ranking by endorsements, counts on the works list
  or the publication read (READ-01 bodies and validators are unchanged), reasons or facets, rate
  limits, receipt expiry.

## Work tags (TAG-PR-03)

Semantics are owned by the [decision record](../../../../../docs/architecture/work-taxonomy.md)
(`decide_tag_change`, `rank_matches`, `resolve_slug`); choices, data flow and the operator runbook by
[the API guide](../../../../../docs/development/taxonomy-api.md). A tag describes a work: one
optional **form** (`facet: "form"`, dạng bài) and up to three distinct **themes** (`"theme"`, chủ
đề) chosen from a curated catalog. Labels keep case and Vietnamese marks; `slug` is lowercase with
marks kept (`ký-ức`), never reassigned; accent folding only finds candidates (`ma`, `má`, `mà` are
three tags). Every response of these routes, success or error, carries `Cache-Control: no-store`.

| Method and path | Success | Errors | Fixture |
| --- | --- | --- | --- |
| `GET /api/v1/tags?facet=` | `200` active tags, forms first, then by accent-folded label; at most 200 (`truncated: true` when more exist) | `422` (`facet`, `query`), `503` | [tags.list.json](fixtures/tags.list.json) |
| `GET /api/v1/tags/suggestions?q=&facet=&limit=` | `200` ranked suggestions of active tags for `q` | `422` (`q`, `facet`, `limit`, `query`), `503` | [tags.suggestions.json](fixtures/tags.suggestions.json) |
| `GET /api/v1/tags/slugs/{slug}` | `200` what the slug names: `canonical`, `redirect` or `retired` | `404`, `503` | [redirect](fixtures/tag.resolve.redirect.json), [retired](fixtures/tag.resolve.retired.json) |
| `GET /api/v1/drafts/{work_id}/tags` | `200` the actor's draft tags with `version` | `401`, `404`, `503` | [draft.tags.get.json](fixtures/draft.tags.get.json) |
| `PUT /api/v1/drafts/{work_id}/tags` | `200` new `version` and the saved identities | `401`, `403` (`forbidden`, `principal_suspended`), `404`, `409` (`version_conflict`, `tag_retired`), `422` (`validation_failed`, `unknown_tag`, `wrong_facet`, `too_many_themes`), `503` | [request](fixtures/draft.tags.put.request.json), [response](fixtures/draft.tags.put.response.json) |

- **A tag.** `{tag_id, facet, label, slug, description, status, successor}`. `status` is `active`
  (selectable) or `retired` (kept where it already is, never added); `successor` is
  `{tag_id, label, slug}` of the curator's hint on a retired tag, or `null`. `description` is the
  curator's one-paragraph note or `null`.
- **Catalog reads are public.** They never resolve an actor, so an Identity outage does not affect
  them; a database outage is `503`, never an empty list. Only active tags are listed or suggested;
  aliases, former names, retired tags, audit and usage counts are never listed. Unknown query
  parameters are `422` (`field: query`).
- **Suggestions.** `q` is required: 1–40 characters after NFC with at least one letter or digit.
  `limit` is 1–10 (default 10); `facet` is `form` or `theme`. `query` echoes `q`, so a client drops
  answers to an older input. `match` is `exact` (same slug: same letters and marks, any case or
  NFC/NFD), `folded` (same accent-folded key, e.g. `ma` for `má`) or `prefix` (the folded query
  starts the name or one of its words). `via_alias` is the alias spelling that matched (`Hồi ức`
  for `Ký ức`), or `null`; the suggestion is always the canonical tag, once.
- **Slugs.** The path segment is percent-decoded and normalized (NFD and uppercase spell the same
  slug); a malformed slug is `404`. `canonical`: the tag's own slug. `redirect`: an alias or a
  former name (after a rename) of an active tag; use `tag.slug` from now on. `retired`: any slug of
  a retired tag, with `tag.successor` when the curator named one. Unknown: `404`.
- **Draft tags are private** and act for the server-resolved principal (`401` without one; production
  answers `401` until D-05). Another owner's work and a missing work are both `404`. They have their
  own `version`, separate from the draft text's: `PUT /api/v1/drafts/{work_id}` neither carries nor
  clears tags, and a tag save does not change the text version. Saving tags publishes nothing: the
  public reads above and `GET …/publication` are unchanged.
- **Saving.** The body is the whole desired state and **every field is required**:
  `{"expected_version", "form", "themes"}`. `form` is a form tag ID or `null` (clears it); `themes`
  is a list of theme tag IDs (`[]` clears them), at most 6 references, deduplicated in first-seen
  order, at most 3 distinct. An omitted field, an unknown field (`actor_id`, `owner_id`…) or a
  malformed body is `422 validation_failed` (`field: body`), so a client that does not know a field
  cannot clear it by omission. A malformed ID is `422` with `field: form` or `themes`.
- **Outcomes.**
  - `expected_version` equal to the stored one and every tag admissible: **accepted**, `200` with
    `version` + 1 and the saved `form`/`themes` IDs — also when nothing changed (`changed: false`):
    that advance fences every older request still in flight.
  - Another version: `409 version_conflict` with `current_version` and `tags` (`{form, themes}`, the
    stored IDs); nothing written. If `tags` equal what the client wanted, its desired state holds:
    adopt `current_version` (this is how a lost acknowledgement is reconciled; it does not say
    which request committed). Otherwise show the stored tags; never resend the old intent with the
    new version.
  - `422 unknown_tag` (“chưa có thẻ này”; no proposal exists), `422 wrong_facet` (a theme as `form`
    or a form in `themes`), `409 tag_retired` (`successor_id` when hinted; a retired tag already on
    the draft may stay, never be added) and `422 too_many_themes`. `field` names `form` or `themes`
    and `tag_id` the refused tag. Nothing is written.
  - `403 principal_suspended`: the author was suspended before the save committed.
- **Not provided:** catalog writes over HTTP (the operator command `vink-app tag …` is the only
  entrypoint), tag proposals by authors, validators/`304` for catalog reads, rate limits. Discovery
  by tag and public tags on cards/publications: [Public discovery](#public-discovery-tag-pr-04).

## Sign-in (D-05/S-03 spike)

The browser-facing side of the native email-code authority ([ADR](../../../../../docs/architecture/auth-authority.md)).
Production authentication is NOT_IMPLEMENTED: Identity only writes codes to a local outbox. No web
or native consumer uses these routes yet. Every answer is `Cache-Control: no-store`; unsafe methods
need the same-origin `Origin` like every other route. Bodies never contain the challenge handle or
the session credential: they travel only in `__Host-` cookies (`Path=/; Secure; HttpOnly`).

| Method and path | Success | Errors | Fixture |
| --- | --- | --- | --- |
| `POST /api/v1/auth/sign-in/challenges` `{email}` | `202 {expires_at}` + `Set-Cookie: __Host-vink_sign_in=…; SameSite=Strict` | `403`, `422` (`field: email`), `429 rate_limited`, `501 authority_not_configured`, `503` | [request](fixtures/auth.challenge.request.json), [response](fixtures/auth.challenge.response.json) |
| `POST /api/v1/auth/sign-in/complete` `{code}` + challenge cookie | `200 {state}` (`needs_principal` or `active` + `principal_id`) + `Set-Cookie: __Host-vink_session=…; SameSite=Lax`, challenge cookie cleared | `403`, `422 validation_failed` (`field: code`), `422 code_rejected`, `409 challenge_closed`, `403 account_disabled`, `403 principal_suspended`, `501`, `503` | [request](fixtures/auth.complete.request.json), [needs principal](fixtures/auth.session.needs_principal.json), [active](fixtures/auth.session.active.json) |
| `GET /api/v1/auth/session` | `200 {state, principal_id?}`: `anonymous`, `needs_principal`, `suspended` or `active` | `503` | same shapes |
| `POST /api/v1/auth/principal` `{pen_name}` | `201 {principal_id, created: true}`; `200 … created: false` when already bound (pen name unchanged) | `401`, `403`, `403 principal_suspended`, `422` (`field: pen_name`), `503` | [request](fixtures/auth.principal.request.json), [response](fixtures/auth.principal.response.json) |
| `DELETE /api/v1/auth/session` | `204`, session cookie cleared | `403`, `503` (cookie kept, retry) | — |

- The code may contain spaces (`"042 917"`); it must be six digits. A wrong code keeps the
  challenge cookie (`code_rejected`); a closed challenge, a disabled account or a suspended
  principal clears it.
- Completing a challenge is **authentication only**. A subject without a principal gets a session
  but no authority (`needs_principal`; every private route answers `401`) until `POST
  /auth/principal` creates exactly one principal for it — concurrent or repeated calls return the
  same principal. A suspended principal, or a binding lookup that cannot be decided, revokes the
  fresh session in Identity and sets no cookie.
- Logout succeeds only after Identity revoked the session (or nothing was presented). If Identity
  is down the answer is `503` and the cookie stays, so logout can be retried; meanwhile the session
  cannot be verified and grants nothing.
- Per-IP rate limits, a real mail transport, email change for end users and profile/pen-name
  editing are not implemented.

## Errors

Every error body is `{"error": {"code", "message", "current_version"?, "field"?}}`, plus the
code-specific members named below. Branch on `code`; `message` is developer text.

| Status | `code` | Meaning | Fixture |
| --- | --- | --- | --- |
| 422 | `validation_failed` | Body, title, document or `expected_version` invalid; `field` names it | [error.validation_failed.json](fixtures/error.validation_failed.json) |
| 401 | `unauthenticated` | No authenticated Vink principal | [error.unauthenticated.json](fixtures/error.unauthenticated.json) |
| 403 | `forbidden` | Origin check failed for an unsafe method | [error.forbidden.json](fixtures/error.forbidden.json) |
| 404 | `not_found` | Missing, private to someone else, or not published | [error.not_found.json](fixtures/error.not_found.json) |
| 409 | `version_conflict` | Draft changed since `expected_version`, comments settings set to the other state since `expected_generation`, a comment's moderation state since `expected_version`, an endorsement's order moved past `expected_order`, or draft tags changed since `expected_version`; `current_version` is the stored one (endorsements also carry `endorsement` and `field: expected_order`; draft tags carry `tags` and `field: expected_version`) | [draft](fixtures/error.version_conflict.json), [settings](fixtures/error.discussion_conflict.json), [moderation](fixtures/error.moderation_conflict.json), [endorsement](fixtures/error.endorsement_conflict.json), [tags](fixtures/error.tag_version_conflict.json) |
| 422 | `unknown_tag` | A draft tag that is not in the curated catalog; `field` (`form`/`themes`) and `tag_id` | [error.unknown_tag.json](fixtures/error.unknown_tag.json) |
| 409 | `tag_retired` | A retired tag that is not already on the draft; `field`, `tag_id`, `successor_id` when hinted | [error.tag_retired.json](fixtures/error.tag_retired.json) |
| 422 | `wrong_facet` | A theme tag as `form` or a form tag in `themes`; `field`, `tag_id` | [error.wrong_facet.json](fixtures/error.wrong_facet.json) |
| 422 | `too_many_themes` | More than three distinct themes; `field: themes` | [error.too_many_themes.json](fixtures/error.too_many_themes.json) |
| 409 | `revision_changed` | Selecting a revision that is no longer the public one; `current_revision_id` names the public revision | [error.revision_changed.json](fixtures/error.revision_changed.json) |
| 403 | `self_endorsement` | The work's author or the comment's writer tried to endorse their own content | [error.self_endorsement.json](fixtures/error.self_endorsement.json) |
| 403 | `endorsement_not_permitted` | The actor has no current endorsement capability | [error.endorsement_not_permitted.json](fixtures/error.endorsement_not_permitted.json) |
| 409 | `comments_closed` | The work's author closed comments (new comments and replies) | [error.comments_closed.json](fixtures/error.comments_closed.json) |
| 409 | `target_unavailable` | Reply target missing, of another work or not public | [error.target_unavailable.json](fixtures/error.target_unavailable.json) |
| 422 | `idempotency_key_reused` | Same `command_key`, different payload (comment submission or endorsement command); `field` is `command_key` | [comment](fixtures/error.idempotency_key_reused.json), [endorsement](fixtures/error.endorsement_key_reused.json) |
| 403 | `not_comment_writer` | Removing a visible comment written by someone else | [error.not_comment_writer.json](fixtures/error.not_comment_writer.json) |
| 403 | `not_moderator` | No active moderator grant | [error.not_moderator.json](fixtures/error.not_moderator.json) |
| 403 | `moderator_recused` | The moderator wrote the comment or owns the work | [error.moderator_recused.json](fixtures/error.moderator_recused.json) |
| 409 | `comment_removed` | A decision on a removed comment | [error.comment_removed.json](fixtures/error.comment_removed.json) |
| 501 | `authority_not_configured` | Sign-in routes when no Identity authority resolves requests | [error.authority_not_configured.json](fixtures/error.authority_not_configured.json) |
| 429 | `rate_limited` | Too many sign-in codes for the address | [error.rate_limited.json](fixtures/error.rate_limited.json) |
| 422 | `code_rejected` | Wrong sign-in code; `field` is `code`; retry allowed | [error.code_rejected.json](fixtures/error.code_rejected.json) |
| 409 | `challenge_closed` | Sign-in code expired, used, exhausted or no challenge cookie | [error.challenge_closed.json](fixtures/error.challenge_closed.json) |
| 403 | `account_disabled` | The mailbox was proven but the Identity account is disabled | [error.account_disabled.json](fixtures/error.account_disabled.json) |
| 403 | `principal_suspended` | The Vink principal is suspended (sign-in, provisioning, and a draft-tag save or comment submission that commits after the suspension; a suspended writer's exact retry is refused too) | [error.principal_suspended.json](fixtures/error.principal_suspended.json) |
| 503 | `dependency_unavailable` | PostgreSQL or Identity unreachable; retry later | [error.dependency_unavailable.json](fixtures/error.dependency_unavailable.json) |
| 500 | `internal` | Stored data failed validation, an unexpected database error, or an exhausted endorsement token | — |

## Health (not versioned)

`GET /health/live` answers `200` while the process serves HTTP. `GET /health/ready` answers
`200` only when PostgreSQL is reachable and its migration history equals the embedded
migrations, otherwise `503` with `checks.database` (`ok`/`unavailable`) and `checks.schema`
(`current`/`pending_migrations`/`diverged`/`unknown`; `diverged` means an unfinished, edited or
unknown applied migration). `checks.role` is `restricted`, `privileged` (the database role can
change schema or roles) or `unknown`; in the production profile `privileged` is `503`. It also
reports `integrations.ai_moderation` as `not_implemented`,
`integrations.jobs` as `none` and `integrations.identity` as `not_configured`, `no_authority`,
`available` or `unavailable` — informational: public reading does not depend on Identity, so
Identity never makes the App not ready.
