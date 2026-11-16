# Ngân sách document v1 — #229

Baseline đo tài liệu tổng hợp trước khi quyết định chia nhỏ storage. Đây là evidence phát triển,
không phải SLA production hay quyết định đổi giới hạn bài viết.

<!-- vdr: active USER-09@r1 -->

- **Sản phẩm:** `USER-09@r1 PRODUCT_CONFIRMED`, bài ngắn; `WR-04 DEFERRED` trong PRD.
- **Kiến trúc:** giữ các trạng thái ADR hiện hành. JSONB/chunking và adoption LZ4 chưa được chốt.
- **Triển khai:** chỉ corpus, regression và phép đo opt-in; runtime `persistence-api`/`web` giữ nguyên.
- **Evidence:** source base `a3e65360e316d27e5d029a43aef8b3083f622fb7`; kết quả thực thi và giới hạn
  nằm bên dưới. Không dùng bằng chứng từ PR #227 đang mở làm kết quả của lần chạy này.

## Tái lập

Owner corpus: [support/document_budget.rs](../../services/vink-app/tests/support/document_budget.rs).
Seed `229`, LCG cố định, 21 case: 30k/100k/200k scalar sau NFC × ASCII lặp, tiếng Việt seeded
NFC/NFD, emoji, thơ bốn dòng/khổ có tab, nhiều block một ký tự và JSON escaping.
Tiếng Việt seeded là chuỗi ký tự tổng hợp có entropy; không đại diện phân bố văn học production.
Scalar không tính dấu xuống dòng/khổ do schema biểu diễn bằng cấu trúc. Source editor có thêm các
dấu xuống dòng đó. JSON UTF-8, JSONB/TOAST, HTML và RAM là các đại lượng riêng.

```sh
# Chọn cluster local scratch có CREATEDB; không trỏ vào database cá nhân/production.
export DATABASE_URL='postgres://localhost:55529/postgres'
export VINK_TEST_DATABASE_URL="$DATABASE_URL"
export VINK_DOCUMENT_BUDGET_OUTPUT=/tmp/vink-229-evidence/run
export VINK_CHROME='/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'
# Giảm dung lượng build; phải giữ giống nhau giữa các lần so sánh.
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0
export CARGO_BUILD_JOBS=3
cargo xtask decision query USER-09@r1
cargo xtask decision query WR-04
cargo test --locked -p vink-app --test document_budget
cargo test --locked -p vink-app --test postgres long_document_http_boundaries -- --test-threads=1
cargo xtask build
cargo test --locked -p vink-app --test postgres document_budget::long_document_budget -- --ignored --exact --nocapture --test-threads=1
```

SQLx tạo database riêng và áp dụng migrator thật. Browser chỉ được gọi khi có `VINK_CHROME`;
thiếu biến này tạo `browser.json` với `NOT_RUN`. Output phải là đường dẫn tuyệt đối, đặt ngoài
`target` để `cargo clean` không xóa evidence. `fixtures.json` chỉ chứa corpus tổng hợp để browser
so sánh đầy đủ; không đưa manuscript thật vào harness. Không commit profile Chrome hoặc database.

Hàm benchmark dùng `Vink<PgStore>` thật; fixture public được tạo bằng SQL tương tự `dev_seed`
sau snapshot thật, không gọi evaluator và không chứng minh luồng duyệt/publication production.
Server loopback có identity test cố định; không phải bằng chứng đăng nhập production.
Không thêm runner, migration hay dependency.

## Phương pháp và ngưỡng đề xuất trước khi đọc kết quả

`CHOICE-01 A` + browser phase mỏng: mở rộng integration tests hiện có, CDP hiện có; không thêm
framework benchmark. Năm mẫu mỗi phép đo CPU/SQL; giữ raw array, median và max để mô tả,
không gọi max của năm mẫu là p95. Lần đầu trong process không đồng nghĩa OS/PG cold cache.
Browser phân biệt fresh profile/cold bundle cache với process/bundle warm; network loopback không
đại diện mạng mobile. Rust và WASM chạy profile `dev` không debug symbols, không suy latency release.

