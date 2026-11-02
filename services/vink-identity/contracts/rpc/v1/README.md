# Vink Identity private contract v1 (draft)

**Owner:** `vink-identity` ([src/contract.rs](../../../src/contract.rs), [src/http.rs](../../../src/http.rs)).
**Status:** draft, introduced with commit 2 of `codex/identity-postgres-boundaries`. Every path,
field and code below is a **new design**, not an existing API. The only consumer is the App's
client ([services/vink-app/src/identity/client.rs](../../../../vink-app/src/identity/client.rs)), which keeps its own
representation, decodes these fixtures in its tests and does not import this crate. It also
checks that the socket is served by `VINK_IDENTITY_SERVER_UID` before sending anything.

No authentication authority is selected (D-05, S-03). The provider therefore answers every
verification with `authority_not_configured`; production authentication is NOT_IMPLEMENTED. The
`authenticated`/`rejected` fixtures are produced only by a labelled unit-test authority
(`authority: "test-fixture"`) that is not compiled into the binary.

## Transport and caller

HTTP/1.1 with JSON bodies on the Unix socket `VINK_IDENTITY_SOCKET`; there is no TCP listener.
The caller is the OS user of the connecting process (`SO_PEERCRED`/`getpeereid`), which must equal
`VINK_IDENTITY_CALLER_UID`. Without that setting every contract call is refused. Headers such as
`x-user-id`, `x-vink-principal`, `x-forwarded-for` or `authorization` are never read; a browser
never reaches this socket and nothing it sends can name a caller or a subject.

## Operations

| Request | Success | Errors |
| --- | --- | --- |
| `GET /identity/v1/capabilities` | `200` [capabilities.not_configured.json](fixtures/capabilities.not_configured.json) | `401` |
| `POST /identity/v1/verify-credential` with [verify.request.json](fixtures/verify.request.json) | `200` [authenticated](fixtures/verify.authenticated.json) or [rejected](fixtures/verify.rejected.json) once an authority exists | `401`, `400`, `413`, `501`, `503` |

`credential` is an opaque string (1–4096 bytes) that an end user presented to the caller, for
example a token issued by the future authority. Unknown request fields are rejected: a caller cannot
supply `subject` or any other claim.

`authenticated` is an **authentication fact**: `subject.authority` + `subject.id` (stable within
that authority; never an email or pen name) and `authenticated_at` (RFC 3339, UTC). It is not a
session and grants no Vink permission. Mapping a subject to a Vink principal, deciding freshness and
issuing an application session belong to the App. `rejected` means the authority checked the
credential and does not accept it.

## Errors

Body: `{"error":{"code":"...","message":"..."}}` ([fixtures](fixtures/)). Messages are fixed
strings and never quote request content. Consumers branch on `code`.

| Status | `code` | Meaning for the consumer |
| --- | --- | --- |
| 401 | `caller_unauthenticated` | This process is not the configured caller (or none is configured); fix deployment |
| 400 | `malformed_request` | Body is not valid JSON for the operation, a field is missing/empty/too long or unknown |
| 413 | `payload_too_large` | Body over 8192 bytes |
| 404 | `unsupported_version` | Path names a contract version other than `v1` (shown only to the caller) |
| 404 | `not_found` | No such v1 operation |
| 405 | `method_not_allowed` | Wrong method for the operation |
| 501 | `authority_not_configured` | No authority exists; nothing was verified. Treat as "no authenticated user", not as an outage |
| 503 | `dependency_unavailable` | The authority or its storage could not answer; nothing was decided. Treat as unavailable, never as rejected |
| 503 | `deadline_exceeded` | The call (body upload included) did not finish within 2 s; nothing was decided |

Verification has no durable effect, so a consumer may retry `503` answers with its own deadline;
it must never turn a `503` or `501` into an authenticated principal.

## Limits and compatibility

- Request head within 2 s of connecting (the connection is closed otherwise); body ≤ 8192 bytes;
  credential ≤ 4096 bytes; server deadline 2 s per call, body upload included. A consumer should
  use a shorter client timeout and map its own timeout like `503`.
- A malformed path under `/identity/` is `not_found`; every answer under it uses the error envelope.
- The version is the path segment. Within `v1` the provider may add response fields and new
  operations; consumers must ignore unknown response fields. Removing or renaming a field, changing
  a code's meaning or accepting new request fields needs `v2`, served side by side until consumers move.
- Health endpoints (`/health/live`, `/health/ready`) are operational, not part of this versioned
  contract; readiness reports `checks.database`, `checks.schema`, `checks.role` (`restricted` is
  required: a role that can change schema or roles is `privileged`), `checks.callers`, `authority`
  and `production_authentication: "not_implemented"`.

## Redaction

The credential is never logged, echoed or printed (`Credential`'s `Debug` is redacted, parse errors
are discarded). Logs record refusals with the peer uid only. Database URLs never appear in
configuration errors.
