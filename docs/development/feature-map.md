# Bản đồ tính năng và đường kiểm chứng Vink

Bản đồ này nối hành trình người dùng với route, owner tài liệu và phép kiểm hiện có. Nó là chỉ mục
vào source và test, không phải bảng PASS. Sự tồn tại của test là ý định coverage; chỉ một lượt chạy
trên input đã xác định mới là bằng chứng. Lệnh và phạm vi chạy do
[scoped workflows](scoped-workflows.md) sở hữu; quy tắc chọn bằng chứng ở
[verification guide](../../.agents/skills/vink-engineering/references/verification.md). Lịch sử từng
PR nằm trên GitHub.

## Điều kiện để đi qua các luồng

- Dùng đúng `VINK_PUBLIC_ORIGIN` của môi trường local ([development](README.md)). POST/PUT từ origin
  khác bị từ chối; mở được trang chưa chứng minh thao tác ghi hoạt động.
- [dev_seed.rs](../../services/vink-app/src/dev_seed.rs) có tác phẩm `[Mẫu] Mưa trên phố cũ`
  (`SAMPLE_WORK = 33333333-3333-4333-8333-333333333333`). Seed chỉ nạp khi gọi rõ ràng và giữ hàng
  đã tồn tại; dùng dữ liệu riêng cho thử nghiệm ghi.
- Bàn viết cần hydration và principal phía server. `VINK_DEV_PRINCIPAL` chỉ là fixture loopback;
  [Auth Web](auth-web.md) dùng Identity thật. Không có authority thì API riêng trả `401`.
- Định tuyến Web ở [lib.rs](../../apps/vink-web/src/lib.rs), API ở
  [router](../../services/vink-app/src/http/api.rs); phương thức và lỗi do
  [contract v1](../../services/vink-app/contracts/http/v1/README.md) sở hữu.

## Hành trình trên Web

Cột “Kiểm” ghi scenario của `cargo xtask verify web --scenario <tên>` (browser), test PostgreSQL
(`services/vink-app/tests/postgres/<tên>.rs`, lane `postgres`) hoặc unit trong `verify quick`.

