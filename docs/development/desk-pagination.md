# Bàn viết: tìm toàn bộ nháp và phân trang (#201)

Owner yêu cầu: [UX-03, WR-01/03](../../prd/requirements.md),
[issue #201](https://github.com/loveoverflowcom/vink/issues/201),
[contract API](../../services/vink-app/contracts/http/v1/README.md#danh-sách-nháp-riêng-201).
Base khảo sát `ebbde221583360b3b059b00aca696ad68b0595f8`, sau #210/#200.
Registry được query lại tại `4ae44ab` bằng `cargo xtask decision query READ-02-CHOICE-01`: snapshot
`f2f2e49cf0c079e71729722841b8e2f50b5d714dc8a87f209700dd47c0e4c1a8`;
`READ-02-CHOICE-01@r1` vẫn `DESIGN_PROPOSED`, chỉ áp public reads; source
`docs/development/public-read-freshness.md:49` tại import `630e80a9`.

## Phạm vi và trạng thái

Luật sản phẩm giữ nguyên nháp riêng, create/save/version/CAS và lost-ACK recovery.
Issue cho phép chọn kỹ thuật trong scope; đây không phải signoff hoặc ADR `Accepted`.
Mức triển khai hiện tại: `core` + `persistence-api` + `web`; consumer dùng coordinator
[#212/#202](https://github.com/loveoverflowcom/vink/pull/212), merge `73fdbe0` trên develop
được tích hợp bằng `c45fee3`. UI gửi query/sort/limit/cursor rõ ràng, không lọc local. Không native/offline write,
không full-text search, không đưa nháp vào public cache.

## Lựa chọn và owners

| Choice | Chọn và lý do | Đánh đổi / kiểm |
| --- | --- | --- |
| CHOICE-01 | A: cùng `/drafts`, giữ `drafts`, thêm `next_cursor` tùy chọn; không tham số giữ Recent/100 | Fixture cũ + trang mới, cả old/new decoder; reject field lạ/lặp, cursor scope và no-store |
| CHOICE-02 | A: `working_drafts.desk_title_key` generated/stored do PostgreSQL duy trì | Rewrite/backfill bảng một lần; tính key lúc ghi, không mỗi lần đọc. Chưa thêm index riêng; số đo bên dưới |
| CHOICE-03 | B: Tải thêm, phần hiển thị tối đa 10 trang × 24; Xem phần tiếp theo khi còn cursor | [LayoutSpec](../design/desk-server-pagination.md); dùng PageChain/ReadOwner đã merge từ #202, không dựng coordinator song song |

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

## Consumer Web

[DeskIndex](../../apps/vink-web/src/pages/desk.rs) tách input đang gõ khỏi query đã submit;
scope gồm session-generation/query/sort/limit. Ticket cũ bị bỏ cả A→B→A. Mọi request đi qua
transport `no-store`; chỉ summary nằm trong memory của route, không SSR/public cache/storage.
Delta [#213](https://github.com/loveoverflowcom/vink/issues/213): head và append mỗi intent chỉ một GET
page 24; bỏ resume và Recent/1. First-use dựa trên head không lọc thành công rỗng; query miss
không suy corpus rỗng. Search giữ node cả khi empty/loading/error; create không dựa vào resume.
Không tải body theo card. Session đổi xóa dữ liệu cũ, còn lỗi tạm thời giữ kết quả cùng scope.

[DraftPages](../../apps/vink-web/src/draft_list/paging.rs) là adapter DTO cho `accept_page`,
không sở hữu transport hoặc coordinator khác. Tối đa 10 trang × 24 mục trong một phần;
“Xem phần tiếp theo” chỉ thay phần cũ sau khi trang kế tiếp được chấp nhận. Cursor/data cùng
commit, failed page không tiến cursor; duplicate/loop/oversize yêu cầu tải lại. Refresh hợp lệ
thay toàn chain và bỏ tail, giữ input node/selection; anchor theo work ID được giữ nếu còn
ở head mới. Mục di chuyển hoặc bị loại khỏi head không có snapshot/anchor xuyên mọi trang.

[LayoutSpec](../design/desk-server-pagination.md) giữ R6/Quiet Lavender và shared primitives.
Create/save/CAS/leave guard/editor không đổi; create uncertain chỉ được mở lại bởi GET thành
công và intent tạo thêm rõ ràng. Unit adapter kiểm bound, invalid page, head supersedes append
và scope/session A→B→A. Khi nút Tải thêm đang giữ focus bị thay bởi Tải lại từ đầu,
focus chuyển tới nút phục hồi sau render; không chuyển nếu người dùng đã sang ô khác.

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
`target/issue201/query-plans.log`. Nguồn backend ghim tại `832c8fbfb64bc68fe9b15e6d0c05d54e4455322a`;
SQL production đo ở đây giữ nguyên sau tích hợp #202.

## Bằng chứng local trên nguồn tích hợp

Nguồn implementation/test cuối: `4ae44ab9256c55271ab9347677db6d4a6595a344`, sau khi
merge develop có #212. Các thay đổi sau SHA này chỉ ghi tài liệu bằng chứng. CWD:
`/Users/manhblue/projects/loveoverflow/vink`; macOS aarch64, Rust 1.99.1, PostgreSQL 17.11,
Node 22.15.0, npm 10.9.2, HeadlessChrome 157.0.0.0. Hai lane có DB dùng scratch loopback
`127.0.0.1:55440`, database bootstrap `postgres`, role local `vink_review`; runner tự cấp
và thu hồi DB/role của bài kiểm, không DB phát triển hoặc production.

Lệnh chạy tuần tự, với `CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0`:

| Command / boundary | Kết quả |
| --- | --- |
| `cargo xtask verify quick` — unit/contracts, SSR/WASM lint, repository owners | PASS toàn lane; gồm 218 Web tests và các unit/contract owner khác |
| `VINK_TEST_DATABASE_URL=<scratch> cargo xtask verify postgres` — PostgreSQL thật | PASS toàn lane: store 6, App 185 (2 measurement ignored), Identity 48; provisioning/worker/binaries/sign-in/enrollment/backup-restore và cleanup |
| `VINK_TEST_DATABASE_URL=<scratch> cargo xtask verify web --scenario main --scenario auth` — browser + App/Identity thật | PASS cả hai scenario và cleanup; các scenario không chọn không được tính PASS |
| `cargo run -q -p vink-app --example draft_search_migration` rồi `cmp` migration | PASS tại `c0ab638`; generator và SQL giữ nguyên ở nguồn cuối |
| `git diff --check` và relative file links | PASS; không thay YAML/skill metadata |

Browser dùng `VINK_CHROME=/Applications/Google Chrome.app/Contents/MacOS/Google Chrome`,
`npm_config_cache=/private/tmp/vink-pr210-npm-cache`; dựng bundle từ nguồn mỗi lượt.
Logs local: `target/issue201/{quick-final,postgres-final,web}.log`;
run browser `target/verify-web/1794740135289939000-33428`, metadata/hash nguồn ở
`screenshots/contexts.json`. Corpus HTTP thật có 250 nháp, traversal Recent/Title tới hết
qua 10×24 + phần cuối 10, tìm nháp cũ ngoài 100 mà chưa tải trang trung gian. `DraftGate`
chỉ giữ/trả lỗi cho transport race; không dùng fixture đó thay bằng chứng query PostgreSQL.
Auth chạy Identity và outbox local thật; old-A response được giữ trong browser để kiểm đổi phiên.

Backend trước tích hợp đã PASS full PostgreSQL tại `832c8fb` (store 6, App 185 + 2 measurement
ignored, Identity 48, provisioning/binaries/worker/sign-in/backup-restore). Query-plan riêng
ở trên không được tính là latency benchmark hoặc SLO. Bằng chứng cuối trong bảng thay cho
việc suy PASS từ backend cũ.

### Đối chiếu acceptance #201

AC-01..09: PASS trong các boundary và giới hạn ghi dưới đây.

| AC | Bằng chứng và ranh giới |
| --- | --- |
| AC-01 | PG corpus 175 + browser/API thật 250: nháp cũ ngoài 100 vẫn tìm được ngay; query/sort trước limit ở SQL |
| AC-02 | PG traversal hai sort với timestamp/folded-title ties và UUID; Unicode exhaustive parity, NFD/Đ/case/whitespace/untitled; browser duyệt đủ 250 ID không trùng/thiếu |
| AC-03 | Domain/HTTP tests: giới hạn/version/canonical scope owner-query-sort-limit, malformed/repeated/unknown params; auth trước query, owner từ session |
| AC-04 | Fixture cũ/mới và old/new decoder; PostgreSQL HTTP/SSR/trace checks; browser nháp không vào storage/public cache; chỉ summary, no-store |
| AC-05 | Browser query A→B→A, đổi sort, account generation A→B, delayed append/retry/422; unit adapter cùng ReadOwner; loaded count và terminal riêng |
| AC-06 | PG live title/timestamp movement cần head mới; browser delayed append bị refresh supersede, tail bị bỏ; resume Recent/1 không đổi theo query/Title |
| AC-07 | Main giữ create uncertainty, save/CAS/conflict, lost ACK, leave guard/export và editor node/text/selection; reconnect không replay POST; composition/undo/redo là CDP simulation |
| AC-08 | 19 EXPLAIN shapes trên SQL production, small/large/neighbour cardinality và scan/index tradeoff ở trên; application bounded, không N+1 theo card |
| AC-09 | Final quick/PostgreSQL theo bảng; main/auth PASS; hẹp/chữ 200% và Enter phục hồi cursor thật qua consumer |

### Review UI theo năm chiều

`self-review`, [LayoutSpec r1](../design/desk-server-pagination.md); R6 là regression
reference, `fidelity: NO_APPROVED_REFERENCE`. Không thêm hay tự duyệt style anchor.
Target là Chrome/Web; text 100/200% là root font-size 16/32px, không phải browser zoom hoặc OS font scale.

| Chiều | Kết quả / target / giới hạn |
| --- | --- |
| Functional | PASS main/auth: search/sort/continuation, 250 ID, scoped races, create/editor recovery; transport-delay fixture được ghi riêng |
| Geometry | PASS continuation/restart 390/1440 × 100/200%; không overflow ngang; nút restart ≥44px và toàn bộ nằm trên bottom nav khi giữ focus; R6 matrix hiện hữu vẫn PASS |
| Visual | PASS self-review cho các ảnh đã mở bên dưới: phân cấp hàng → continuation rõ, chữ Việt đầy đủ, nền/controls giữ Quiet Lavender; không pixel fidelity claim, chưa mở ảnh dark cho state continuation mới |
| Accessibility/Responsive | PASS các target đã đo: Tab/Enter, full labels, focus phục hồi ở 390/200%; R6 text contrast light ≥5,79 / dark ≥8,16 (disabled exempt); OS screen-reader NOT_RUN, không tuyên bố audit WCAG đầy đủ |
| Interaction | PASS explicit submit, composition guard giả lập, input/caret/selection qua reconnect, giữ cursor khi lỗi và không nhận late response; OS IME NOT_RUN, CMP/native ngoài scope |

Ảnh đã mở ở **run cuối** (đều dưới `screenshots/`, build `4ae44ab`):
`desk-paging-390-200.png`, `desk-more-390-200.png`, `desk-more-1440-100.png`,
`desk-list-390-200-dark.png`, `desk-list-1440-100-light.png`. Các ảnh continuation mới dùng
system theme resolve light, viewport cao 900px; list light/dark là fixture geometry có nhãn.
Ở 200%, panel lỗi dài được cuộn dọc; focus nút thay thế làm hành động hiện đầy đủ trên thanh
điều hướng. Bản chụp tại vị trí hành động có thể nằm sau tiêu đề panel, không cắt nội dung DOM.

Một vòng sửa accessibility có render: cursor-invalid từng loại bỏ nút đang focus; đã chuyển
focus sang restart có điều kiện và chạy Enter phục hồi tại 390/200. Finding `fixed` ở nguồn
cuối; không đổi tiêu chí để đạt. Hai lỗi browser trước đó là assertion copy R6 cũ và giả định
SAMPLE_WORK luôn ở trang đầu; đã giữ kiểm editor và tìm bản mẫu bằng query thật. Logs
`web-1.log`, `web-2.log` giữ lượt thất bại, không tính PASS. Các prerequisite/lock failure trước
khi test cũng không tính PASS; không xóa hay bỏ qua artifact lock để chạy song song.

Review code cuối không còn finding có bằng chứng cần sửa trong scope. Migration stored key
cần rewrite/backfill bảng, truy vấn owner lớn có thể scan toàn bảng, live keyset không cung cấp
snapshot qua các trang; đây là giới hạn đã ghi, không là product/architecture signoff.
GITHUB_CI: `NOT_REQUIRED`, không đọc/poll Actions. OS IME và screen-reader: `NOT_RUN`.
