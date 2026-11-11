# Vink Identity private contract v1 (draft)

**Owner:** `vink-identity` ([src/contract.rs](../../../src/contract.rs), [src/http.rs](../../../src/http.rs)).
**Status:** implemented private contract, subject to the configured native methods and the D-05
production gate in the App. The only consumer is the App's
client ([services/vink-app/src/identity/client.rs](../../../../vink-app/src/identity/client.rs)), which keeps its own
representation, decodes these fixtures in its tests and does not import this crate. It also
checks that the socket is served by `VINK_IDENTITY_SERVER_UID` before sending anything.

Without configuration the provider has no authority and answers every verification with
`authority_not_configured`. The D-05/S-03 spike ([ADR](../../../../../docs/architecture/auth-authority.md))
introduced native accounts proven by a one-time email code and opaque server-side sessions.
The native authority now supports `email-code`, `password` or `native` (both methods). Mail delivery
uses SMTP or a local outbox. D-05 remains Provisional. The
`verify.authenticated.json`/`verify.rejected.json` fixtures come from a labelled unit-test authority
(`authority: "test-fixture"`) that is not compiled into the binary; the email-code fixtures below are
produced by the real types with fixed values.

## Transport and caller

HTTP/1.1 with JSON bodies on the Unix socket `VINK_IDENTITY_SOCKET`; there is no TCP listener.
The caller is the OS user of the connecting process (`SO_PEERCRED`/`getpeereid`), which must equal
`VINK_IDENTITY_CALLER_UID`. Without that setting every contract call is refused. Headers such as
`x-user-id`, `x-vink-principal`, `x-forwarded-for` or `authorization` are never read; a browser
never reaches this socket and nothing it sends can name a caller or a subject.

## Operations

| Request | Success | Errors |
| --- | --- | --- |
| `GET /identity/v1/capabilities` | `200` [not configured](fixtures/capabilities.not_configured.json), [email-code](fixtures/capabilities.email_code.json), [password](fixtures/capabilities.password.json) or [both](fixtures/capabilities.native.json) | `401` |
| `POST /identity/v1/verify-credential` with [verify.request.json](fixtures/verify.request.json) | `200` [authenticated](fixtures/verify.authenticated.json) or [rejected](fixtures/verify.rejected.json); email-code adds [session facts](fixtures/verify.authenticated.session.json) or a [reason](fixtures/verify.rejected.revoked.json) | `401`, `400`, `413`, `501`, `503` |
| `POST /identity/v1/sign-in/challenges` with [request](fixtures/sign_in.challenge.request.json) | `202` [challenge.issued.json](fixtures/challenge.issued.json) | `401`, `400`, `413`, `422 invalid_email`, `429 rate_limited`, `501`, `503` |
| `POST /identity/v1/sign-in/complete` with [request](fixtures/sign_in.complete.request.json) | `200` [authenticated](fixtures/sign_in.authenticated.json) or [rejected](fixtures/sign_in.rejected.json) | `401`, `400`, `413`, `501`, `503` |
| `POST /identity/v1/sessions/revoke` with [request](fixtures/session.revoke.request.json) | `200` [session.revoked.json](fixtures/session.revoked.json) | `401`, `400`, `413`, `501`, `503` |
| `POST /identity/v1/email-change/challenges` with [request](fixtures/email_change.challenge.request.json) | `202` [challenge.issued.json](fixtures/challenge.issued.json) | as sign-in, plus `403 credential_rejected`, `409 address_unavailable` |
| `POST /identity/v1/email-change/complete` with [request](fixtures/email_change.complete.request.json) | `200` [changed](fixtures/email_change.changed.json) or [rejected](fixtures/email_change.rejected.json) | `401`, `400`, `413`, `403 credential_rejected`, `501`, `503` |
| `POST /identity/v1/password/register` with [request](fixtures/password.register.request.json) | `200` [authenticated](fixtures/password.register.authenticated.json) or [rejected](fixtures/password.register.rejected.username_taken.json) | `401`, `400`, `413`, `422 invalid_username`, `422 invalid_password`, `429 rate_limited`, `501`, `503` |
| `POST /identity/v1/password/sign-in` with [request](fixtures/password.sign_in.request.json) | `200` [authenticated](fixtures/password.sign_in.authenticated.json) or [rejected](fixtures/password.sign_in.rejected.invalid_credentials.json) | `401`, `400`, `413`, `429 rate_limited`, `501`, `503` |
| `POST /identity/v1/password/enroll/challenges` with [session request](fixtures/password.enroll.challenge.request.json) | `202` [challenge issued](fixtures/challenge.issued.json) | `401`, `400`, `413`, `403 credential_rejected` or `account_disabled`, `409 already_enrolled` or `address_unavailable`, `429`, `501`, `503` |
| `POST /identity/v1/password/enroll` with [request](fixtures/password.enroll.request.json) | `200` [enrolled](fixtures/password.enroll.enrolled.json) or rejected with a reason | `401`, `400`, `413`, `403 credential_rejected`, `422 invalid_username` or `invalid_password`, `501`, `503` |
| `POST /identity/v1/password/enroll/status` with [session request](fixtures/password.enroll.challenge.request.json) | `200` [enrolled](fixtures/password.enroll.status.enrolled.json) or [unenrolled](fixtures/password.enroll.status.unenrolled.json) | `401`, `400`, `413`, `403 credential_rejected` or `account_disabled`, `501`, `503` |