| Hành trình | Route / điểm vào | Owner | Kiểm |
| --- | --- | --- | --- |
| Khám phá tác phẩm theo chữ và thẻ | `/?q=…&tag=…&cursor=…`, `/the/:slug` | [discovery API](discovery-api.md), [discovery UI](discovery-ui.md), [LayoutSpec R4.3](../design/redesign-r4/layout-spec.md) | `discovery`, `reader`; PG `discovery`, `taxonomy_api` |
| Quay lại đúng vị trí kết quả | Reader → Back | [LayoutSpec #236](../design/discovery-return-position-236.md) | `return-position` |
| Khám phá tác giả, hồ sơ, Theo dõi | `/?mode=authors`, `/@handle`, `/@handle/{nguoi-theo-doi,dang-theo-doi}`, `/?mode=following` | [Social Graph](social-graph.md), [LayoutSpec](../design/social-289.md) | `author-search`; PG `author_discovery`, `follow`, `feed`, `handle` |
| Đọc tác phẩm | `/tac-pham/:id` (giữ ngữ cảnh `q/tag/cursor/from_tag`) | [public reading](../design/public-reading.md), [READ-01](public-conditional-reads.md), [READ-02](public-read-freshness.md), [SSR Reader](reader-ssr-boundary.md) | `reader`, `storage`; PG `conditional_reads` |
| Thấm / Hữu ích và danh sách Thấm | Cuối Reader, `/tac-pham/:id/tham/:revision` | [LVE runtime](literary-endorsement-runtime.md), [Pen Flight R2](../design/pen-flight-r2.md) | `endorsement`, `notifications`; PG `endorsement` |
| Nhận xét hai tầng | Reader → nhận xét gốc/phản hồi, Markdown tuỳ chọn | [discussion core](discussion-core.md), [Web reads](discussion-web-reads.md), [LayoutSpec](../design/discussion-ui.md) | `discussion`; PG `discussion`, `discussion_reads`, `outbox` |
| Lưu bài, tab Đã lưu | `#save-work` cuối Reader, `/:author/da-luu` | [registry](saved-works.md), [LayoutSpec](../design/saved-works.md) | `reader`; PG `saved_works` |
| Lượt xem (chỉ tác giả thấy) | Reader gửi ứng viên → Bàn viết | [registry](work-views.md), [LayoutSpec r3](../design/work-views.md) | `reader`, `desk`; PG `work_view*` |
| Bàn viết: tìm, sắp xếp, phân trang | `/ban-viet` | [desk pagination](desk-pagination.md), [feedback read-model](desk-author-feedback.md), [LayoutSpec R4.3](../design/redesign-r4/layout-spec.md) | `desk`; PG `author_works`, `drafts` |
| Tạo nháp khi mất phản hồi | “Tạo bản thảo” → đối soát bằng GET | [desk pagination](desk-pagination.md) | `desk`, `main` |
| Viết và lưu nháp, xung đột | `/ban-viet/:id`, `#save-slot`, `#editor-text-conflict` | [document format](document-source-format.md), [editor runtime](../../apps/vink-web/editor/README.md), [Editor R4](../design/editor-r4.md), [unified save](../design/ui-editor-unified-save-217.md) | `editor`, `editor-chrome`, `editor-format`, `editor-first-draft`; PG `drafts`, `canonical_document`, `draft_http_budget` |
| Thẻ và tuỳ chọn bài viết | `#work-options` trong editor | [taxonomy editor](taxonomy-editor.md), [LayoutSpec thẻ](../design/tag-selector.md) | `editor`; PG `taxonomy` |
| Công khai / riêng tư, gửi duyệt | Vùng “Công khai” trong `#work-options` | [author Web](publication-author-web.md), [runtime](publication-runtime.md), [LayoutSpec](../design/publication-author.md) | `publication`, `evaluator-message`; PG `publication*` |
| Trạng thái kiểm duyệt trong Bàn viết | `/ban-viet/:id/kiem-duyet` (legacy), Bàn viết R4.4 | [moderation runtime](moderation-runtime.md), [LayoutSpec R4.4](../design/redesign-r4/desk-moderation.md) | `review`, `evaluator-message`; PG `work_moderation`, `moderation_queue` |
| Trang sau kiểm duyệt / đã ẩn | URL cũ của bài | [publication core](publication-core.md), [moderation runtime](moderation-runtime.md) | `review`; PG `publication_state` |
| Rời editor khi chưa lưu | Rail, Back/Forward khi còn chữ hoặc thẻ chưa xác nhận | [Editor R4](../design/editor-r4.md) | `main`, `editor-chrome` |
| Điều hướng thích ứng | Rail 220/96px, modal tablet, thanh dưới mobile | [navigation contract](../../packages/ui-system/shared/navigation.md), [NavigationBar R1](../design/bottom-nav-r1-131.md) | `appearance`, `main` |
| Tài khoản, Giao diện, hồ sơ | `/toi`, `/toi/chinh-ho-so` | [account R5](../design/account-profile-r5.md), [Giao diện R1](../design/me-preferences-r1-209.md), [handle #480](../design/profile-inline-handle-480.md) | `appearance`, `password`, `standalone` |
| Đăng ký, đăng nhập (mật khẩu, mã email) | `/toi`, sheet đăng nhập trong Reader | [Auth Web](auth-web.md), [LayoutSpec mật khẩu](../design/auth-password-web.md), [Reader auth](../design/auth-reader-web.md) | `auth`, `password`, `standalone` |
| Xóa tài khoản | `/toi` → Xóa tài khoản | [vòng đời](../architecture/account-deletion.md), [LayoutSpec](../design/account-deletion.md) | `account-deletion`; PG `account_deletion` |
| Thông báo | Chuông, `/thong-bao`, `/thong-bao/nhan-xet/:id/:version` | [registry](notifications.md), [LayoutSpec R4](../design/notifications.md) | `notifications`; PG `notification*` |
| Làm mới và phục hồi | Loading/lỗi/thử lại trên mọi route | [product web refresh](product-web-refresh.md), [state & feedback](../design/state-and-feedback.md) | `main`, `desk`, `reader` |
| Ngày giờ theo lịch người xem | Mọi `Timestamp` | [human time](../design/human-time.md) | unit `vink-patterns` |
| Hiệu năng Khám phá → Reader → Back | Opt-in | [perf baseline](web-perf-baseline.md) | `perf-baseline` |

Thứ tự scenario, các alias (`main` gồm `editor-first-draft`, `reader`, `discussion`, `appearance`,
`desk`, `editor-chrome`, `editor`, `editor-format`, `storage`) và scenario cần Identity thật nằm ở
[plan.rs](../../xtask/src/plan.rs). Browser smoke ghi capture và context vào
`target/verify-web/<run-id>/`; ảnh chưa mở không chứng minh bố cục.

## Dịch vụ và vận hành

| Khả năng | Điểm vào | Owner | Kiểm |
| --- | --- | --- | --- |
| Health/readiness | `/health/live`, `/health/ready` | [health.rs](../../services/vink-app/src/http/health.rs) | `quick` |
| Identity riêng, phiên, đình chỉ principal | `vink-identity serve`, `vink-app principal …`, `vink-identity account …` | [auth operations](auth-operations.md), [ADR D-05](../architecture/auth-authority.md) | `postgres`, `quick` |
| PostgreSQL theo owner, backup/restore | `services/postgres/provision.sh`, `vink-app migrate` | [services/postgres](../../services/postgres/README.md), [ranh giới](../architecture/identity-postgres-boundaries.md) | `postgres` ([provision.rs](../../xtask/src/provision.rs)) |
| Quản trị kiểm duyệt, grant, ca, phản đối | `vink-app admin …`, `/api/v1/moderation/*` | [moderation runtime](moderation-runtime.md) | PG `work_moderation`, `comment_moderation_governance` |
| Hậu kiểm bình luận do operator yêu cầu | `vink-app post-review …` | [post-review](discussion-post-review.md) | PG `post_review` |
| Thẻ catalog (operator) | `vink-app tag …` | [taxonomy API](taxonomy-api.md) | PG `taxonomy_api` |
| Worker: outbox, đánh giá, retention | `vink-worker` | [outbox ADR](../architecture/discussion-outbox.md), [moderation runtime](moderation-runtime.md) | PG `outbox`, `work_moderation` |
| Notification service | `services/vink-notifications` | [registry](notifications.md) | `postgres` |
| Điểm Vink (Reputation #600) | Backend-only, `NOT_IMPLEMENTED`; sáu lát S01..S06 (#601, #644–#648) | [PRD §19](../../prd/requirements.md#19-điểm-vink-reputation-v1), [đặc tả](../architecture/reputation.md), [registry](reputation.md), [SR-01](reputation-science.md), [API dự thảo](../../services/vink-app/contracts/http/v1/reputation.md) | `decision validate`, [script SR-01](../../services/vink-app/bench/reputation-sr01/qualify.py) |
| Biểu cảm (Preview) | `/expressions.html` trên Preview host | [expressions](expressions.md) | `expressions`, `kani` |

## Phần việc còn lại của quyết định sản phẩm 10/11/2026

Nguồn và bằng chứng model ở [registry](product-decisions-registry.md).

| Hành vi đích | Yêu cầu | Hiện có | Còn thiếu |
| --- | --- | --- | --- |
| Audience, gửi duyệt, đăng đúng bản | AUD-01/02, PUB-01/02 | API tác giả, worker đánh giá (mock bootstrap #536), vùng Công khai trên Web | Evaluator AI thật |
| Trợ lý AI duyệt, admin override | MOD-07..09 | Lõi + lease/claim/retry trong worker | AI provider thật (`ai_moderation: not_implemented`) |
| Quản trị kiểm duyệt | MOD-09..15 | Ghế admin, grant/nhóm, ca, phản đối, dừng tự động qua API | Điều tra riêng, ca bình luận, UI grant/nhóm |
| Xóa bài để lại tombstone; trang đã ẩn | PUB-04/05/07, COM-11 | Trang sau kiểm duyệt và trang đã ẩn (API/SSR/Web) | Tombstone runtime, route xóa |
| Sửa bình luận, dấu “Đã sửa” | COM-10 | Model `comment_edit.qnt` | Runtime và UI |
| Bộ sưu tập cá nhân | COL-01..06 | Model `collections.qnt` | Runtime và UI |
| Hashtag | HTG-01..06 | Đặc tả | Runtime và UI |
| Đề xuất, Top 100 | RANK, TOP | Model `discovery.qnt` | Runtime; `/` vẫn sắp theo thời gian |
| Khôi phục mật khẩu | ACC-03, O-11 | Đăng ký/đăng nhập/thêm mật khẩu | Reset, đổi email qua Web, CMP auth |

## Giới hạn coverage cần giữ

- Browser lane dùng Chrome headless trên PostgreSQL scratch. Safari/Firefox, IME hệ điều hành,
  screen reader, thiết bị thật và zoom trình duyệt thật cần phiên kiểm riêng.
- CDP `Input.insertText`/`imeSetComposition` không phải IME hệ điều hành; chữ 200% thường được mô
  phỏng bằng root font-size.
- Không có idempotency key cho tạo/lưu nháp; GET đối soát không chứng minh POST nào đã ghi.
- Lane `discussion` kiểm model Quint và replay thuần, không kiểm locking, transaction hay I/O.
- Native UI (Android/iOS/Desktop) chỉ có host chẩn đoán; không có app native sản phẩm
  ([UI system](../../packages/ui-system/README.md)).
- Public IndexedDB (READ-02) nằm sau gate tắt; không có service worker hay lưu trữ riêng tư.
