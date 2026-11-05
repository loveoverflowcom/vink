# Vink HTTP API v1 (bootstrap)

**Owner:** `vink-app` (`services/vink-app/src/http/contract.rs`). **Status:** implemented for the
skeleton; not a stable public API. Consumers keep their own types: the web app's live in
`apps/vink-web/src/contract.rs`. Fixtures in [fixtures/](fixtures/) are checked on both sides:
the provider serializes its responses and compares them field-for-field; the consumer decodes
every fixture and must keep every field.

Only the endpoints below exist. Publish/withdraw and account endpoints do not. Discussion
(comments) is Vink-owned (owner decision 03/11/2026; no Discourse) and has the endpoints in
[Discussion](#discussion-d2t-02b) and [Comment moderation](#comment-moderation-d2t-02c). Literary
endorsement has the endpoints in [Literary endorsement](#literary-endorsement-lve-73); no web or
native consumer uses them yet.

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
- **Provisional credential carrier:** when the App is configured with the Identity process
  (`VINK_IDENTITY_SOCKET`), a request may present `Authorization: Bearer <credential>`; the App
  asks Identity (private contract `vink.identity.v1`) to verify it. Without an authority, a
  rejection or an authenticated subject that has no principal binding (none exists yet), the
  answer is `401`. Only when a credential is presented and Identity cannot decide (down, timeout,
  malformed answer, untrusted server) is it `503 dependency_unavailable`. No credential is always
  `401`, never `503`. D-05 may replace this carrier with a session model.

## Endpoints

| Method and path | Success | Errors | Fixture |
| --- | --- | --- | --- |
| `GET /api/v1/works` | `200` list of current publications (at most 12); `304` on a matching `If-None-Match` | `503` | [works.list.json](fixtures/works.list.json) |
| `GET /api/v1/works/{work_id}/publication` | `200` the public snapshot (never the draft); `304` on a matching `If-None-Match` | `404`, `503` | [publication.get.json](fixtures/publication.get.json) |
| `GET /api/v1/drafts` | `200` the actor's drafts | `401`, `503` | [drafts.list.json](fixtures/drafts.list.json) |
| `POST /api/v1/drafts` | `201` a new empty draft at version 1 | `401`, `403`, `503` | [draft.get.json](fixtures/draft.get.json) (same shape) |
| `GET /api/v1/drafts/{work_id}` | `200` the actor's draft | `401`, `404`, `503` | [draft.get.json](fixtures/draft.get.json) |
| `PUT /api/v1/drafts/{work_id}` | `200` new version | `401`, `403`, `404`, `409`, `422`, `503` | [request](fixtures/draft.save.request.json), [response](fixtures/draft.save.response.json) |

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
- **Consistency.** Each response is one SQL statement (one snapshot) serialized once; the tag is
  computed from those bytes, so it never travels with another body. A `304` still runs that query
  and the serialization: it saves transfer, not database work.
- **Withdrawal.** A request whose read starts after a withdrawal commits gets `404`, not `304`.
  Copies a client already received are not recalled by this endpoint.
- **Consumers.** The web app sends no `If-None-Match` itself and treats a raw `304` as an
  unexpected status; the browser's HTTP cache adds the validator and hands the application the
  stored `200`. A client that sends `If-None-Match` itself must handle `304`.

## Discussion (D2T-02b)

Two tiers: roots, and ONE flat list of replies per root. `reply_to` is context, never a third
level. New comments are **pending** until a moderator approves them through
[Comment moderation](#comment-moderation-d2t-02c). Writes act for
the server-resolved principal (`401` without one), require the same `Origin` as other unsafe
methods, and never accept an actor, root, role or display name from the client. Every
discussion response carries `Cache-Control: no-store`: it contains viewer-specific state.

| Method and path | Success | Errors | Fixture |
| --- | --- | --- | --- |
| `GET /api/v1/works/{work_id}/discussion?cursor=&limit=` | `200` root page of a public work | `404`, `422` (`cursor`, `limit`, `query`), `503` | [discussion.get.json](fixtures/discussion.get.json) |
| `GET /api/v1/works/{work_id}/discussion/roots/{root_id}/replies?cursor=&limit=` | `200` replies of one root | `404`, `422`, `503` | [discussion.replies.json](fixtures/discussion.replies.json) |
| `POST /api/v1/works/{work_id}/discussion/comments` | `201` new pending comment; `200` with `replayed: true` for a retry | `401`, `403`, `404`, `409` (`comments_closed`, `target_unavailable`), `422` (`body`, `command_key`, `target_id`, `idempotency_key_reused`), `503` | [request](fixtures/discussion.comment.submit.request.json), [response](fixtures/discussion.comment.submit.response.json) |
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

One positive signal per (actor, target): “Đánh giá cao” on a public work, “Hữu ích” on a root
comment or a flat reply. Policy `lve-core-v1`; meaning and choices are owned by the
[decision record](../../../../../docs/architecture/literary-endorsement.md), data flow and
operations by [the runtime guide](../../../../../docs/development/literary-endorsement-runtime.md).
Every endorsement response, success or error, carries `Cache-Control: no-store`. Writes act for
the server-resolved principal (`401` without one, so production answers `401` until D-05), need
the same `Origin` as other unsafe methods, and never accept an actor, count, role, root or work
from the body: unknown fields are `422`.

| Method and path | Success | Errors | Fixture |
| --- | --- | --- | --- |
| `GET /api/v1/works/{work_id}/endorsements?comment_ids=` | `200` public summaries of a public work and of the listed comments | `404`, `422` (`comment_ids`, `query`), `503` | [endorsements.get.json](fixtures/endorsements.get.json) |
| `GET /api/v1/works/{work_id}/endorsements/me?comment_ids=` | `200` the actor's own states | `401`, `404`, `422`, `503` | [endorsements.me.get.json](fixtures/endorsements.me.get.json) |
| `PUT /api/v1/works/{work_id}/endorsement/me` | `200` the committed result | `401`, `403` (`forbidden`, `self_endorsement`), `404`, `409` (`version_conflict`, `revision_changed`), `422`, `503`, `500` | [request](fixtures/endorsement.work.put.request.json), [response](fixtures/endorsement.work.put.response.json) |
| `PUT /api/v1/works/{work_id}/discussion/comments/{comment_id}/endorsement/me` | `200` the committed result | `401`, `403`, `404`, `409` (`version_conflict`, `comments_closed`), `422`, `503`, `500` | [request](fixtures/endorsement.comment.put.request.json), [response](fixtures/endorsement.comment.put.response.json) |

- **Desired state, not a toggle.** `endorsed` is the state the actor wants and
  `expected_version` the `version` the client last saw for that target (`1` before any
  action). A work endorsement also names `observed_revision_id`: the `revision_id` of the
  publication the page rendered. It is required when `endorsed` is `true` and must be `null`
  or absent when clearing. The server stores the revision it verified, never one from the body.
- **Outcomes.** The version is compared first:
  - `expected_version` equal to the stored one: **accepted**, `200` with `accepted: true` and
    `version` + 1, also when the state already held (a reaffirmation). That advance fences every
    older request of this actor on this target still in flight.
  - another version and the state already holds: `200` with `accepted: false` and the stored
    state; nothing written, nothing fenced (a retry after a lost response, or a duplicate).
  - another version and the other state: `409 version_conflict` with `current_version` and
    `endorsement` (the stored state). Show it; never resend the old intent with the new version.
  - `409 revision_changed` with `current_revision_id`: the work was republished since the page
    was read; nothing written. Show the current text before offering to endorse it.
  - `409 comments_closed`: the work's author closed comments; a NEW comment endorsement is
    refused, clearing and reaffirming an existing one still work. The work's own endorsement is
    not governed by the comments switch.
  - `403 self_endorsement`: the work's author or the comment's writer.
  - `404`: missing, of another work, not public (draft, withdrawn, pending, rejected, removed,
    or a hidden root). Replies under a hidden root stay endorsable while they are public.
- **Result.** `endorsement` is the actor's state as committed: `state` (`not_endorsed`,
  `endorsed`, `endorsed_earlier_revision`), `version`, `revision_id` (works: the anchor while
  selected), `can_endorse` and `blocked_reason` (`own_content`, `comments_closed` or `null`).
  `summary` is the target's public count read in the same transaction after the write.
  `work_id`/`comment_id` name the target. A client applies a result only when its `version` is
  greater than the one it shows for that (session, target), so a late answer never moves the UI
  back. There is no command key or receipt: “was this exact request accepted?” is answered by
  resending it unchanged (`accepted: false` if it committed) or by reading own state back.
- **Ordering scope.** The version orders one actor's requests on one target. Nothing orders
  different targets or actors, and two devices of one actor are not told which intent was “last”:
  the first accepted at a version wins, the other gets `409`.
- **Reads.** `comment_ids` is a comma-separated list of comment IDs of that work, at most 100
  (duplicates count; the answer lists each distinct ID once, in request order). Unknown query
  parameters are `422`. A work that is not public is `404` for both reads. Within a public work,
  a missing, foreign or hidden comment is `{"status": "unavailable", "count": null}` in summaries
  and `{"state": "unavailable", "version": null}` in own state: never zero, never a stored
  version. `work.count` counts endorsements of the **current** publication's revision only
  (`work.revision_id`); endorsements of an earlier revision show as `endorsed_earlier_revision`
  to their owner and are not counted until reconfirmed. Summaries need no principal and never
  resolve one, so an identity outage cannot hide them; own state is `401` for a guest, never
  `not_endorsed`. Each read is one snapshot; summaries and own state are separate snapshots.
- **Capabilities are advisory.** `can_endorse`/`blocked_reason` come from the same decision on a
  read snapshot; every write decides again on the facts it locks.
- **Not provided:** voter lists or attribution, ranking/sorting by endorsements, counts on the
  works list or the publication read (READ-01 bodies and validators are unchanged), reasons or
  facets, rate limits.

## Errors

Every error body is `{"error": {"code", "message", "current_version"?, "field"?}}`. Branch on
`code`; `message` is developer text.

| Status | `code` | Meaning | Fixture |
| --- | --- | --- | --- |
| 422 | `validation_failed` | Body, title, document or `expected_version` invalid; `field` names it | [error.validation_failed.json](fixtures/error.validation_failed.json) |
| 401 | `unauthenticated` | No authenticated Vink principal | [error.unauthenticated.json](fixtures/error.unauthenticated.json) |
| 403 | `forbidden` | Origin check failed for an unsafe method | [error.forbidden.json](fixtures/error.forbidden.json) |
| 404 | `not_found` | Missing, private to someone else, or not published | [error.not_found.json](fixtures/error.not_found.json) |
| 409 | `version_conflict` | Draft changed since `expected_version`, comments settings set to the other state since `expected_generation`, a comment's moderation state since `expected_version`, or an endorsement set to the other state since `expected_version`; `current_version` is the stored one (endorsements also carry `endorsement`) | [draft](fixtures/error.version_conflict.json), [settings](fixtures/error.discussion_conflict.json), [moderation](fixtures/error.moderation_conflict.json), [endorsement](fixtures/error.endorsement_conflict.json) |
| 409 | `revision_changed` | The work was republished since the endorsing page was read; `current_revision_id` names the public revision | [error.revision_changed.json](fixtures/error.revision_changed.json) |
| 403 | `self_endorsement` | The work's author or the comment's writer tried to endorse their own content | [error.self_endorsement.json](fixtures/error.self_endorsement.json) |
| 409 | `comments_closed` | The work's author closed comments (new comments, replies and new comment endorsements) | [error.comments_closed.json](fixtures/error.comments_closed.json) |
| 409 | `target_unavailable` | Reply target missing, of another work or not public | [error.target_unavailable.json](fixtures/error.target_unavailable.json) |
| 422 | `idempotency_key_reused` | Same `command_key`, different payload; `field` is `command_key` | [error.idempotency_key_reused.json](fixtures/error.idempotency_key_reused.json) |
| 403 | `not_comment_writer` | Removing a visible comment written by someone else | [error.not_comment_writer.json](fixtures/error.not_comment_writer.json) |
| 403 | `not_moderator` | No active moderator grant | [error.not_moderator.json](fixtures/error.not_moderator.json) |
| 403 | `moderator_recused` | The moderator wrote the comment or owns the work | [error.moderator_recused.json](fixtures/error.moderator_recused.json) |
| 409 | `comment_removed` | A decision on a removed comment | [error.comment_removed.json](fixtures/error.comment_removed.json) |
| 503 | `dependency_unavailable` | PostgreSQL unreachable; retry later | [error.dependency_unavailable.json](fixtures/error.dependency_unavailable.json) |
| 500 | `internal` | Stored data failed validation, an unexpected database error, or an exhausted endorsement version | — |

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
