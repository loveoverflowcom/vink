# Vink HTTP API v1 (bootstrap)

**Owner:** `vink-app` (`services/vink-app/src/http/contract.rs`). **Status:** implemented for the
skeleton; not a stable public API. Consumers keep their own types: the web app's live in
`apps/vink-web/src/contract.rs`. Fixtures in [fixtures/](fixtures/) are checked on both sides:
the provider serializes its responses and compares them field-for-field; the consumer decodes
every fixture and must keep every field.

Only the endpoints below exist. Publish/withdraw and account endpoints do not. Discussion
(comments) is Vink-owned (owner decision 03/11/2026; no Discourse) and has the endpoints in
[Discussion](#discussion-d2t-02b) and [Comment moderation](#comment-moderation-d2t-02c).

## Conventions

- JSON, UTF-8. Identifiers are UUID strings; timestamps are RFC 3339 in UTC.
- `document` uses schema version 1: `{"schema_version": 1, "blocks": [{"lines": ["…"]}]}`.
  A block is a paragraph or stanza; each line is non-blank, without line breaks or control
  characters (tab allowed). The server stores lines NFC-normalized. At most 200 000
  characters; titles at most 200 characters, single line, trimmed.
- Unknown request fields are rejected.
- Unsafe methods (`POST`, `PUT`) require an `Origin` header equal to the server's
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
  `expected_generation` guards against a stale tab: asking for the current state is `200`
  without a change; another change since the given generation is `409 version_conflict` with
  the current generation in `current_version`. Closing stops new comments and replies; approved
  comments stay readable. Comments start open on a public work.
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

## Errors

Every error body is `{"error": {"code", "message", "current_version"?, "field"?}}`. Branch on
`code`; `message` is developer text.

| Status | `code` | Meaning | Fixture |
| --- | --- | --- | --- |
| 422 | `validation_failed` | Body, title, document or `expected_version` invalid; `field` names it | [error.validation_failed.json](fixtures/error.validation_failed.json) |
| 401 | `unauthenticated` | No authenticated Vink principal | [error.unauthenticated.json](fixtures/error.unauthenticated.json) |
| 403 | `forbidden` | Origin check failed for an unsafe method | [error.forbidden.json](fixtures/error.forbidden.json) |
| 404 | `not_found` | Missing, private to someone else, or not published | [error.not_found.json](fixtures/error.not_found.json) |
| 409 | `version_conflict` | Draft changed since `expected_version`, comments settings since `expected_generation`, or a comment's moderation state since `expected_version`; `current_version` is the stored one | [draft](fixtures/error.version_conflict.json), [settings](fixtures/error.discussion_conflict.json), [moderation](fixtures/error.moderation_conflict.json) |
| 409 | `comments_closed` | The work's author closed comments | [error.comments_closed.json](fixtures/error.comments_closed.json) |
| 409 | `target_unavailable` | Reply target missing, of another work or not public | [error.target_unavailable.json](fixtures/error.target_unavailable.json) |
| 422 | `idempotency_key_reused` | Same `command_key`, different payload; `field` is `command_key` | [error.idempotency_key_reused.json](fixtures/error.idempotency_key_reused.json) |
| 403 | `not_comment_writer` | Removing a visible comment written by someone else | [error.not_comment_writer.json](fixtures/error.not_comment_writer.json) |
| 403 | `not_moderator` | No active moderator grant | [error.not_moderator.json](fixtures/error.not_moderator.json) |
| 403 | `moderator_recused` | The moderator wrote the comment or owns the work | [error.moderator_recused.json](fixtures/error.moderator_recused.json) |
| 409 | `comment_removed` | A decision on a removed comment | [error.comment_removed.json](fixtures/error.comment_removed.json) |
| 503 | `dependency_unavailable` | PostgreSQL unreachable; retry later | [error.dependency_unavailable.json](fixtures/error.dependency_unavailable.json) |
| 500 | `internal` | Stored data failed validation or an unexpected database error | — |

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
