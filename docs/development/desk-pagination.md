# Bàn viết: tìm toàn bộ nháp và phân trang (#201)

Owner yêu cầu: [UX-03, WR-01/03](../../prd/requirements.md),
[issue #201](https://github.com/loveoverflowcom/vink/issues/201),
[contract API](../../services/vink-app/contracts/http/v1/README.md#danh-sách-nháp-riêng-201).
Base khảo sát `ebbde221583360b3b059b00aca696ad68b0595f8`, sau #210/#200.
Registry được query tại base: snapshot
`f2f2e49cf0c079e71729722841b8e2f50b5d714dc8a87f209700dd47c0e4c1a8`;
`READ-02-CHOICE-01@r1` vẫn `DESIGN_PROPOSED`, chỉ áp public reads.

## Phạm vi và trạng thái

Luật sản phẩm giữ nguyên nháp riêng, create/save/version/CAS và lost-ACK recovery.
Issue cho phép chọn kỹ thuật trong scope; đây không phải signoff hoặc ADR `Accepted`.
Mức triển khai hiện tại: `core` + `persistence-api`; consumer phân trang Web đang chờ
coordinator [#202](https://github.com/loveoverflowcom/vink/issues/202). Dashboard hiện tại
vẫn dùng danh sách Recent/100 và lọc local cho tới lúc tích hợp. Không native/offline write,
không full-text search, không đưa nháp vào public cache.

## Lựa chọn và owners

| Choice | Chọn và lý do | Đánh đổi / kiểm |
| --- | --- | --- |
| CHOICE-01 | A: cùng `/drafts`, giữ `drafts`, thêm `next_cursor` tùy chọn; không tham số giữ Recent/100 | Fixture cũ + trang mới, cả old/new decoder; reject field lạ/lặp, cursor scope và no-store |
| CHOICE-02 | A: `working_drafts.desk_title_key` generated/stored do PostgreSQL duy trì | Rewrite/backfill bảng một lần; tính key lúc ghi, không mỗi lần đọc. Chưa thêm index riêng; số đo bên dưới |
| CHOICE-03 | B: Tải thêm, phần hiển thị tối đa 10 trang × 24; Xem phần tiếp theo khi còn cursor | [LayoutSpec](../design/desk-server-pagination.md); nối PageChain/ReadOwner của #202 sau merge, không dựng coordinator song song |

- Domain: [draft_query.rs](../../services/vink-app/src/domain/draft_query.rs) sở hữu giới hạn,
  gập khóa, Recent/Title và cursor v1 bound principal/query/sort/limit.
- Application: [drafts.rs](../../services/vink-app/src/application/drafts.rs) xác thực actor
  trước store; port `DraftStore::search_drafts`. `list_drafts` cũ gọi cùng use case mặc định.
- Adapter: [drafts.rs](../../services/vink-app/src/persistence/drafts.rs) chỉ một SQL statement
  lấy summary, order/filter trước limit+1. `work_id` phá hòa; Title dùng `COLLATE "C"` như
  thứ tự chuỗi Rust, không phụ thuộc locale database. Không `OFFSET` hoặc N+1 từ application.
- Migration [20261120000000](../../services/vink-app/migrations/20261120000000_draft_search.sql)
  dùng NFD + bảng translate sinh từ toolchain/`unicode-normalization` đang pin. Chín canonical
  decomposition mới hơn PostgreSQL 17 có supplement; test so mọi scalar bị biến đổi với Rust.
  Không dùng `vink_search_key` của public discovery vì nó bỏ dấu câu và tìm prefix từ.
  [Generator](../../services/vink-app/examples/draft_search_migration.rs) tái tạo SQL;
  nâng Unicode phải kiểm parity và viết migration mới, không sửa migration đã áp dụng.
- HTTP: [decoder](../../services/vink-app/src/http/drafts.rs) giới hạn URL và decode nghiêm;
  request trace chỉ ghi method/path, không query có tiêu đề/cursor. Không thay create/save.

## Query-plan experiment

PostgreSQL 17.11 Homebrew, aarch64 macOS, Rust 1.99.1; scratch loopback, không DB phát triển.
Corpus tổng hợp: owner nhỏ 150, owner lớn 20.001 (gồm seed), neighbour 80.000 nháp;
timestamps và folded titles trùng, tiêu đề rỗng; body fixture ngắn. `ANALYZE` trước đo,
page 24 + lookahead 1; query rỗng, selective `ban cu duy nhat`, nonselective `duong ve`,
không khớp `missing`, và cursor giữa corpus. Một lần đo mỗi shape, không p95 hoặc SLO.

Lệnh: `DATABASE_URL=<scratch> cargo test -p vink-app --test postgres drafts::draft_query_plans -- --ignored --nocapture`.
Flags theo máy: `CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0`.
Test EXPLAIN chính các constant SQL production với `ANALYZE, BUFFERS, COSTS OFF`.

| Corpus / shape | Execution time local |
| --- | --- |
| Nhỏ, Recent/Title với bốn query | 0,101–0,261 ms |
| Lớn, Recent/Title với bốn query | 5,091–12,449 ms |
| Lớn, cursor giữa: Recent / Title | 7,278 / 9,822 ms |
| Lớn, Title tính cùng key lúc đọc (candidate B) | 2377,484 ms |

Owner nhỏ dùng `works_owner_id_idx` → PK `working_drafts` (150 heap lookups trong **một SQL**)
→ top-N sort. Owner lớn dùng hash join + seq scan bảng 100.151 dòng, top-N sort khi phù hợp;
query không khớp vẫn scan keys toàn bảng. Đây là tradeoff còn thật, không tuyên bố scan luôn
giới hạn bằng page size. Stored key tránh chi phí gập từng title khi đọc; generated column bảo
đảm SQL writer khác không làm key cũ. Chưa có bằng chứng cần denormalize owner/order hoặc
trigram index. Response/read buffer phía application tối đa 101 summaries, không manuscript.
Plan đầy đủ: [query-plans.txt](evidence/desk-pagination/query-plans.txt); log runner ở
`target/issue201/query-plans.log`. Sẽ ghim final source/evidence sau tích hợp #202.

## Bằng chứng và phần còn lại

Lượt backend trên base + dirty inputs #201: focused unit draft 9 PASS;
focused PostgreSQL đầu tiên 4 PASS/1 measurement ignored (paging, isolation, backfill/CAS,
Unicode parity). Đã phát hiện và sửa migration version trùng trước khi commit;
supplement Unicode được thêm sau ca parity thất bại. Những lần thất bại không là PASS.
Các test trace/blank-title và checks cuối đang tiếp tục; chưa nghiệm thu AC-01..09 toàn bộ.

Unit/contract, PostgreSQL, browser và source-only được báo riêng khi hoàn tất. Cần #202 merge
vào `develop`, cập nhật nhánh từ đó, nối Web và chạy `cargo xtask verify web --scenario main`.
GITHUB_CI: `NOT_REQUIRED`, không đọc/poll Actions. OS IME và screen-reader: `NOT_RUN`.
