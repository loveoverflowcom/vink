# Bàn viết — read-model tác giả và Thấm/Nhận xét (#214)

Lựa chọn read-model được đưa vào [VDR-AUTHOR-WORK-FEEDBACK@r2](../../decisions/proposals/VDR-AUTHOR-WORK-FEEDBACK.r2.toml), **PENDING**. Thấm theo revision; Nhận xét đếm public thread, không reset theo revision. Không chốt POLICY-01 B/C hoặc O-07.

Một lát dọc `persistence-api` + `web`, sau #213 tại base
`b28e4bdd42eb84c1f45a7ebff4b983cfbc9f9567`. Giữ nguyên API nháp cũ; không thêm tracking,
xếp hạng, native, danh sách người Thấm, kế thừa vote hoặc policy tích lũy.
[LayoutSpec](../design/desk-author-feedback.md) sở hữu bố cục. [Delta R4.4](../design/redesign-r4/desk-moderation.md) bổ sung chi tiết kiểm duyệt mở theo nhu cầu.

## Quyết định và phạm vi

Tra CLI tại checkout: snapshot
`cef5662c19d3db1219bab14d197b5413f203aa816c99eaa11e8f2c31688091c2`.
`LVE-PR-01-CHOICE-02@r1` (`DESIGN_PROPOSED`, import `73ab92f`, payload
`766485bf6d2c6eabc3292921fc478d01a13376b46bed36bee9a00150a6ad6a03`) neo vào
`RevisionId` đang công khai; cùng revision đăng lại giữ số, revision mới có số riêng.
`O-07@r1` (`OPEN`, import `af3c34b`, payload
`321a00230042fb590ffaca83cefdb1360fef91a9a3564223c050d2221e8d5e3c`) chưa chốt
hệ quả qua bản sửa/ranking. PR này áp dụng baseline A của issue, POLICY-01 B/C vẫn OPEN;
không ghi signoff hay nâng architecture thành Accepted. Product/architecture, implementation
và bằng chứng chạy là bốn chiều riêng.
Publication lookup cùng snapshot: `USER-02@r1`, `ADM-06/07/08@r1` đều
`PRODUCT_CONFIRMED` (import `af3c34b`); `O-17@r2` `PRODUCT_CONFIRMED`, từ proposal
`decisions/proposals/O-17.r2.toml` và signoff tương ứng. Consumer giữ audience, lifecycle, intent/moderation và current pointer
riêng; không suy runtime cho các cơ chế chỉ mới có core hoặc proposal.

CHOICE-01 B: schema cho phép `works` không có `working_drafts` (không có FK ngược).
`GET /api/v1/author/works` trả một row/owned WorkId, kể cả không có draft. Tên lấy draft,
hoặc snapshot mới nhất, hoặc chưa đặt tên; thời điểm lấy draft, fallback ngày tạo work.
Không có draft thì không có `draft_version`/link sửa. API `/drafts` cũ giữ semantics/fixtures.
Cùng `DraftSearch` chuẩn hóa query, sort, owner-bound cursor, limit 1..100, `limit+1`;
namespace cursor riêng ngăn dùng nhầm cursor `/drafts` sang corpus tác phẩm và ngược lại;
Web dùng lại `DraftPages`/`ReadOwner`, 24/page, 240/part, không cache/coordinator thứ hai.

CHOICE-02 B: tối đa hai SELECT (aggregate chỉ khi có public anchor) trong cùng transaction `REPEATABLE READ, READ ONLY`:

1. Owner/query/order/keyset → page CTE MATERIALIZED → publication axes, intent moderation,
   latest moderation, current public anchor/title và diff nội dung/metadata; không body ra port.
2. Batch anchors của trang → hai aggregate độc lập (selected work endorsements của đúng
   revision; approved/present comments theo thread). Không fan-out, không loop query theo card.

