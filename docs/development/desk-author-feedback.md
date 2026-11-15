# Bàn viết — read-model tác giả và Thấm/Nhận xét (#214)

Một lát dọc `persistence-api` + `web`, sau #213 tại base
`b28e4bdd42eb84c1f45a7ebff4b983cfbc9f9567`. Giữ nguyên API nháp cũ; không thêm tracking,
xếp hạng, native, danh sách người Thấm, kế thừa vote hoặc policy tích lũy.
[LayoutSpec](../design/desk-author-feedback.md) sở hữu bố cục.

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

CHOICE-01 B: schema cho phép `works` không có `working_drafts` (không có FK ngược).
`GET /api/v1/author/works` trả một row/owned WorkId, kể cả không có draft. Tên lấy draft,
hoặc snapshot mới nhất, hoặc chưa đặt tên; thời điểm lấy draft, fallback ngày tạo work.
Không có draft thì không có `draft_version`/link sửa. API `/drafts` cũ giữ semantics/fixtures.
Cùng `DraftSearch` chuẩn hóa query, sort, owner-bound cursor, limit 1..100, `limit+1`;
Web dùng lại `DraftPages`/`ReadOwner`, 24/page, 240/part, không cache/coordinator thứ hai.

CHOICE-02 B: hai SELECT trong cùng transaction `REPEATABLE READ, READ ONLY`:

1. Owner/query/order/keyset → page CTE MATERIALIZED → publication axes, intent moderation,
   latest moderation, current public anchor/title và diff nội dung/metadata; không body ra port.
2. Batch anchors của trang → hai aggregate độc lập (selected work endorsements của đúng
   revision; approved/present comments theo thread). Không fan-out, không loop query theo card.

Metrics nằm trong savepoint: lỗi aggregate trả `tham/comments: null`, vẫn giữ title/state/link
editor. Lỗi toàn page vẫn là lỗi đọc; không biến thành empty/zero. `None` publication khi paired
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
Tên public khác tên draft có context rõ; count là text, link đọc là sibling của link editor.

## Owners và kiểm chứng

- Application/port: `application/author_works.rs`, `ports::AuthorWorkStore`.
- SQL: `persistence/author_works.rs`; `author_page_sql` và `AUTHOR_METRICS_SQL` là SQL production
  được EXPLAIN trực tiếp. Không thêm migration/index khi chưa có bằng chứng cần.
- HTTP provider: `http/author_works.rs`; fixture `author.works.page.json`.
- Web: `contract/author_works.rs`, `ui::adapters::author_work_summary`, `DeskIndex`,
  DTO-free `AuthorWorkRow`. Enum tương lai giữ Unknown, metric thiếu/âm không thành zero.
- PostgreSQL: `tests/postgres/author_works.rs` (corpus, actual public projection, read race,
  aggregate failure, SQL statement capture và EXPLAIN small/large).
- Browser: `authorWorks` trong `main`, phase focused `author-works`; paging main và auth
  đổi-account giữ gate ở endpoint thật mới. Fixture/race/geometry được ghi riêng với API thật.

Bằng chứng nguồn cuối, AC→command và bảng UX năm chiều được bổ sung sau khi chạy các kiểm local.