- **Transport:** hiện là 2 MiB body mặc định Axum. Bất kỳ corpus hợp lệ bị từ chối là compatibility
  gap phải xử lý ở boundary HTTP trước khi tự đặt thêm product limit. 512 KiB cache record chỉ là
  quota cache; vượt quota phải miss/fallback, giữ online snapshot và buffer đang viết.
- **Editor đề xuất:** khi đo release trên thiết bị đích, snapshot/input trên main thread có p95
  >50 ms hoặc save local ngoài network >250 ms thì ưu tiên tối ưu adapter/render. Phép đo desktop
  dev hiện tại chỉ phát hiện hotspot, chưa phê chuẩn ngân sách này.
- **Reader đề xuất:** SSR local >500 ms hoặc DOM >10.000 element ở corpus được phép là tín hiệu
  cần đánh giá giới hạn render/pagination/virtualization. Không tự thay nội dung hoặc cắt block.
- **History đề xuất:** owner-state ở 100 revision >50 ms trên môi trường release đích, hoặc tăng
  gần tuyến tính theo tổng byte lịch sử, ưu tiên loader #230 trước storage #231.
- **`CHOICE-02`:** giữ compression hiện hành. Chỉ đề xuất LZ4 khi fresh dataset cho ≥20% giảm
  tổng storage/WAL hoặc latency đáng kể, không làm read/write chậm >10%, lặp lại được với corpus
  đại diện và sample đủ lớn. Không quyết adoption từ năm mẫu dev hay từ một case ASCII lặp.
- **`CHOICE-03 A` cho runtime hiện tại:** giữ domain limit. Byte/block/line guard mới chỉ là đề
  xuất nếu evidence vượt budget; phải phân tích dữ liệu cũ và quyền tác giả trước signoff riêng.
  Không lấy quota cache làm lý do hạ 200.000 scalar.

`EXPLAIN ANALYZE` SELECT không buộc xuất/deserialize JSONB; dùng cùng kết quả adapter load để
thấy chi phí detoast/JSON/validation. UPDATE plan chạy trong transaction rồi rollback; WAL trong
plan vẫn là WAL thật. App save timings gồm standing fence và commit. WAL delta dùng insert LSN
của cluster scratch cô lập; vẫn có background/checkpoint noise. Relation sizes là cumulative của
toàn bộ case đã chạy, không gán page allocation của bảng chung cho một work. Dead tuples là ước
lượng stats, không phải kiểm đếm physical tuple chính xác.

History 1/10/100 chạy cho hai case 200k (`vi_nfc`, `tiny_blocks`) bằng save + capture thật, mỗi
revision có title khác để không bị reuse snapshot. Case khác có một revision. Đây là repeated-save
workload mô phỏng, không khẳng định autosave đã có. Capture time/WAL ghi tại checkpoint 1/10/100,
không phải tổng của toàn history; `revision_document_column_bytes` là tổng byte column snapshot.

Compression probe dùng bảng scratch riêng với JSON từ serializer thật, fresh dataset giống nhau
cho `default`, `pglz`, `lz4`. Nó đo insert, năm read/update, WAL và heap/TOAST/index. Read/update
này là SQL probe, không thay cho app timings. Sau ALTER pglz→LZ4, kiểm `pg_column_compression`
ở hàng cũ và hàng mới; không rewrite dữ liệu cũ, không DDL production. Adoption cần tính WAL,
đĩa tạm, thời gian rewrite/lock, backup/restore và rollback; đổi policy về pglz cũng không tự
rewrite hàng LZ4 đã tồn tại.

## Kết quả lần chạy

Đang thu thập; chưa có kết luận PASS trước khi thực thi. Báo cáo cuối phải ánh xạ AC-01..AC-07,
raw evidence, môi trường, source SHA và các boundary chưa chạy.