Metrics nằm trong savepoint: lỗi aggregate trả `tham/comments: null`, vẫn giữ title/state/link
editor. Từ #342, SELECT batch thứ ba đọc Lượt xem theo `WorkId` cho mọi thẻ của trang (không cần
public anchor) trong savepoint riêng; luật và bằng chứng ở
[registry Lượt xem §9](work-views.md#9-đọc-theo-quyền-342). Lỗi toàn page vẫn là lỗi đọc; không biến thành empty/zero. `None` publication khi paired
pointer lệch public projection là unknown. Không lộ manuscript, actor/voter ID hay toàn revisions.
Snapshot chỉ nhất quán trong một response; nhiều trang là live keyset, refresh lấy lại head.

`has_draft_changes`: so title và document JSON thật + ordered `(slot, tag_id)` với metadata
snapshot gắn revision. No-op save/version tăng không tạo marker. Legacy thiếu metadata có
kết quả unknown khi nội dung không khác; không đoán từ việc draft tồn tại hay so version với ID.

Nhận xét đếm root + flat reply `review='approved' AND removal IS NULL`, giống actor-free public
projection; reply dưới root ẩn vẫn hiện nên vẫn đếm. Anchor/placeholder không phải comment công
khai. Không tính own pending/rejected hoặc Hữu ích comment; đóng nhận xét không làm mất count.
Hai aggregate độc lập; loaded page length không phải tổng.

Owner chỉ từ Actor/session; cursor là vị trí, không quyền. Route nằm trong no-store layer cả lỗi.
Browser chỉ tải sau hydration; không serialize author facts trong SSR, không public/IndexedDB
cache. ReadOwner scope/generation chống late reply/ABA, xóa khi 401/403/đổi account/logout.
Same-account retained snapshot báo stale; reconnect không replay create/publish/endorsement.
Tên public khác tên draft có context rõ. [R4 #410](https://github.com/loveoverflowcom/vink/issues/410)
hiển thị mỗi tác phẩm trên thẻ riêng, chuyển các trạng thái thành tối đa ba nhãn có icon
và dùng icon kèm số cho Thấm/Nhận xét. Số chưa tải được có dấu “—” kèm tên chỉ số.
Trước delta 25/11/2026, #410 có liên kết “Đọc” mở bản công khai bên cạnh liên kết editor.

**Delta SSOT 25/11/2026 (`#323`/`#340` `D-01`/`D-02`, triển khai UI ở `#514`):**
- `D-01`: hàng metric cố định thứ tự **Thấm → Nhận xét → Lượt xem** (Lượt xem để cuối); giữ nguyên
  luật quyền/ẩn khi bài riêng tư (`OQ-1 = A` — [Lượt xem](work-views.md)).
- `D-02`: bỏ hẳn nút/link “Đọc” / “Đọc bản công khai” trên thẻ tác phẩm Bàn viết (không thay bằng
  icon, menu, CTA khác, click số liệu hay vùng giữ chỗ; giữ nguyên liên kết mở bản thảo/editor của
  hàng/nhan đề và giữ nguyên Reader công khai). Giao diện hiện hành triển khai theo
  [LayoutSpec Lượt xem r3](../design/work-views.md) (đồng bộ R4.3/R4.4 và `docs/design/desk-author-feedback.md`).

### Ngữ cảnh kiểm duyệt (#614)

`has_review_context` là dữ kiện riêng trong publication summary. SELECT đầu kiểm intent, pointer
và revision bị rút, cùng `EXISTS` theo khóa `(work_id, revision_id)` của `work_revision_moderation`
trên các WorkId đã phân trang. Một bản chụp chưa gửi không có moderation row. Bản chụp mới hơn
không che moderation row của lần gửi cũ. API trả cờ cho tác giả trong cùng snapshot no-store.
Web chỉ ẩn lối vào khi provider trả `false`. Summary null hoặc field vắng giữ khả năng mở.
Review detail vẫn được đọc theo yêu cầu và xác minh quyền riêng cho đích hiện hành.

## Owners và kiểm chứng

- Application/port: `application/author_works.rs`, `ports::AuthorWorkStore`.
- SQL: `persistence/author_works.rs`; `author_page_sql` và `AUTHOR_METRICS_SQL` là SQL production
  được EXPLAIN trực tiếp; `AUTHOR_VIEWS_SQL` (#342) thuộc [Lượt xem](work-views.md#9-đọc-theo-quyền-342). Không thêm migration/index khi chưa có bằng chứng cần.
- HTTP provider: `http/author_works.rs`; fixture `author.works.page.json`.
- Web: `contract/author_works.rs`, `ui::adapters::author_work_summary`, `DeskIndex`,
  DTO-free `AuthorWorkRow`. Enum tương lai giữ Unknown, metric thiếu/âm không thành zero.
- PostgreSQL: `tests/postgres/author_works.rs` (corpus, actual public projection, read race,
  aggregate failure, PostgreSQL server statement log và EXPLAIN small/large).
- Browser: `authorWorks` trong `main`, phase focused `author-works`; paging main và auth
  đổi-account giữ gate ở endpoint thật mới. Fixture/race/geometry được ghi riêng với API thật.

## Đo PostgreSQL trên source triển khai

Source SQL `3e10fdd` (giữ nguyên tại `5787cd1`), test/measurement `5787cd1`, macOS aarch64, PostgreSQL 17.11 Homebrew;
scratch riêng localhost:55514, không database phát triển/production.
Focused `cargo test --locked -p vink-app --test postgres author_work -- --include-ignored --nocapture --test-threads=1`
(với `DATABASE_URL=<scratch>`, `VINK_PG_QUERY_LOG=<server-log>`) **PASS 5/5**.
Đo statement đọc từ log server PostgreSQL, không suy từ vòng lặp Rust: page size 1/24/100
đều 1 SELECT page + 1 SELECT metrics, với số public anchors cũng tăng theo page size. BEGIN/SET/savepoint/commit là control roundtrips cố định.
Measurement được ignore mặc định vì cần superuser local `SET log_statement` + đường log server;
regression metric failure không phụ thuộc logging. Measurement chạy tuần tự trên cluster scratch
riêng/quiescent để các log statement khác không xen giữa hai marker.

EXPLAIN SQL production trên corpus owner 100 và 10.001, ties cùng timestamp, 10.000 comments dồn trên một public work,
page 24 + sentinel. Recent/Title owner lớn 6,594 / 7,263 ms; nhỏ 0,841 / 0,756 ms;
aggregate một anchor/10.000 comments 4,985 ms. Đây là một lần đo trên máy local, không SLO.
Owner filtering/key sort vẫn có scan/sort cả corpus owner; counts vẫn đọc các selected rows và
public comments thực tế. Không tuyên bố database scan bounded bằng page size hoặc cần cache/index
mới từ một mẫu này. Hai aggregate group riêng, không nhân số theo fan-out.

## Còn mở

- Issue #214 giữ mở cho AC-09/10: browser lane `main` từng dừng ở lỗi caret editor có sẵn
  (viewport 390×420); OS IME, screen reader và chạm thật `NOT_RUN`.
- `POLICY-01` B/C và `O-07` vẫn `OPEN`; không suy đã triển khai.