The email-code operations require email-code. Registration and password sign-in require password.
All three enrollment operations require both methods. A missing method returns `501 authority_not_configured`.

`credential` is an opaque string (1–4096 bytes) that an end user presented to the caller, for
example a token issued by the future authority. Unknown request fields are rejected: a caller cannot
supply `subject` or any other claim.

`authenticated` is an **authentication fact**: `subject.authority` + `subject.id` (stable within
that authority; never an email or pen name) and `authenticated_at` (RFC 3339, UTC: when the person
proved control of the mailbox). It grants no Vink permission: mapping a subject to a Vink principal,
suspension and every role belong to the App. `rejected` means the authority checked the credential
and does not accept it; the optional `reason` (`unknown`, `expired`, `revoked`, `account_disabled`)
is informational and every value means "no authenticated subject".

### Email-code authority (spike)

- **Subject.** `authority: "vink-identity"`, `id` = the account UUID. It never changes when the
  address changes and is never reused; a freed address that is proven again creates a new account.
- **Challenges.** `sign-in/challenges` does not read accounts: the answer and the mail are the same
  for a new or existing address (no enumeration through this operation). At most 3 codes per
  address per 15 minutes (any purpose; `429`). A code is 6 digits, valid 10 minutes, 5 wrong
  attempts close the challenge. `challenge` is `vink_c1.<id>.<secret>`; the database keeps only
  `SHA-256(domain, secret, code)`, so the handle must stay with the client that asked (the App puts
  it in an HttpOnly cookie) and is useless without the mailed code, and vice versa.
- **Completion.** `rejected.reason`: `invalid_code` (another attempt allowed), `challenge_closed`
  (expired, used, out of attempts, unknown or not a handle: ask for a new code), `account_disabled`
  (the code is spent, no session). On success the account for the address is found or created —
  exactly one under concurrent first sign-ins — and a session is issued in the same transaction.
  `session.credential` (`vink_s1_…`, 256 random bits) appears **only** in this answer; the provider
  stores its SHA-256 and the caller hands it to the client without logging or keeping it.
- **Sessions.** Absolute lifetime 30 days, no sliding renewal. `verify-credential` checks the
  credential on every call (revocation is effective on the next call). `sessions/revoke` is
  idempotent logout (`revoked: false` when nothing active matched).
- **Delivery (D-05 production path).** `202` means the challenge and its delivery intent are
  committed together; the code is mailed afterwards by the provider's worker (at least once,
  bounded retries; a used or expired code is never sent). `202` is not proof of delivery, and the
  answer is the same whether the relay is up or down.
- **Email change.** Requires a valid session and mails the code to the *new* address. On success the
  account keeps its subject, every *other* session is revoked, and outstanding password enrollment
  challenges for the account are closed. The account's
  *own* current address is `409`. An address *another* account has verified answers `202` like a
  free one but no code is mailed (so the operation cannot probe who is registered); completing it
  would be `rejected.address_unavailable`, as when an address is taken meanwhile. An account may
  start 3 changes per hour.
- **Disable.** Only the operator command `vink-identity account disable|enable <subject>` changes it
  (no contract operation). Disabling revokes every session in the same transaction; re-enabling
  never restores them.

### Password enrollment

- `password/enroll/challenges` mails a six-digit code to the verified address of the account named
  by the session. Its purpose, account and issuing session are bound in the database. It uses the
  same 10-minute expiry, five failed attempts and address/account/global ceilings as email codes.
- `password/enroll` checks account and session standing again under lock. It consumes the proof
  and inserts one `account_passwords` row in the same transaction. Both methods then issue opaque
  sessions for the same subject. Enrollment leaves the presenting session and verified address intact.
