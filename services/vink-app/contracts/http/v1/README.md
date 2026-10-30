# Vink HTTP API v1 (bootstrap)

**Owner:** `vink-app` (`services/vink-app/src/http/contract.rs`). **Status:** implemented for the
skeleton; not a stable public API. Consumers keep their own types: the web app's live in
`apps/vink-web/src/contract.rs`. Fixtures in [fixtures/](fixtures/) are checked on both sides:
the provider serializes its responses and compares them field-for-field; the consumer decodes
every fixture and must keep every field.

Only the endpoints below exist. Moderation, publish/withdraw, Discourse and account endpoints
do not.

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
  body or a header selects the actor. Draft responses carry `Cache-Control: no-store`.

## Endpoints

| Method and path | Success | Errors | Fixture |
| --- | --- | --- | --- |
| `GET /api/v1/works` | `200` list of current publications (at most 12) | `503` | [works.list.json](fixtures/works.list.json) |
| `GET /api/v1/works/{work_id}/publication` | `200` the public snapshot (never the draft) | `404`, `503` | [publication.get.json](fixtures/publication.get.json) |
| `GET /api/v1/drafts` | `200` the actor's drafts | `401`, `503` | [drafts.list.json](fixtures/drafts.list.json) |
| `POST /api/v1/drafts` | `201` a new empty draft at version 1 | `401`, `403`, `503` | [draft.get.json](fixtures/draft.get.json) (same shape) |
| `GET /api/v1/drafts/{work_id}` | `200` the actor's draft | `401`, `404`, `503` | [draft.get.json](fixtures/draft.get.json) |
| `PUT /api/v1/drafts/{work_id}` | `200` new version | `401`, `403`, `404`, `409`, `422`, `503` | [request](fixtures/draft.save.request.json), [response](fixtures/draft.save.response.json) |

`PUT` writes only if the stored version equals `expected_version` and the actor owns the work;
the check and the write are one statement. Otherwise nothing is written. A draft that does not
exist and a draft owned by someone else both answer `404`. A malformed `work_id` is `404`.

`POST` is not idempotent: a retried request after a lost response may create a second empty
draft. Clients disable the button while a request is pending.

## Errors

Every error body is `{"error": {"code", "message", "current_version"?, "field"?}}`. Branch on
`code`; `message` is developer text.

| Status | `code` | Meaning | Fixture |
| --- | --- | --- | --- |
| 422 | `validation_failed` | Body, title, document or `expected_version` invalid; `field` names it | [error.validation_failed.json](fixtures/error.validation_failed.json) |
| 401 | `unauthenticated` | No authenticated Vink principal | [error.unauthenticated.json](fixtures/error.unauthenticated.json) |
| 403 | `forbidden` | Origin check failed for an unsafe method | [error.forbidden.json](fixtures/error.forbidden.json) |
| 404 | `not_found` | Missing, private to someone else, or not published | [error.not_found.json](fixtures/error.not_found.json) |
| 409 | `version_conflict` | Draft changed since `expected_version`; `current_version` is the stored one | [error.version_conflict.json](fixtures/error.version_conflict.json) |
| 503 | `dependency_unavailable` | PostgreSQL unreachable; retry later | [error.dependency_unavailable.json](fixtures/error.dependency_unavailable.json) |
| 500 | `internal` | Stored data failed validation or an unexpected database error | — |

## Health (not versioned)

`GET /health/live` answers `200` while the process serves HTTP. `GET /health/ready` answers
`200` only when PostgreSQL is reachable and every embedded migration is applied, otherwise
`503` with `checks.database` (`ok`/`unavailable`) and `checks.schema`
(`current`/`pending_migrations`/`unknown`). It also reports `integrations.discourse` and
`integrations.ai_moderation` as `not_implemented` and `integrations.jobs` as `none`.
