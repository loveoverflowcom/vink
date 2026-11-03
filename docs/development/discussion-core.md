# Bình luận hai tầng — core, application, PostgreSQL và HTTP (D2T-01, D2T-02a/b)

**Owner:** Vink, module `discussion` trong `vink-app` — quyết định của chủ sản phẩm ngày
03/11/2026 ([ADR ownership](../architecture/discussion-ownership.md)). Discourse không chạy trong
hệ thống. **Lát cắt 1** (#49, nền `develop@8045d45`) thêm core cấu trúc và replay Quint.
**Lát D2T-02a** (nền `develop@31a62b1`) thêm domain vòng đời, use case application, migration
và adapter PostgreSQL. **D2T-02b** thêm HTTP API v1 + contract fixtures
([contract](../../services/vink-app/contracts/http/v1/README.md#discussion-d2t-02b)): đọc, gửi, gỡ
của chính mình, bật/tắt; không có route kiểm duyệt. Production chưa có auth authority nên lệnh
ghi trả `401`. **D2T-02c** thêm authority kiểm duyệt: grant trong database do lệnh vận hành
`vink-app moderator` cấp/thu hồi, kiểm lại trong transaction của từng quyết định, tự né bình luận
của mình và tác phẩm của mình, lý do bắt buộc khi từ chối/gỡ (người viết thấy lý do từ chối),
hàng đợi + route kiểm duyệt. Chưa có UI. Issue: [#41](https://github.com/loveoverflowcom/vink/issues/41),
[#43](https://github.com/loveoverflowcom/vink/issues/43).

## HOW: từ lệnh tới hàng trong database

```text
Vink::submit_comment(actor, {work, target?, body, command_key})       application/discussion.rs
  actor bắt buộc → CommandKey::parse → CommentBody::parse (NFC)       domain/discussion/{submission,body}.rs
  PayloadDigest::of_submission; CommentId/ThreadId do server sinh
  → PgStore::submit_comment (một transaction)                          persistence/discussion.rs
      publications FOR SHARE → discussion_threads (tạo nếu thiếu) FOR SHARE
      → claim receipt (ON CONFLICT DO NOTHING) → target FOR SHARE
      → decide_submission(facts)  ← quyết định thuần duy nhất
      → INSERT comments (pending) + receipt, COMMIT; mọi nhánh khác ROLLBACK
```

Database tự giữ hai tầng, không phụ thuộc Rust (migration `20261103000000_discussion.sql`):
FK ghép `(root_id, thread_id, root_kind)` → `(id, thread_id, kind)` buộc root là **root cùng
thread**; FK `(reply_to_id, root_id, thread_id)` buộc context là **reply cùng root**, xóa context
chỉ `SET NULL (reply_to_id)` (không cascade); `replied_to_reply` bất biến phân biệt context đã bị
xóa với trả lời trực tiếp root. Trigger chặn đổi placement/author/thread, giữ gỡ là terminal và
`moderation_version` không giảm. Audit kiểm duyệt append-only.

**Race đã xử lý:** tác giả tắt bình luận (UPDATE hàng thread) và người đọc gửi bình luận (FOR SHARE
cùng hàng) tuần tự hóa: tắt commit trước thì lệnh gửi thấy `Closed`; ngược lại bình luận đã tồn
tại. Tám lần gửi đồng thời cùng khóa idempotency chỉ tạo một bình luận; các lần còn lại replay.

**Chưa xử lý:** chưa có lệnh rút bài (test xóa con trỏ publication để mô phỏng); thông báo, outbox,
projection/cache, reaction, ranking, sticker và UI ack chưa tồn tại.

## Lựa chọn trong lát này

- **Tiền kiểm:** bình luận mới luôn `pending` (COM-02, D-08). Không có sửa bình luận (D-08 chặn sửa
  công khai cho tới khi có gate). Quyết định duyệt/từ chối/gỡ gắn `ModerationVersion`.
- **Hai trục:** `removal` (null/author/moderation) tách `review` (pending/approved/rejected). Chỉ
  present + approved là công khai; người viết thấy trạng thái bình luận của mình.
- **Gỡ:** người viết gỡ của mình → xóa chữ khỏi database. Kiểm duyệt gỡ → giữ chữ cho audit, không
  đường đọc nào trả về. Root bị ẩn còn reply công khai → điểm neo không chữ/danh tính.
- **Tác giả tác phẩm:** chỉ bật/tắt (`CommentsMode`, `SettingsGeneration`). Gỡ bình luận người
  khác: `NotCommentWriter` khi bình luận đang công khai, `NotFound` khi bị ẩn; không có grant kiểm
  duyệt qua quyền sở hữu tác phẩm.
- **Tắt:** ngừng nhận root/reply mới, không ẩn bình luận đã duyệt; tác phẩm mới mặc định bật
  (chủ sản phẩm xác nhận 03/11/2026).
- **Phân trang:** root và reply cùng thứ tự `created_at ASC, id ASC`; cursor `c1.<micros>.<uuid>`;
  page 1..50 (mặc định 20); preview 3 reply mỗi root với `preview_next` để đọc tiếp. Một trang dùng
  snapshot `REPEATABLE READ READ ONLY` để đếm, root và preview khớp nhau.
- **Idempotency:** khóa `[A-Za-z0-9_-]{8,128}`, phạm vi (actor, operation). Digest SHA-256 của
  work + target + body đã NFC. Replay trả ID + trạng thái của chính người viết, không trả chữ.
  Retention của receipt chưa có.

## Registry traceability của #41

Đây là bảng mapping duy nhất cho D2T-INV trong repo. Requirements/ADR sở hữu quyết định; model
Quint sở hữu transition cấu trúc đã chọn; Rust hiện thực; migration giữ ràng buộc độc lập;
fixture sinh từ model là oracle để replay input qua Rust, không là implementation thứ hai.

| ID / nguồn | Model Quint | Rust production / test | PostgreSQL (`tests/postgres/discussion.rs`) | Kani | Gap còn lại |
| --- | --- | --- | --- | --- | --- |
| D2T-INV-01 — #41 | `structure.apply`, `rootIsRoot`/`canonicalContext`/`noCycles` | `reply_placement`, `root_placement`, `DiscussionThread`; enumeration 20^4 và trace replay | `the_database_alone_rejects_a_third_tier_and_cross_scope_references` | NOT_RUN | Model cấu trúc không có removal; neo/context ẩn (INV-05) chưa có model |
| D2T-INV-02 — #41 | Một thread là assumption | `decide_submission` (target tra trong thread), `CommentPlacement::from_stored` | Cùng test: cross-thread root, context khác root, đổi thread bị chặn | NOT_RUN | Chưa tenant; work binding qua `discussion_threads.work_id` unique |
| D2T-INV-03 — COM-03, COM-07, INV-05 | `lifecycle.setMode`/`submit`, `modeAuthority`, `premoderatedSubmission`, `scopedWrites`; trace `comments_mode` | `authorize_mode_change`, `DiscussionSettings::change_mode`, actor bắt buộc ở application | `closing_waits_for_an_in_flight_submission_and_vice_versa`, `the_work_author_toggles_comments_but_has_no_power_over_others`, `a_withdrawn_work_closes_its_discussion…` | NOT_RUN | Auth authority thật (D-05), ban/suspension, lệnh rút bài thật |
| D2T-INV-04 — COM-02, MOD-02/04 | `viewerProjection`, `premoderatedSubmission`; replay so `view` cho 4 người xem | `lifecycle`, `visibility::view`; lý do chỉ cho `OwnRejected` | `a_new_comment_is_private_until_approved…`, `sql_visibility_filters_match_the_domain_projection`, `rejection_reasons_reach_only_their_writer…` | NOT_RUN | Cache/SSR/notification chưa tồn tại; sửa bình luận chặn theo D-08 |
| D2T-INV-05 — PUB-03 | Một phần: `removeOwn`, gỡ là terminal trong `versionedLifecycle`; không có neo/context | `remove_by_author`, `root_listing`, `reply_context` | `hidden_roots_anchor_public_replies_and_hidden_contexts_degrade`; SET NULL context ở test toàn vẹn | NOT_RUN | Purge/retention, restore, quote/snippet |
| D2T-INV-06 — COM-01/04 | Chưa có | `CommandKey`, `PayloadDigest`, `decide_submission` (receipt trước mọi gate) | `concurrent_retries_with_one_key_create_exactly_one_comment`, idempotency trong test pending | NOT_RUN | Retention receipt, endpoint trạng thái lệnh |
| D2T-INV-07 — MOD-02, INV-07, COM-08 | `lifecycle.moderate`, `writeAuthority`, `versionedLifecycle`; trace `moderation_authority`; counterexample `author-recusal` | `ModerationAction::apply`, `authorize_moderation` (grant + tự né), `ModerationVersion` | `moderation_is_version_bound_audited…`, `moderation_authority_is_a_revocable_grant_rechecked_at_commit` (thu hồi đang chạy vs quyết định đang chạy), `moderators_recuse_from_their_own_comments_and_their_own_works`, `the_http_moderation_api_keeps_every_outcome_distinct` | NOT_RUN | Worker/job fencing, appeal/mở lại, cảnh báo backlog |
| D2T-INV-08 — COM-06 | Chưa có | Chưa có reaction | — | NOT_RUN | Desired-state set/remove, cardinality |
| D2T-INV-09 — #41 | Chưa có | `PageCursor`, `PageLimit`, `PREVIEW_REPLIES` | `pages_follow_the_keyset_order_and_previews_continue_on_the_reply_page` (ties theo ID) | NOT_RUN | Ranking “Nổi bật”, epoch/snapshot, benchmark |
| D2T-INV-10 — #45 | Chưa có | Không đổi client | — | NOT_RUN | Ack theo actor/session/draft ở consumer |
| D2T-INV-11 — #25 | [EMO lifecycle owner](expressions.md#registry-emo--một-model-lifecycle), không model comment | `CommentBody` giữ Unicode; `vink-expressions` sở hữu manifest/policy riêng, Web pilot dùng thật | — | Owner-local EMO harnesses, xem evidence PR | Chưa typed sticker/reaction persistence và composer #43/#45; không nhận proof EMO là proof discussion |

Các symbol model, fixture và lệnh thực thi của lát cắt 1 được chỉ mục ở
[owner-local behavior](../../services/vink-app/behavior/discussion/README.md).
Các hàng “Chưa có” vẫn là gap mở, không phải invariant đã PASS nhờ default state rỗng.

## Kiểm chứng theo lớp

Lệnh được sở hữu tại [development guide](README.md), runner ở `xtask`; CI gọi cùng lệnh.

1. **Domain thuần (`quick`):** unit tests cho từng module, gồm duyệt mọi chuỗi 4 lệnh của vòng đời,
   bảng quyết định đầy đủ của `decide_submission` theo receipt × publication × mode × target, và
   enumeration 20^4 của cấu trúc. Đây là finite enumeration của code, không gọi là Kani.
2. **Application (`quick`):** store ghi nhận để kiểm xác thực trước validate, input thô không tới
   store, ID do server sinh và mỗi kết quả giữ mã lỗi riêng. Không chứng minh luật của store.
3. **Quint (`discussion`):** model cấu trúc (lát cắt 1) và model vòng đời `lifecycle.qnt`
   (D2T-02: tiền kiểm, gỡ, version, bật/tắt, grant tại commit, tự né). Simulation ngẫu nhiên có
   giới hạn và replay input qua các quyết định Rust production, không phải model checking; khóa
   và transaction vẫn chỉ có bằng chứng ở lane `postgres`.
4. **PostgreSQL thật (`postgres`):** ràng buộc bằng SQL trực tiếp bỏ qua Rust, race có khóa tường
   minh, idempotency đồng thời, kiểm duyệt có audit, neo/context, keyset có ties, và so sánh bộ lọc
   SQL với projection domain trên mọi trạng thái × người xem. Chạy với superuser của harness;
   ràng buộc/trigger áp cho mọi role.
5. **Kani NOT_RUN:** release 0.68.0 cần `nightly-2026-08-21`, máy giữ duy nhất Rust stable 1.99.1.
   **Apalache/model checking và browser NOT_RUN** vì chưa có lane/UI.
6. **HTTP (`quick` + `postgres`):** fixture tests hai phía (provider serialize so field-for-field,
   consumer `vink-web` decode giữ đủ field và phân loại mã lỗi), và
   `the_http_api_keeps_every_discussion_outcome_distinct` qua router thật trên PostgreSQL: thứ tự
   xác thực → Origin → body, `no-store`, 201/200 replay, 422 theo field, 403/404/409 theo mã.

Nguồn toolchain Kani: [release 0.68.0](https://github.com/model-checking/kani/releases/tag/kani-0.68.0),
[compiler pin](https://github.com/model-checking/kani/blob/kani-0.68.0/rust-toolchain.toml).
Kết quả lệnh thực tế được ghi trong PR; sự có mặt của test/CI không tự là PASS.
