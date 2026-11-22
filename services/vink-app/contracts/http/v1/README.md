# Vink HTTP API v1 (bootstrap)

**Owner:** `vink-app` (`services/vink-app/src/http/contract.rs`). **Status:** implemented for the
skeleton; not a stable public API. Consumers keep their own types: the web app's live in
`apps/vink-web/src/contract.rs`. Fixtures in [fixtures/](fixtures/) are checked on both sides:
the provider serializes its responses and compares them field-for-field; the consumer decodes
every fixture and must keep every field.

The endpoints below include author publication, [authentication and account
session](#sign-in-d-05). Profile editing has no endpoint. Discussion
(comments) is Vink-owned (owner decision 03/11/2026; no Discourse) and has the endpoints in
[Discussion](#discussion-d2t-02b) and [Comment moderation](#comment-moderation-d2t-02c). Literary
endorsement has the endpoints in [Literary endorsement](#literary-endorsement-lve-73); no web or
native consumer uses them yet. Work tags have the endpoints in [Work tags](#work-tags-tag-pr-03):
public catalog reads and the author's private draft tags; no route changes the catalog. The web
desk (TAG-PR-05) consumes the catalog list, suggestions and draft tags; slug resolution has no
consumer yet and no native consumer exists.

The [proposed Social Graph contract r1](social-graph.md) defines public, own and viewer representations, authorization before conditional reads, and command reconciliation for #288. The [implemented author address routes](social-graph.md#địa-chỉ-tác-giả-290) serve the minimal public profile, setup, rename and receipt readback, and the [follow graph routes (#294)](social-graph.md#theo-dõi-tác-giả-và-danh-sách-quan-hệ-294) serve follow intent, counts, lists, receipts and batch viewer membership.

The [proposed Notifications v1 contract](notifications.md) (`NOT_IMPLEMENTED`, #345) defines the shared inter-service envelope/receipt/ACK semantics, gateway badge/inbox/`Seen`/`Read`/private decision-detail semantics, and the proposed public paginated **Thấm** list for #346–#348 and non-Web consumers (`P-14A`).

## Conventions

- JSON, UTF-8. Identifiers are UUID strings; timestamps are RFC 3339 in UTC.
- `document` is the one canonical, unversioned document (#255,
  [architecture](../../../../../docs/architecture/document-source-format.md),
  `VDR-DOCUMENT-SOURCE-FORMAT@r2` `ACCEPTED`): `{"format": "default" | "markdown", "source": "…"}`.
  `source` is the author's exact text: Unicode form (NFD stays NFD), emoji sequences, tabs,
  indentation, blank and whitespace-only lines and line endings are stored and returned byte for
  byte. Refused: control characters other than tab/LF/CR, U+2028/U+2029, more than 200 000
  characters (counted after NFC, line breaks excluded) or 2 097 152 bytes. `asciidoc` is deferred
  and refused; unknown formats and any other key (including the retired `schema_version`/`blocks`)
  are refused. There is no v1/v2 document and no legacy reader.
- Title authority (#263): for `default`, the request's `title` is required (blank allowed in a
  draft; trimmed, NFC, single line, at most 200 characters). For `markdown`, `title` must be absent
  or `null`: the title is the first H1 of the CommonMark source (core dialect, no extensions),
  flattened to text; no H1 or an empty first H1 gives an empty title (clients show “Chưa đặt
  tên”). Subsequent headings remain body text. An invalid first H1 returns 422
  `validation_failed` at `title`. Its message points to the first H1 in `document.source`,
  where the author can correct it. Every `title` in responses is the effective title.
  Snapshots keep the title of the captured source.
- Both canonical formats are eligible for `POST …/publication/request-public` (#264). The request
  still names an exact revision and generation. Moderation and current public intent determine which
  revision appears in the public read models.
- Unknown request fields are rejected.
- Unsafe methods (`POST`, `PUT`, `DELETE`) require an `Origin` header equal to the server's
  `VINK_PUBLIC_ORIGIN`; otherwise `403 forbidden` before any use case runs.
- Draft endpoints act for the principal resolved **by the server**. No authority is
  configured in production yet (D-05), so they answer `401` there. Nothing in the request
  body or a header such as `x-user-id` selects the actor. Draft responses carry
  `Cache-Control: no-store`.
- **Session carrier (D-05/S-03 spike):** when the App is configured with the Identity process
  (`VINK_IDENTITY_SOCKET`), a request presents a session credential either as the
  `__Host-vink_session` cookie (Web; set only by [Sign-in](#sign-in-d-05)) or as
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
| `GET /api/v1/works/{work_id}/reader` | `200` `{"status":"published", …publication}` (conditional like `/publication`) or, after a moderator/admin rejected the public revision (`O-17@r2`), `{"status":"review_withdrawn","work_id","author_name"}` with `no-store` and no validator; a public work made private returns `{"status":"hidden","work_id"}` (PUB-07), also `no-store`, with no author, title, body or tags | `404` (never public, private without a hidden page, deleted or removed), `503` | — |
| `GET /api/v1/drafts` | `200` trang nháp của actor; `q`, `sort`, `limit`, `cursor` | `401`, `422`, `503` | [legacy/end](fixtures/drafts.list.json), [còn trang](fixtures/drafts.page.json) |
| `POST /api/v1/drafts` | `201` a new empty draft at version 1 (`{"format":"default"\|"markdown"}` optional; omitted/empty = `default`) | `401`, `403`, `422`, `503` | [draft.get.json](fixtures/draft.get.json) (same shape) |
| `GET /api/v1/drafts/{work_id}` | `200` the actor's draft | `401`, `404`, `503` | [draft.get.json](fixtures/draft.get.json) |
| `PUT /api/v1/drafts/{work_id}` | `200` new version | `401`, `403`, `404`, `409`, `422`, `503` | [request](fixtures/draft.save.request.json), [response](fixtures/draft.save.response.json) |
| `GET /api/v1/works/{work_id}/publication/state` (`/publication/me`) | `200` the author's own state, `no-store`: `audience`, `lifecycle`, `generation`, `intent`, `pointer`, `revisions` (oldest first, each with `moderation`, `decision_version`, `decision_reason`, `can_appeal`), `open_ticket` | `401`, `403`, `404`, `503` | [public r1 + pending r2](fixtures/publication.state.public_r1_pending_r2.json), [rejected](fixtures/publication.state.public_rejected.json), [private](fixtures/publication.state.private.json) |
| `POST /api/v1/works/{work_id}/snapshots` | `201` (`200` on replay) a revision frozen at exactly `expected_draft_version` + `expected_metadata_version`; a replay with the same `command_key` and versions returns the first revision even after the draft moved on | `401`, `403`, `404`, `409` `version_conflict` (`field`: `expected_draft_version` or `expected_metadata_version`, with `current_version`), `422` (`idempotency_key_reused`, or `validation_failed` at `title` for an untitled draft), `503` | [request](fixtures/snapshot.capture.request.json), [response](fixtures/snapshot.capture.response.json), [draft moved](fixtures/error.snapshot_draft_conflict.json), [tags moved](fixtures/error.snapshot_metadata_conflict.json) |
| `POST /api/v1/works/{work_id}/publication/request-public` | `200` `submitted`/`waiting`/`published`/`reaffirmed` with the new `generation`; a replay (`replayed: true`) may omit `ticket_id`: read the state | `401`, `403`, `404` (`unknown_revision`), `409` (`version_conflict` with `field: expected_generation`, `revision_rejected`, `work_unavailable`), `422`, `503` | [request](fixtures/publication.request_public.request.json), [response](fixtures/publication.request_public.response.json), [replayed](fixtures/publication.request_public.response.replayed.json), [stale](fixtures/error.generation_conflict.json), [rejected](fixtures/error.revision_rejected.json), [unknown](fixtures/error.unknown_revision.json), [inactive](fixtures/error.work_unavailable.json), [key reused](fixtures/error.publication_key_reused.json) |
| `POST /api/v1/works/{work_id}/publication/set-private` | `200` `made_private` with the new `generation` | as above | [request](fixtures/publication.set_private.request.json), [response](fixtures/publication.set_private.response.json) |

### Danh sách nháp riêng (#201)

`GET /api/v1/drafts` luôn xác thực actor từ session; không nhận owner từ URL/cursor.
Mọi response API (kể cả lỗi) là `Cache-Control: no-store`, không có body bản thảo.
Chỉ trả `work_id`, `title`, `version`, `updated_at` trong `drafts`; thêm `next_cursor`
khi còn trang. Thiếu/null `next_cursor` là đã hết; không có tổng đếm. Decoder cũ bỏ qua
field thêm, decoder mới nhận cả fixture cũ. Không tham số vẫn Recent, tối đa 100 nháp.

- `q`: tối đa 200 ký tự sau NFC, 3200 byte UTF-8 thô, không NUL. Mặc định/rỗng là tất cả.
  Tìm **chuỗi con nguyên văn** của tiêu đề hiển thị sau NFD, bỏ combining marks, lowercase
  từng ký tự, Đ/đ → d, trim/gộp Unicode whitespace. Giữ dấu câu, `%` và `_` là ký tự thường.
  Tiêu đề rỗng hiển thị “Chưa đặt tên”. Không tìm nội dung; không dùng semantics word-prefix
  hoặc bỏ dấu câu của discovery công khai. Chuỗi tác giả lưu không bị thay bởi khóa so sánh.
- `sort`: `recent` (mặc định): `updated_at DESC, work_id ASC`; `title`:
  folded display-title ASC theo byte UTF-8 (`COLLATE "C"`), rồi `work_id ASC`.
- `limit`: số nguyên ASCII `1..=100`, mặc định 100. Web mới gửi giới hạn tường minh.
- `cursor`: chuyển nguyên `next_cursor` của trang trước. Lowercase hex v1, tối đa 6500 byte;
  chứa vị trí và SHA-256 scope (principal, folded query, sort, limit), không chứa credential.
  Một query có cùng khóa gập vẫn cùng scope; đổi owner/sort/limit/query-key phải bắt đầu lại.
  Cursor không ký và không cấp quyền: sửa vị trí hợp lệ chỉ di chuyển trong corpus của actor.
  Server không bảo đảm cursor là token đã phát hành. Version/scope/shape/UTF-8 sai bị từ chối.
- Không chấp nhận field lạ hoặc tham số lặp, thiếu `=`, `%` hỏng/UTF-8 không hợp lệ;
  raw query tối đa 32768 byte. Lỗi `422 validation_failed`, `field` là `query/q/sort/limit/cursor`.
  Không echo dữ liệu đầu vào trong lỗi hoặc ghi query/cursor vào request trace. Thứ tự:
  xác thực actor → decode URL → q → sort → limit → cursor → PostgreSQL.

SQL filter owner + query + keyset và sort **trước** `LIMIT limit+1`, chỉ một statement,
không đọc từng card riêng. Trang là snapshot của một statement; nhiều trang là **live keyset**,
không snapshot toàn danh sách. Draft sửa tiêu đề/thời gian có thể di chuyển qua cursor;
refresh bắt đầu chain mới, không ghép đầu mới với đuôi cũ. “Viết tiếp” đọc Recent của owner
(`q` rỗng, `limit=1`) độc lập filter/sort đang xem. Đọc nháp không đi vào SSR payload,
public cache hoặc READ-02. [Owner triển khai và query plan](../../../../../docs/development/desk-pagination.md).

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
- **Answer.** `works`: cards (`work_id`, `title`, `excerpt`, `author_display_name`, `published_at`, `tags`; `excerpt` is `null` or a bounded plain-text snippet derived from the published revision body).
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

Draft-content `PUT` alone accepts bodies up to **8,388,608 bytes (8 MiB)**, including JSON
framing, title and `expected_version`. An anonymous draft save returns `401` before the App reads
its body. Draft tags retain Axum's default 2 MiB cap. Auth routes keep their own 32 KiB cap.
The compact UTF-8 Web serializer needs at most 4,199,232 bytes for a canonical request,
including the escaped 2 MiB source, an optional trimmed title of 200 NFC scalars and framing.
Markdown omits the client title. Web trims the title as the domain already does, retaining the live input.
Arbitrary extra whitespace, title padding or alternative escaping is accepted only within the
cap. Oversize (declared length or chunked) and malformed bodies keep the existing error shape:
`422 validation_failed`, `field: body`, `Cache-Control: no-store`, without changing durable
content/version. This is not a smaller product-content limit or a global body-cap expansion.
See [bound, resource measurements and #285 integration](../../../../../docs/development/draft-http-budget.md).

`POST` is not idempotent: a retried request after a lost response may create a second empty
draft. Clients disable the button while a request is pending.

### Conditional public reads (READ-01)

Only the public reads above are conditional (the reader's `review_withdrawn` and `hidden` answers are not); `HEAD` answers like `GET` without content.
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
discussion response carries `Cache-Control: no-store`, errors included: the composed read and the
overlay contain viewer-specific state, and the public projection has no validator yet, so no
cache may keep a body (or a `404`/`503`) past a removal, revocation or recovery.

| Method and path | Success | Errors | Fixture |
| --- | --- | --- | --- |
| `GET /api/v1/works/{work_id}/discussion?cursor=&limit=` | `200` root page of a public work | `404`, `422` (`cursor`, `limit`, `query`), `503` | [discussion.get.json](fixtures/discussion.get.json) |
| `GET /api/v1/works/{work_id}/discussion/roots/{root_id}/replies?cursor=&limit=` | `200` replies of one root | `404`, `422`, `503` | [discussion.replies.json](fixtures/discussion.replies.json) |
| `GET /api/v1/works/{work_id}/discussion/public?cursor=&limit=` | `200` public projection of the root page (#120 DISC-02/03) | `404`, `422` (`cursor`, `limit`, `query`), `503` | [discussion.public.get.json](fixtures/discussion.public.get.json) |
| `GET /api/v1/works/{work_id}/discussion/public/roots/{root_id}/replies?cursor=&limit=` | `200` public replies of one root | `404`, `422`, `503` | [discussion.public.replies.json](fixtures/discussion.public.replies.json) |
| `GET /api/v1/works/{work_id}/discussion/me?cursor=&limit=` or `?ids=` | `200` the viewer's private overlay (#120 DISC-02/03) | `401`, `404`, `422` (`cursor`, `limit`, `ids`, `query`), `503` | [list](fixtures/discussion.me.get.json), [exact](fixtures/discussion.me.ids.json) |
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
- **Public projection** (`…/discussion/public`, #120 DISC-02/03). The same public comments,
  anchors, `public_replies`, order, previews and `c1.` cursors as the composed read gives a guest,
  without any viewer field: no `visibility`, `can_remove`, `rejection_reason` or
  `viewer_is_work_author`. It resolves no actor, so an Identity outage or a stale session cookie
  never hides it, and a request that names a principal gets the same body as a guest. The server
  refuses to build it if a viewer fact reaches it (`500`, never a partial body). The composed
  `…/discussion` read is unchanged and still resolves the actor (`503` while Identity cannot
  answer).
- **Private overlay** (`…/discussion/me`, #120 DISC-02/03). Only the authenticated viewer's own
  comments of a public work; `401` for a guest before any input is checked; `404` like the other
  reads. Two forms, never mixed (`422 ids`):
  - `?cursor=&limit=`: the viewer's present, unpublished (`pending` or `rejected`) comments,
    oldest first, wherever they sit in the public lists — a root on a later page, or a reply
    under a root that is no longer listed (`placement.root_listed: false`). `limit` 1–50
    (default 20); `next_cursor` is an `o1.` cursor that no public list accepts, and vice versa.
  - `?ids=` 1–20 comma-separated comment IDs (duplicates count once): the current `status`
    (`pending`, `approved`, `rejected`, `removed`) of those the viewer wrote in this work, read
    from the comment row at this request — not from a submit receipt. Other people's or unknown
    IDs are omitted, indistinguishable from each other. `next_cursor` is `null`.

  Each item: `status`, `body` (`null` once removed), `created_at`, `placement`
  (`{"kind": "root"}` or `{"kind": "reply", "root_id", "root_listed", "reply_to"}` with the same
  `reply_to` rules as above), `rejection_reason` (only while `rejected`) and `can_remove`
  (while present). A root's text or writer is never part of the overlay. A client composes
  overlay and public lists by `comment_id`: an approved comment leaves the overlay list and
  appears once in the public list. Drop the overlay on sign-out or actor change and ignore late
  answers of an earlier actor; an overlay error is not a lost comment.
- **Paging.** Roots and replies are ordered `created_at ASC, comment_id ASC`. `limit` is 1–50
  (default 20). Cursors are opaque; pass `next_cursor` back unchanged. Each root previews up to
  three replies; `preview_next_cursor` continues that root's replies endpoint after the preview.
  Paging is not a snapshot: a comment approved later can appear before an issued cursor.
  A cursor is accepted only exactly as issued: at most 56 bytes, the list's own prefix (`c1.`
  thread lists, `o1.` overlay) and the canonical spelling (no `+`, leading zeros, `-0` or
  upper-case hex); anything else is `422 cursor`, never clamped or ignored. A repeated query
  parameter is `422 query`. A cursor is a position, not a permission: every page re-applies its
  visibility and viewer filters.
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

## Work moderation (#159)

Authority is server data: admin seats (`vink-app admin add|remove|list`, audited), admin-issued
grants (`works`/`comments`, a principal or a group, all of Vink or one work, optional expiry) and
groups. Every command runs in one transaction behind the standing fence and is audited; every
response is `no-store`. A reviewer never reads or takes a case on their own work (`403` code
`recused`). Details and lock order: [moderation runtime](../../../../../docs/development/moderation-runtime.md).

| Method and path | Who | Success | Errors |
| --- | --- | --- | --- |
| `GET /api/v1/moderation/works/queue` | grant `works` or admin | `200` open cases the caller may review | `401`, `403` |
| `GET /api/v1/moderation/works/cases/{case_id}` | same | `200` case, notes and the exact submitted snapshot (`content_digest`, `title`, `document`) plus caller-scoped `viewer_is_admin` | `401`, `403` (`recused`, `not_authorized`), `404` |
| `POST /api/v1/moderation/works/{work_id}/revisions/{revision_id}/cases` | authority on the work | `201` `{case_id, version: 1}` intervention case at the current decision version (MOD-13) | `401`, `403`, `404`, `409` `case_already_open` |
| `POST …/cases/{case_id}/claim` · `release` · `escalate` `{expected_version}` | claimant rules of `ReviewCase` | `200` `{status, version}` | `401`, `403` (`not_claimant`, `escalated_to_admin`, authority codes), `404`, `409` (`case_version_stale`, `already_claimed`, `case_closed`) |
| `POST …/cases/{case_id}/note` `{expected_version, note}` | claimant or admin | `200`; the note is stored only with the version change | same |
| `POST …/cases/{case_id}/takeover` `{expected_version, reason}` | admin | `200` | same, `422` |
| `POST …/cases/{case_id}/conclude` `{expected_version, verdict: approved\|rejected, reason}` | claimant | `200` `{status, verdict}`; decision, history, pointer and jobs in one commit | same, `409` `stale_decision`; a claimant who lost every basis gets `403` and the case returns to the queue |
| `GET /api/v1/moderation/grants` · `POST` `{grantee_kind, grantee_id, capability, scope_kind, scope_work_id?, expires_at?}` | admin | `200` list · `201` `{grant_id}` | `401`, `403`, `409`, `422` |
| `POST /api/v1/moderation/grants/{grant_id}/revoke` | admin | `204`; claims resting on it are revalidated in the same commit | `401`, `403`, `404`, `409` |
| `POST /api/v1/moderation/groups` `{name}` · `POST …/groups/{group_id}/members` `{principal_id}` · `DELETE …/members/{principal_id}` | admin | `201` `{group_id}` · `204` · `204` (removed member's claims revalidated) | `401`, `403`, `404`, `409`, `422` |
| `GET /api/v1/moderation/automation` · `POST …/automation/mode` `{expected_version, mode: enabled\|paused, reason}` | admin | `200` `{mode, version}`; a pause reroutes open jobs to people in the same commit | `401`, `403`, `409` `automation_version_stale`, `422` |

### Work/appeal queue continuation (#205)

- `GET /api/v1/moderation/works/queue/page` và `GET /api/v1/moderation/appeals/page`
  trả `{items, next_cursor, authority_key}`. Query duy nhất: `limit` (mặc định 50, 1–100) và `cursor`.
  `next_cursor: null` chỉ có nghĩa hết trang sau response 200. Không có total.
- Cursor v1 hex chính tắc, ràng buộc actor + loại queue + limit + thứ tự cố định
  `created_at ASC, id ASC`; chỉ chứa vị trí của mục cuối được phép trả về. Không chứa lookahead
  bị từ chối. Không phải chữ ký hay quyền; sửa vị trí không bỏ được bất kỳ kiểm tra quyền nào.
- Query lạ/trùng/malformed UTF-8/percent: `422 validation_failed`, field `query`;
  limit sai: field `limit`; cursor sai version/actor/endpoint/limit: field `cursor`.
  Lỗi không echo cursor. Success/error đều `no-store`.
- Quyền/admin/standing/nhóm/expiry và rows cùng một repeatable-read snapshot mỗi request;
  recusal và public-active lọc trước LIMIT + 1. Queue sống, không snapshot quyền xuyên trang.
  Anchor đã đóng không chặn đi tiếp. Refresh bắt đầu chuỗi mới; detail/command kiểm quyền riêng.
- `authority_key` là fingerprint opaque của phạm vi quyền hiệu lực, không phải grant. Khi đổi
  (kể cả 200 sau mất một phần grant), Web bỏ chuỗi cũ và bắt đầu lại. Không suy diễn total hay ID từ key.
- Tương thích dạng body: route array cũ `/moderation/works/queue`, `/moderation/appeals` vẫn
  nhận cùng query, nay hữu hạn 50/100 và gửi `Link: <...>; rel="next"` nếu còn trang.
  Web mới chỉ dùng `/page`; client cũ vẫn parse được array, cần theo Link để duyệt hết.
  Bundle Web trước #205 bỏ qua Link, nên chỉ hiển thị trang đầu (tối đa 50), không thể duyệt hết
  queue lớn. Đây không phải tương thích chức năng đầy đủ giữa hai phiên bản. Rollout phải có
  server `/page` và migration trước bundle mới, rồi yêu cầu tải lại các tab kiểm duyệt cũ trước
  khi tiếp tục duyệt; nếu cần giữ nguyên tab cũ thì chưa đáp ứng điều kiện rollout.
  Không silently fallback từ lỗi `/page` sang empty hay route cũ.
- Fixtures: [trang tác phẩm](fixtures/moderation.works.page.json),
  [trang cuối](fixtures/moderation.works.end.json), [rỗng](fixtures/moderation.queue.empty.json),
  [phản đối](fixtures/moderation.appeals.page.json). Author appeal history `/works/{id}/appeals`
  và revision history giữ contract riêng, chưa được phân trang trong #205.

## Comment moderation (D2T-02c)

Authority comes from active comment moderation governance in Vink's database: an active admin seat
(`vink-app admin`), an active individual or group grant with capability `"comments"` in `global`
or matching `work` scope (`/api/v1/moderation/grants`, `/api/v1/moderation/groups`), or an
inherited legacy operator grant (`vink-app moderator grant|revoke|list`). Owning a work never
implies moderation authority. Each queue read checks live capability/membership/expiry/standing in
its snapshot. Each decision resolves against locked authority rows and checks standing and expiry
after waiting on the comment and principal locks. A moderator **recuses** from comments they wrote
and from comments on works they own (`403 moderator_recused` on an otherwise authorized target).
`403 not_moderator` covers missing `comments` authority and, for a work-scoped moderator, both
out-of-scope and missing comment IDs. Global reviewers receive `404` for a missing ID. Both routes
send `Cache-Control: no-store`.

| Method and path | Success | Errors | Fixture |
| --- | --- | --- | --- |
| `GET /api/v1/moderation/comments?cursor=&limit=` | `200` pending comments in the caller's scope (excluding recused items), oldest first, with `pending_total` | `401`, `403` (`not_moderator`, `principal_suspended`), `422`, `503` | [moderation.queue.json](fixtures/moderation.queue.json) |
| `POST /api/v1/moderation/comments/{comment_id}/decisions` | `200` the comment's updated review state | `401`, `403` (`forbidden`, `not_moderator`, `moderator_recused`, `principal_suspended`), `404`, `409` (`version_conflict`, `comment_removed`), `422` (`expected_version`, `reason`, `body`), `503` | [request](fixtures/moderation.decision.request.json), [response](fixtures/moderation.decision.response.json) |

- `action` is `approve`, `reject` or `remove`. `reason` (1–500 characters, NFC, newlines
  allowed) is **required** for `reject` and `remove` and is what the writer reads (MOD-04).
- `expected_version` is the queue item's `version`. Another decision since then is
  `409 version_conflict` with `current_version`; repeating the current review decision still commits
  an audit entry and increments `version` by 1 (`200`). A removed comment accepts no decision
  (`409 comment_removed`): removal is terminal.
- `can_decide` is `true` for every item returned by the queue, because out-of-scope works and
  recused comments (comments written by the moderator or on works they own) are filtered before
  `pending_total` and pagination. A cursor sets the starting position in the current authorized
  scope. `work_title` is the current public snapshot's title, or `null`
  when the work is not publicly served (private, author-hidden, or review-withdrawn).
- Every decision is recorded append-only with the moderator, the verified authority provenance
  (`legacy_grant` with `grant_id`, `governance_grant` with `governance_grant_id`, or `admin`), and
  the reason. **Not provided:** reports, comment cases/appeals, hide/restore, reopening a removed
  comment, queue age alerts.

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
  *(R3 update #305/#345: runtime above remains unchanged today; the proposed public paginated
  “Thấm” list for works is specified in [notifications.md](notifications.md) and
  `LVE-PR-02-03-CHOICE-12@r2` (`PENDING`), while comment “Hữu ích” stays excluded from Epic #305.)*

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

## Sign-in (D-05)

The App facade for native email-code and password authentication is governed by
[D-05](../../../../../docs/architecture/auth-authority.md), which is Provisional. In the production
profile, sign-in, registration, password enrollment and first-email attachment answer `501 authority_not_configured`
until `VINK_AUTH_SIGN_IN=enabled`. The local profile defaults to enabled. Session reads and
logout work independently of that switch. Identity queues codes on `202` and delivers them
asynchronously. [Web `/toi`](../../../../../docs/development/auth-web.md) uses the password routes
with a separate email-code option.
Every answer is `Cache-Control: no-store`. Unsafe methods
need the same-origin `Origin` like every other route. Bodies never contain the challenge handle or
the session credential: they travel only in `__Host-` cookies (`Path=/; Secure; HttpOnly`).

| Method and path | Success | Errors | Fixture |
| --- | --- | --- | --- |
| `POST /api/v1/auth/sign-in/challenges` `{email}` | `202 {expires_at}` + `Set-Cookie: __Host-vink_sign_in=…; SameSite=Strict` | `403`, `422` (`field: email`), `429 rate_limited` (+ `Retry-After` when the App's network window refused), `501 authority_not_configured`, `503` | [request](fixtures/auth.challenge.request.json), [response](fixtures/auth.challenge.response.json) |
| `POST /api/v1/auth/sign-in/complete` `{code}` + challenge cookie | `200 {state}` (`needs_principal` or `active` + `principal_id`) + `Set-Cookie: __Host-vink_session=…; SameSite=Lax`, challenge cookie cleared | `403`, `422 validation_failed` (`field: code`), `422 code_rejected`, `409 challenge_closed`, `403 account_disabled`, `403 principal_suspended`, `429 rate_limited` + `Retry-After`, `501`, `503` | [request](fixtures/auth.complete.request.json), [needs principal](fixtures/auth.session.needs_principal.json), [active](fixtures/auth.session.active.json) |
| `POST /api/v1/auth/register` or `/api/v1/auth/password/register` `{username, password}` | `200 {state: "needs_principal"}` + session cookie | `403`, `422 validation_failed`, `409 username_taken` or `already_signed_in`, `429 rate_limited`, `501`, `503` | [request](fixtures/auth.register.request.json), [response](fixtures/auth.session.needs_principal.json), [already signed in](fixtures/error.already_signed_in.json) |
| `POST /api/v1/auth/login` or `/api/v1/auth/password/login` `{username, password}` | `200 {state}` (`needs_principal` or `active` + `principal_id`) + `Set-Cookie: __Host-vink_session=…; SameSite=Lax` | `401 invalid_credentials`, `403`, `422 validation_failed`, `429 rate_limited`, `501`, `503` | [request](fixtures/auth.login.request.json), [needs principal](fixtures/auth.session.needs_principal.json), [active](fixtures/auth.session.active.json) |
| `POST /api/v1/auth/password/enroll/challenges` with a session and empty body | `202 {expires_at}` + `Set-Cookie: __Host-vink_enroll=…; SameSite=Strict` | `401`, `403`, `409 already_enrolled`, `422`, `429`, `501`, `503` | [response](fixtures/auth.challenge.response.json), [already enrolled](fixtures/error.already_enrolled.json) |
| `POST /api/v1/auth/password/enroll` with the same session, proof cookie and `{code, username, password}` | `200 {enrolled: true}`, proof cookie cleared | `401`, `403`, `409 challenge_closed`, `username_taken` or `already_enrolled`, `422 code_rejected` or `validation_failed`, `429`, `501`, `503` | [request](fixtures/auth.enroll.request.json), [response](fixtures/auth.enroll.response.json) |
| `GET /api/v1/auth/password/enroll` with a session | `200 {enrolled}` | `401`, `403 principal_suspended` or `account_disabled`, `501`, `503` | [present](fixtures/auth.enroll.response.json), [absent](fixtures/auth.enroll.status.unenrolled.json) |
| `GET /api/v1/auth/email/attachment` | `200 {principal_id, session_id, password_account, verified_email}` (email nullable) | `401`, `403`, `501`, `503` | [verified](fixtures/auth.attachment.status.verified.json) |
| `POST /api/v1/auth/email/attachment/challenges` with `{email, password, session_id}` | `202 {challenge_id, email, expires_at}` + `Set-Cookie: __Host-vink_attach=…; SameSite=Strict` | `401`, `403`, `409 attachment_context_changed`, `already_attached` or `no_password`, `422 invalid_credentials` or `validation_failed`, `429`, `501`, `503` | [request](fixtures/auth.attachment.challenge.request.json), [response](fixtures/auth.attachment.challenge.response.json) |
| `POST /api/v1/auth/email/attachment/complete` with proof cookie and `{challenge_id, code}` | `204` only after commit; no session cookie change | `401`, `403`, `409 attachment_context_changed`, `challenge_closed`, `already_attached`, `no_password` or `email_unavailable`, `422 invalid_code` or `validation_failed`, `429`, `501`, `503` | [request](fixtures/auth.attachment.complete.request.json) |
| `GET /api/v1/auth/session` | `200 {state, principal_id?}`: `anonymous`, `needs_principal`, `suspended` or `active` | `503` | same shapes |
| `GET /api/v1/auth/session/account` | `200 {principal_id, display_name}` for the current active Vink principal, `Cache-Control: no-store` | `401` without an active principal, `503` if the account cannot be read | [account](fixtures/auth.session.account.json) |
| `POST /api/v1/auth/principal` `{pen_name}` | `201 {principal_id, created: true}`; `200 … created: false` when already bound (pen name unchanged) | `401`, `403`, `403 principal_suspended`, `422` (`field: pen_name`), `503` | [request](fixtures/auth.principal.request.json), [response](fixtures/auth.principal.response.json) |
| `DELETE /api/v1/auth/session` | `204`, session cookie cleared | `403`, `503` (cookie kept, retry) | — |

- The code may contain spaces (`"042 917"`); it must be six digits. A wrong code keeps the
  challenge cookie (`code_rejected`); a closed challenge, a disabled account or a suspended
  principal clears it.
- Completing a challenge is **authentication only**. A subject without a principal gets a session
  but no literary permissions (`needs_principal`) until `POST
  /auth/principal` creates exactly one principal for it — concurrent or repeated calls return the
  same principal. A suspended principal, or a binding lookup that cannot be decided, revokes the
  fresh session in Identity and sets no cookie.
- Logout succeeds only after Identity revoked the session (or nothing was presented). If Identity
  is down the answer is `503` and the cookie stays, so logout can be retried; meanwhile the session
  cannot be verified and grants nothing.
- **Rate limits.** Before Identity is called, the App counts the request for the client network
  (IPv4 address or IPv6 /64) in PostgreSQL, shared by every App process: 10 code requests and 30
  code answers per 15 minutes. The network is the TCP peer, or — only when the peer is in
  `VINK_TRUSTED_PROXIES` — the right-most untrusted `X-Forwarded-For` hop. A refused request gets
  `429` with `Retry-After` and never reaches Identity; a counter that cannot be reached is `503`,
  never an unlimited request. Identity's own ceilings (per address, per account, global) also answer
  `429`, without `Retry-After`. Every limit answers the same for existing and new addresses.
- A successful completion revokes the session the browser presented before (if any), so signing in
  again never leaves an orphaned session behind.
- Registration is for guests. An active session supplied as a cookie or Bearer receives
  `409 already_signed_in`. The refusal creates no account and sends no `Set-Cookie`.
- Enrollment accepts `needs_principal` and active accounts with a verified email. App suspension
  yields `403 principal_suspended`. Identity accepts the mailed code only on the session that
  requested it and keeps the session cookie and principal binding intact. `code_rejected` and
  `username_taken` keep the proof cookie for another attempt. A missing or invalid proof cookie
  returns `challenge_closed`. `challenge_closed` and `already_enrolled` clear the proof cookie.
  On a lost response, use the GET readback. It reports the
  signed-in account's state at the time of the read.
- Enrollment counts five challenge requests and 15 completion requests per client network per
  15 minutes. Identity applies its own mail ceilings and attempt limits. Auth request bodies are
  bounded at 32 KiB. Password fields may contain at most 4096 bytes before NFKC normalization.
- First-email attachment requires the browser session cookie and an active Vink principal;
  there is no Bearer alternative. `session_id` and `challenge_id` are correlation values, not
  credentials. A status response is private and bound to the App principal and Identity session
  named in that response. Issuance reauthenticates with the current password, checks the
  expected session and returns the canonical target email. `202` accepts the request; it
  does not prove mail delivery. Status obtains the real binding from Identity, never from
  cookie or form data. On an uncertain completion, use status readback for the same principal,
  session and canonical address; do not replay the write automatically.
- The App refuses a non-matching or absent proof cookie before forwarding an OTP to Identity.
  Completion never sets or clears any cookie; a stale reply cannot erase newer proof state.
  Proof expires naturally and cannot be used after commit even if its cookie remains. Identity
  rejects cross-session and expired proof. An occupied email cannot be taken from its owner;
  a failed attachment supplies no owner information and does not merge or replace accounts.
  App budgets are 5 issuance attempts and 15 completions per client network per 15 minutes.
- End-user email change remains in the Identity contract. Profile and pen-name edits use the
  [App Profile routes](social-graph.md#hồ-sơ-và-bút-danh-292293).

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
| 429 | `rate_limited` | A sign-in ceiling refused the request (this network, or Identity's per-address/account/global ceiling); `Retry-After` when known | [error.rate_limited.json](fixtures/error.rate_limited.json) |
| 422 | `code_rejected` | Wrong sign-in code; `field` is `code`; retry allowed | [error.code_rejected.json](fixtures/error.code_rejected.json) |
| 409 | `challenge_closed` | Sign-in code expired, used, exhausted or no challenge cookie | [error.challenge_closed.json](fixtures/error.challenge_closed.json) |
| 403 | `account_disabled` | The mailbox was proven but the Identity account is disabled | [error.account_disabled.json](fixtures/error.account_disabled.json) |
| 403 | `principal_suspended` | The Vink principal is suspended (sign-in, provisioning, and any protected write — draft create/save, draft tags, comment submission/removal, comments mode, moderation decision, endorsement — that commits after the suspension; a suspended writer's exact retry is refused too) | [error.principal_suspended.json](fixtures/error.principal_suspended.json) |
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

### Human reaffirmation và hậu kiểm operator

Approve/reject tại expected moderation version hiện hành luôn tăng version và ghi audit, kể cả
trạng thái đã đúng. Retry expected version cũ trả `409` theo contract xung đột; không chấp nhận nó
như intent mới. Gỡ đã gỡ vẫn idempotent. Operator hậu kiểm không có HTTP route hay DTO mới: hold
trả comment về `pending`, own overlay/moderator queue và public/endorsement reads áp cùng quyền
hiện có. Internal reason/digest/policy/audit không ra public DTO. Xem
[runbook](../../../../../docs/development/discussion-post-review.md).

### Phản đối quyết định tác phẩm (MOD-10, ADM-04)

Mọi response `no-store`; Origin và principal do server xác thực như lệnh ca duyệt.

| Method / path | Quyền và dữ liệu |
| --- | --- |
| `POST /works/{work_id}/revisions/{revision_id}/appeals` | Tác giả; `{"expected_decision_version": N}`. Trả hồ sơ `200` cùng `replayed`; một hồ sơ cho mỗi phiên bản quyết định, kể cả gửi lại không cùng key. |
| `GET /works/{work_id}/appeals` | Tác giả xem lịch sử của mình, không có ghi chú nội bộ hoặc danh tính người nhận. |
| `GET /moderation/appeals` | Admin; chỉ hồ sơ mở, tự né bài của mình. |
| `GET /moderation/appeals/{id}` | Admin không xung đột; snapshot chỉ trả khi hồ sơ mở và tác phẩm chưa chuyển riêng tư. |
| `POST /moderation/appeals/{id}/conclude` | Admin; `{"expected_decision_version": N, "verdict": "uphold" hoặc "reverse", "reason": "…"}`. Kết luận và quyết định mới cùng commit; đọc lại hồ sơ khi mất ACK. |

Hồ sơ: `id`, `work_id`, `revision_id`, `decision_version`, `state` (`open`, `upheld`, `reversed`, `closed`),
`notice: "not_sent"`, `resolution_reason`, `created_at`. Chưa gửi mail/thông báo. Một phản đối mở
buộc người duyệt giải quyết qua luồng phản đối; lệnh kết luận ca thường trả `409 appeal_open`.
`409 stale_decision`, `final_decision`, `nothing_to_appeal`, `withdrawn`, `already_resolved`;
`403 recused`/`not_admin`; `422` khi thiếu lý do/phiên bản; `404` đối tượng không thuộc tác giả.
Chuyển riêng tư đóng hồ sơ cùng ca/job. Kết luận bản cũ không đổi ý định công khai bản mới.

A receipt is history of one command, never the work's state: after every command the client reads
`…/publication/state` again. Owner state preserves the complete oldest-first history (ties by
revision ID), including legacy null metadata bindings. It reads stored content-hash bindings and
validated publication/moderation metadata without loading historical bodies; it is not a body
integrity audit or approval capability. Commands revalidate full content and current authority;
public body reads validate the exact pointed content. No response field or fixture changes in #230.

Web consumer: the desk's publication region
([publication-author-web](../../../../../docs/development/publication-author-web.md)); it decodes the
fixtures above on its side (`contract/publication.rs`).

Owner publication state includes `can_appeal` and `decision_reason` per revision. These are display facts; the appeal command rechecks owner, standing, audience, provenance and decision version at commit. Case queue rows include the submitted `title`; case detail includes `viewer_is_admin` for rendering controls.


## Author work summaries (#214)

`GET /api/v1/author/works?q=&sort=recent|title&limit=1..100&cursor=…` là read-model riêng tư
của session hiện hành; query/parser/cursor kế thừa `/drafts` (owner/query/sort/limit scoped),
`Cache-Control: no-store` cả lỗi. Không nhận owner ID từ client. `/drafts` cũ không đổi.
Response `works` + `next_cursor` (null ở cuối); [fixture](fixtures/author.works.page.json).
Mỗi work có title, draft_version nullable, updated_at và publication summary nullable (unknown).
Summary giữ audience/lifecycle/generation, intent/latest revision + moderation,
review_withdrawn, has_draft_changes nullable và current_public nullable.
Current public có revision_id/title và hai số `tham`, `comments` nullable: zero thật khác
unavailable/null. Không phải tổng Thấm lịch sử, không Hữu ích comment, không manuscript/voter IDs.
Hai truy vấn batch cùng read snapshot; metric failure có thể giữ title/status với counters null.
Chi tiết semantics, fallback work không draft và giới hạn: [registry](../../../../../docs/development/desk-author-feedback.md).

### Lượt xem của tác giả (#342, VIEW-07..09, VIEW-12)

Mỗi work có thêm `views` (luôn có, đủ ba key), ở cấp work chứ không trong `current_public`: số
theo `WorkId`, đọc được cả khi bài riêng tư, bị ẩn hay bị rút sau duyệt và khi `publication` là
null. Không có ở API, HTML hay cache công khai nào; route giữ `no-store`, không validator.

| `state` | `count` | `counted_since` | Nghĩa |
| --- | --- | --- | --- |
| `counted` | số nguyên ≥ 0 | RFC 3339 hoặc null | Số đo được. `0` chỉ ở đây. `counted_since` là mốc bắt đầu đếm khi bài có thể đã công khai trước mốc (“Tính từ dd/mm/yyyy”, ngày theo `Asia/Ho_Chi_Minh`); null khi bài công khai lần đầu từ mốc trở đi. |
| `not_started` | null | null | Chưa từng có sự kiện nào được quyết định trên hệ thống: chưa đo, không phải 0. |
| `never_public` | null | null | Bài chưa từng công khai nên không thể có lượt: không phải 0. |
| `unavailable` | null | null | Không đọc được số (lỗi nguồn số liệu hoặc dữ liệu không nhất quán). Tên, trạng thái và Thấm/Nhận xét vẫn có thể có. |

Consumer coi `state` lạ là `unavailable`. Ví dụ: [sau mốc đếm](fixtures/author.works.views.json)
(“Tính từ”, 0 thật, bài riêng tư và bài bị rút giữ số cũ, bài chưa công khai, lỗi đọc số) và
[trước mốc đếm](fixtures/author.works.views-not-started.json). Số đọc trong cùng snapshot với
trang bằng một câu lệnh batch trong savepoint riêng. Luật đọc và bằng chứng:
[registry Lượt xem §9](../../../../../docs/development/work-views.md#9-đọc-theo-quyền-342).