- A `rejected.reason` of `invalid_code` allows another attempt. `challenge_closed` means a new
  proof is needed. A [username collision](fixtures/password.enroll.rejected.username_taken.json)
  leaves the proof usable. `already_enrolled` means a password is present and no credential was
  replaced. `account_disabled` means the account is disabled.
- If the completion response is lost, use `password/enroll/status` with the original session. It
  reads one boolean for that account and performs no mutation. A false read is a snapshot and an
  outstanding completion may still commit.
- New passwords follow O-12@r2. The raw UTF-8 field is at most 4096 bytes and NFKC normalization
  precedes the 10–128 code point check and Argon2id hash. Sign-in also verifies the original
  10–128 byte form for existing hashes written before normalization was introduced.

## Errors

Body: `{"error":{"code":"...","message":"..."}}` ([fixtures](fixtures/)). Messages are fixed
strings and never quote request content. Consumers branch on `code`.

| Status | `code` | Meaning for the consumer |
| --- | --- | --- |
| 401 | `caller_unauthenticated` | This process is not the configured caller (or none is configured); fix deployment |
| 400 | `malformed_request` | Body is not valid JSON for the operation, a field is missing/empty/too long or unknown |
| 413 | `payload_too_large` | Body over the operation's byte limit |
| 404 | `unsupported_version` | Path names a contract version other than `v1` (shown only to the caller) |
| 404 | `not_found` | No such v1 operation |
| 405 | `method_not_allowed` | Wrong method for the operation |
| 501 | `authority_not_configured` | No authority exists; nothing was verified. Treat as "no authenticated user", not as an outage |
| 503 | `dependency_unavailable` | The authority or its storage could not answer; nothing was decided. Treat as unavailable, never as rejected |
| 503 | `deadline_exceeded` | The call, including body upload, exceeded its deadline. A write may have committed |
| 422 | `invalid_email` | Email-code only: not a mailbox address (`local@domain.tld`, ≤ 254 bytes) |
| 429 | `rate_limited` | A challenge ceiling or password sign-in throttle refused this attempt |
| 403 | `credential_rejected` | Email change or enrollment. The presenting session is inactive |
| 403 | `account_disabled` | Enrollment challenge or status. The Identity account is disabled |
| 409 | `address_unavailable` | Email change to the account's own address or enrollment with no current verified address |
| 409 | `already_enrolled` | Enrollment challenge request for an account with an existing password |

Verification has no durable effect, so a consumer may retry `503` answers with its own deadline;
it must never turn a `503` or `501` into an authenticated principal.

## Limits and compatibility

- The request head is due within 2 s of connecting. The body limit is 8 KiB, or 32 KiB for
  requests carrying a password so the 4096-byte field fits after JSON escaping. The credential
  limit is 4096 bytes. Server deadlines include upload time and are 2 s for reads and 5 s for writes. A
  consumer should use a shorter client timeout (the App: 1.5 s and 4.5 s) and map its own timeout
  like `503`. A timeout after the provider committed is an uncertain outcome: a session issued but
  never delivered cannot be presented by anyone and expires; the consumed code needs a new one.
- A malformed path under `/identity/` is `not_found`; every answer under it uses the error envelope.
- The version is the path segment. Within `v1` the provider may add response fields and new
  operations; consumers must ignore unknown response fields. Removing or renaming a field, changing
  a code's meaning or accepting new request fields needs `v2`, served side by side until consumers move.
- The email-code operations, the `capabilities.operations` fields, `verify` `session`/`reason`
  and the four new error codes were **added within v1** under the rules above: no field was
  removed or renamed, no existing code changed meaning, and every new request type rejects
  unknown fields. A consumer that ignores unknown response fields (the App does) is unaffected.
- Health endpoints (`/health/live`, `/health/ready`) are operational, not part of this versioned
  contract; readiness reports `checks.database`, `checks.schema`, `checks.role` (`restricted` is
  required: a role that can change schema or roles is `privileged`), `checks.callers`, `authority`,
  `mail_transport` (`local_outbox` or `not_configured`) and
  `production_authentication: "not_implemented"`.

## Redaction

Session credentials, codes, challenge handles, addresses, usernames and passwords are redacted
from `Debug` and logs. Responses carry the session credential only at issuance and the challenge
handle only at challenge issuance. Parse errors are discarded. Logs record refusals and
issued sessions by session id and subject. Database URLs never appear in configuration errors.
