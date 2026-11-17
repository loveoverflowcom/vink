# Ngân sách document v1 — #229

Baseline đo tài liệu tổng hợp trước khi quyết định chia nhỏ storage. Đây là evidence phát triển,
không phải SLA production hay quyết định đổi giới hạn bài viết.

**Đối soát 17/11/2026:** #232/#249 đã merge vào `develop@607ef83`;
[biên bản AC/provenance](long-document-reconciliation.md) xác nhận phạm vi baseline đã đủ.
Các số dưới đây giữ source đo lịch sử; owner-state hiện đã dùng projection #230/#248.
F1 thuộc [#273](https://github.com/loveoverflowcom/vink/issues/273), F3 thuộc
[#274](https://github.com/loveoverflowcom/vink/issues/274). Không gán timing cũ cho HEAD mới.

**Hiệu chỉnh sau review #232:** các mẫu app save `begin/middle/end` của baseline cũ gửi cùng
payload năm lần; chỉ lần đầu đổi nội dung, bốn lần sau là content no-op (version vẫn tăng).
Không dùng median của nhóm đó làm chi phí của năm edit độc lập. Các số cũ được giữ để bảo toàn
provenance; xem [đối soát follow-up](#đối-soát-follow-up-229) và measurement v2 bên dưới.

<!-- vdr: active USER-09@r1 -->

- **Sản phẩm:** `USER-09@r1 PRODUCT_CONFIRMED`, bài ngắn; `WR-04 DEFERRED` trong PRD.
- **Kiến trúc:** giữ các trạng thái ADR hiện hành. JSONB/chunking và adoption LZ4 chưa được chốt.
- **Triển khai:** chỉ corpus, regression và phép đo opt-in; runtime `persistence-api`/`web` giữ nguyên.
- **Evidence:** source base `a3e65360e316d27e5d029a43aef8b3083f622fb7`; kết quả thực thi và giới hạn
  nằm bên dưới. Baseline không dùng báo cáo của PR #227 (khi đó còn mở) làm kết quả tự chạy.

## Tái lập

Owner corpus: [support/document_budget.rs](../../services/vink-app/tests/support/document_budget.rs).
Seed `229`, LCG cố định, 21 case: 30k/100k/200k scalar sau NFC × ASCII lặp, tiếng Việt seeded
NFC/NFD, emoji, thơ bốn dòng/khổ có tab, nhiều block một ký tự và JSON escaping.
Tiếng Việt seeded là chuỗi ký tự tổng hợp có entropy; không đại diện phân bố văn học production.
Scalar không tính dấu xuống dòng/khổ do schema biểu diễn bằng cấu trúc. Source editor có thêm các
dấu xuống dòng đó. JSON UTF-8, JSONB/TOAST, HTML và RAM là các đại lượng riêng.

```sh
# Chọn cluster local scratch có CREATEDB; không trỏ vào database cá nhân/production.
export DATABASE_URL='postgres://manhblue@127.0.0.1:55529/postgres'
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
# Bao gồm kiểm độc lập của từng sample và hai negative controls; benchmark opt-in vẫn ignored.
cargo test --locked -p vink-app --test postgres long_document_ -- --test-threads=1
cargo xtask build
cargo test --locked -p vink-app --test postgres document_budget::long_document_budget -- --ignored --exact --nocapture --test-threads=1
```

SQLx tạo database riêng và áp dụng migrator thật. Browser chỉ được gọi khi có `VINK_CHROME`;
thiếu biến này tạo `browser.json` với `NOT_RUN`. Output phải là đường dẫn tuyệt đối, đặt ngoài
`target` để `cargo clean` không xóa evidence. `fixtures.json` chỉ chứa corpus tổng hợp để browser
so sánh đầy đủ; không đưa manuscript thật vào harness. Không commit profile Chrome hoặc database.

Cluster của phiên này được tạo riêng bằng `initdb -D /tmp/vink-229-pg-data -A trust --no-locale
--encoding=UTF8`, bind `127.0.0.1:55529`, socket `/tmp`, `max_connections=60`,
`shared_buffers=128MB`. Role local là `manhblue`; khi tái lập trên máy khác thay URL bằng role
test của máy đó. Database/data directory cũ được giữ lại; chỉ `initdb` vào thư mục scratch mới.

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

Căn cứ PostgreSQL 17: [TOAST](https://www.postgresql.org/docs/17/storage-toast.html),
[SET COMPRESSION](https://www.postgresql.org/docs/17/sql-altertable.html),
[MVCC/vacuum](https://www.postgresql.org/docs/17/routine-vacuuming.html), đọc 16/11/2026.
Compression policy áp dụng khi ghi mới; MVCC giữ phiên bản cũ đến khi có thể reclaim. Vì vậy
không diễn giải UPDATE là ghi đè tại chỗ, cũng không coi ALTER policy là benchmark rewrite.

## Kết quả lần chạy

Ngày 16/11/2026, Mac Apple M4/24 GiB, macOS 15.6, Rust/Cargo 1.99.1, Node 22.15.0,
PostgreSQL 17.11 Homebrew (`default_toast_compression=pglz`, shared buffers 128 MB),
Chrome 157.0.8103.78. Lượt baseline đầy đủ pin source `5833624`; full SHA và build flags nằm trong
[environment.json](evidence/long-document-229/environment.json). Baseline này trên integration base
`a3e6536`. Trong lúc đo, `develop` merge #227 thành `69f71f0`; branch nhập base đó tại `3d05fa6`
và kiểm tương thích riêng bên dưới. Không gán số đo baseline cho UI lưu sau #227. Domain,
serializer, PostgreSQL persistence và Reader source không thay đổi trong delta upstream đó.

Raw [PostgreSQL + CPU](evidence/long-document-229/postgres.json),
[compression](evidence/long-document-229/compression.json),
[kiểm lặp](evidence/long-document-229/repeat.json). Hai lượt cùng seed tái tạo đúng cả 21 case
về scalar, block/line, JSON/wire byte và hash v1. Lượt đầu hoàn tất PG/compression nhưng browser
dừng ở lỗi harness trả `EditorView` qua CDP; lỗi đã sửa thành giá trị đơn giản. Lượt cuối là nguồn
nghiệm thu. Dao động latency và HTML resource IDs không phải lỗi tái lập corpus.

### Ngân sách byte và đường app

Byte là UTF-8 compact JSON hoặc HTML thật, chưa nén HTTP. `App save/load` là native application +
PgStore, gồm serialize/deserialize, validation, pool và transaction; không phải latency network.
Median save lấy 20 mẫu (5 × no-op/đầu/giữa/cuối); load cũng có 20 mẫu. SSR là một mẫu router,
gồm đọc PostgreSQL và sinh HTML; không gọi một mẫu là p95.

| Case | JSON document B | Save envelope B | SSR HTML B | App save median ms | App load median ms | SSR ms | Cache record |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| ASCII lặp 200k | 200.046 | 200.123 | 619.372 | 17,68 | 14,24 | 21,39 | Nhận |
| Việt NFC 200k | 452.058 | 452.129 | 1.375.387 | 53,88 | 41,67 | 65,35 | Nhận |
| Việt NFD 200k | 452.058 sau NFC | 634.538 trước NFC | 1.375.387 | 54,43 | 41,91 | 63,35 | Nhận sau NFC |
| Emoji 200k | 800.046 | 800.116 | 2.419.344 | 49,18 | 24,76 | 49,06 | Miss |
| Thơ/tab 200k | 470.024 | 470.098 | 982.126 | 58,92 | 45,90 | 72,65 | Nhận |
| Block nhỏ 100k | 1.600.031 | 1.600.107 | 4.519.377 | 139,28 | 102,42 | 538,34 | Miss |
| Block nhỏ 200k | 3.200.031 | 3.200.107 | 9.019.329 | 285,73 | 206,46 | 1.074,43 | Miss |
| Escaping 200k | 285.705 | 285.778 | 1.735.674 | 25,38 | 21,71 | 49,56 | Nhận |

`tiny_blocks-200000` có 200.000 block/line hợp lệ theo domain, nhưng save HTTP trả `422` do body
vượt 2 MiB, không tới conditional UPDATE. Đây là gap transport hiện hữu, không phải product limit
được chấp nhận mới. Direct app save thành công không chứng minh HTTP save thành công. Với NFD,
200k scalar **sau** NFC có nhiều scalar/byte đầu vào hơn; không được suy byte từ độ dài đã chuẩn hóa.

### CPU thuần và chuyển source

Median 5 mẫu, ms, native profile dev; clone input làm trước cửa sổ đo validation. NFC riêng đo
normalize từng line; validation **đã gồm** NFC nên không cộng hai cột đó.

| Case 200k | NFC | Validate gồm NFC | Hash v1 | Serialize JSON | Document → source | Source → DTO |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Việt NFD | 29,94 | 33,04 | 2,21 | 6,19 | 0,04 | 2,01 |
| Emoji | 18,33 | 21,52 | 3,88 | 10,81 | 0,08 | 3,63 |
| Thơ/tab | 38,41 | 42,00 | 3,14 | 6,81 | 0,27 | 2,66 |
| Block nhỏ | 47,91 | 80,47 | 34,71 | 82,37 | 33,09 | 35,44 |

### History, TOAST và MVCC

| Case 200k | Owner-state median ms: 1 / 10 / 100 revision | Tổng byte column document ở 100 revision |
| --- | --- | ---: |
| Việt NFC | 44,88 / 429,64 / 4.269,55 | 29.858.000 |
| Block nhỏ | 255,87 / 2.459,83 / 24.670,40 | 47.035.900 |

Ở baseline trước #248, `load_paired_state` SELECT và decode document của **mọi** revision. Độ tăng gần tuyến tính
theo lịch sử là lý do đo/ưu tiên loader ở #230; không sửa loader trong #229. Snapshot có title khác,
document giống nhau vẫn tốn một column TOAST riêng; không có dedup/CAS được suy ra từ hash.

Snapshot relation sau toàn bộ PG corpus, trước browser (gồm seed và mọi work, không phải một work):

| Bảng | Heap B | TOAST total B | Main indexes B | Total B | Dead tuples ước lượng |
| --- | ---: | ---: | ---: | ---: | ---: |
| revisions | 81.920 | 81.993.728 | 49.152 | 82.149.376 | 0 |
| working_drafts | 49.152 | 64.348.160 | 16.384 | 64.446.464 | 34 |

`Total` còn gồm các fork phụ; không lấy heap + main index để suy toàn bộ dung lượng. Stats/vacuum
và bloat sau update phụ thuộc thời điểm. Raw giữ từng checkpoint cùng read/update plans:
`EXPLAIN (ANALYZE, BUFFERS, WAL, FORMAT JSON)`. Plan read rất rẻ không loại trừ detoast/JSON/NFC
ở adapter. WAL delta của từng save/capture và statement WAL nằm trong raw, không thay bằng ước
lượng từ JSON size.

### Compression: hỗ trợ LZ4 nhưng chưa nên đổi

| Fresh probe, cùng 21 case + cùng chuỗi update | default | pglz | LZ4 |
| --- | ---: | ---: | ---: |
| Tổng relation B sau workload | 30.965.760 | 30.965.760 | 43.933.696 |
| Column Việt NFC 200k B | 298.580 | 298.580 | 356.976 |
| UPDATE Việt NFC 200k median ms | 14,22 | 14,29 | 8,44 |
| UPDATE Việt NFC 200k median WAL B | 327.928 | 327.928 | 391.960 |

LZ4 nhanh hơn ở case này nhưng tổng probe tăng khoảng 41,9%; column và WAL Việt NFC tăng khoảng
19,6%/19,5%. Ngược lại ASCII lặp 200k có column 884 B với LZ4 so với 2.388 B với pglz.
Không dùng ASCII lặp để đại diện corpus có entropy. Relation total gồm churn/dead TOAST sau chuỗi
update giống nhau, không phải chỉ live payload. Read/update của mọi case, heap/TOAST/index và WAL
giữ trong raw. `pg_column_compression` kiểm được hàng cũ vẫn `pglz`, hàng mới là `lz4` sau ALTER.

**`CHOICE-02 A`: giữ default/pglz.** Không production DDL, rewrite hay đổi cấu hình. Lợi ích CPU
chưa vượt được đánh đổi storage/WAL ở corpus này; dữ liệu thật có thể cho kết quả khác.

### Reader/editor trong Chrome

[Raw browser](evidence/long-document-229/browser.json): **PASS 42 Reader scenes + 84 editor scenes**,
21 case. Reader 30k/100k đo tại 1440/chữ100; mỗi case 200k đo đủ 390/1440 × chữ100/200.
Editor của cả 21 case kiểm đủ bốn scene đó. Root font thực tế được đối chiếu 16/32px; không
overflow ngang trong raw. Cùng `EditorView`, toàn bộ raw text và selection anchor giữ nguyên qua
preview/đổi viewport/text scale. CDP composition + commit chạy trên từng case; đây không phải
Telex/VNI hay OS IME thật. No-op, lỗi HTTP và stale saved-version được kiểm ở boundary riêng.

Một mẫu 1440/chữ100, process/bundle cache đã warm:

| Case 200k | Navigation responseEnd ms | Hydrated flag ms từ navigation | Hydration resource script B | DOM elements hiện tại | GetText median ms | Manual save wall ms / outcome |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| Việt NFC | 71,70 | 1.379,70 | 453.747 | 214 | Dưới độ phân giải timer | 188,72 / saved |
| Emoji | 66,40 | 1.820,30 | 801.733 | 214 | Dưới độ phân giải timer | 227,44 / saved |
| Thơ/tab | 83,40 | 590,20 | 482.651 | 995 | Dưới độ phân giải timer | 198,64 / saved |
| Block nhỏ | 1.446,00 | 3.077,00 | 4.001.721 | 200.213 | 3,40 | 436,55 / failed (body quá lớn) |

Navigation timing không phải network thuần: nó gồm TTFB/server và truyền loopback. SSR router
được đo riêng ở bảng trên; raw navigation giữ requestStart/responseStart/responseEnd và
encoded/decoded/transfer size. `hydrated_flag_ms` là marker App hiện hữu, **không** là profiler
đo hoàn tất mọi effect; `ready_wall_ms` chờ marker, body, font và hai frame. Script payload chỉ
đếm script có `__RESOLVED_RESOURCES`; inline bootstrap/module khác được giữ riêng trong raw.
Resource timings và CDP loadingFinished giữ bundle/cache/network bytes. Cold bundle cache chỉ
ở navigation đầu của profile mới, không gọi mọi case đầu tiên là OS/browser cold.

CodeMirror case block nhỏ chứa 399.999 source lines nhưng chỉ có 39 `.cm-line` trong scene
1440/chữ100 (10 ở 1440/chữ200); Reader vẫn dựng 200.000 paragraph. `getText` nhanh không chứng
minh source→DTO/serialize/save nhanh. Manual-save request thật của case này là 3.200.123 B;
HTTP bị từ chối, version máy chủ không đổi và buffer được so **toàn bộ** trước/sau, không chỉ
so độ dài. Các case qua transport đều ACK tăng version; một writer khác làm tăng version rồi
UI save trả conflict, vẫn giữ nguyên buffer. Request bytes giữ riêng với save wall time.

Memory: `Runtime.getHeapUsage` và `Memory.getDOMCounters` được lưu riêng. V8 `usedSize` của các
Reader sample trong lượt này khoảng 3,9–173,9 MB; đây là điểm đo trong process warm, chưa ép GC,
có thể chứa dữ liệu/DOM từ navigation trước. Editor heap ban đầu đo **trước** khi inject expected
source; heap cuối có thêm các bản so sánh của test (raw đã gắn cờ). Không gán chênh lệch heap cho
riêng một document, không gọi đây là peak/RSS hay tổng RAM WASM. Toàn process/WASM memory và
thiết bị mobile thật còn `NOT_RUN`.

Cache oversized: publication emoji hợp lệ có envelope **800.554 B**, vượt **524.288 B**.
Rust encoder/decoder và browser Memory/NoOp/IndexedDB đều miss, không durable ACK, không mất
online snapshot. Gate production vẫn đóng trước khi test chủ động gọi adapter. Đây là evidence
adapter của #203, không phải lỗi offline đang rollout hay lý do đổi content limit.

[Ảnh diagnostic và metadata](evidence/long-document-229/README.md) dùng corpus entropy tổng hợp;
đã mở kiểm 390/chữ200 và 1440/chữ100. Không thay UI, không audit fidelity;
`NO_APPROVED_REFERENCE`. Các ảnh còn lại là capture, không tự là visual PASS.

## Nghiệm thu và điều kiện quyết định

| AC | Kết quả trong phạm vi #229 | Evidence / gap |
| --- | --- | --- |
| AC-01 | PASS | 21 case, validator/serializer/hash thật; 3 pure regressions; repeat giữ byte/hash; scalar, line, block, UTF-8 và envelope tách riêng |
| AC-02 | PASS local PostgreSQL | App saves/loads, history 1/10/100, plans/WAL, relation sizes/stats; không thay bằng Python synthetic PG |
| AC-03 | PASS local Chrome, có giới hạn | SSR/native riêng; 42 Reader + 84 editor scenes, payload/DOM/heap/getText/save, selection/composition; physical mobile/OS IME và total RSS/WASM NOT_RUN |
| AC-04 | PASS | default=pglz so với LZ4 được hỗ trợ; fresh dataset, latency/WAL/sizes và hàng cũ/mới sau ALTER; không production DDL |
| AC-05 | PASS baseline/regression | limit−1/limit/limit+1 sau NFC; no truncation, HTTP rejection không đổi draft, stale/UI conflict giữ buffer; transport gap được ghi rõ |
| AC-06 | PASS adapter/gate | Record 800.554 B miss trong Rust + Memory/NoOp/IDB; online snapshot giữ nguyên; rollout policy không đổi |
| AC-07 | PASS report | Raw evidence, ngưỡng đề xuất, vận hành và go/no-go có điều kiện; không triển khai chương/CAS/autosave |

**Go:** giữ document v1/JSONB nguyên khối trong slice này. Với 200k scalar ít block, app save
median khoảng 18–59 ms và SSR khoảng 21–73 ms trên máy này; chưa có evidence buộc chunking.
Tối ưu hẹp cần ưu tiên là owner-state loader (#230), rồi transport compatibility và Reader nhiều
block. Reader DOM/HTML và decode lịch sử không tự biến mất chỉ nhờ chia storage.

**No-go hiện tại:** không adoption LZ4 hay chunking/migration production từ baseline dev này.
#231 cần corpus đại diện, release/device measurements, concurrency/load, backup/rewrite/WAL/
rollback budget và quyết định sản phẩm nếu thêm byte/block/line limit làm từ chối dữ liệu đã hợp
lệ. Giữ hash v1, NFC, thơ/tab, revision immutable và quyền tác giả; không chọn cap mới trong #229.

Chi phí vận hành cần giữ trong RFC: số revision × compressed document, dead tuples/TOAST và
vacuum, WAL/replica/backup, đĩa trống khi rewrite, thời gian lock và kiểm restore. Policy compression
không rewrite hàng cũ; không mô tả rollback như một ALTER có thể khôi phục mọi byte cũ ngay lập tức.

## Commands và giới hạn bàn giao

- `cargo test --locked -p vink-app --test document_budget`: **PASS 3/3**.
- `cargo test --locked -p vink-app --test postgres long_document_http_boundaries -- --test-threads=1`:
  **PASS 1/1** trên PG scratch.
- Benchmark opt-in với `--ignored --exact --nocapture --test-threads=1`: **PASS 1/1**, source
  `5833624`, 503,52 giây runtime test, đủ 21/21 case và Chrome phase; không đặt timeout SLA từ số này.
- `cargo xtask verify quick`: **PASS** tokens/manifest/fmt, ba clippy, doctests, unit/contract và
  repository checks. Lượt trong sandbox trước đó **FAIL** ở Unix socket Identity; chạy lại với
  quyền socket local đã PASS. Build đầu **FAIL** vì hết đĩa; clean artifacts mới tạo, tắt debug
  symbols/incremental và build lại **PASS**. Không xóa database/evidence để lấy chỗ.
- Scoped `cargo xtask verify web --scenario storage`: **PASS** prerequisites/artifacts, bundle
  secrets scan, transaction/quota/multi-tab/migration/profile reopen và SIGTERM.
- Full `verify postgres`, full `verify web`, Quint/Kani/CMP: **NOT_RUN**; không có production
  behavior/model/native change để đòi các lane đó. GitHub CI **NOT_REQUIRED**, không đọc/chờ/poll.
- Safari/Firefox, physical Android/iOS, OS IME/screen reader, production load, OS/PG cold restart,
  release benchmark và tổng browser RSS/WASM: **NOT_RUN**. Muốn chốt SLA/adoption phải đo lại đúng
  target, không dùng Mac M4 thay mobile.

Lệnh tái lập ở đầu báo cáo chạy từ repository root. Raw lớn `fixtures.json` tái tạo bằng seed,
không commit 26 MB corpus; raw samples được commit cùng báo cáo. Logs đầy đủ của phiên tại
`/tmp/vink-229-evidence`, kèm lượt lỗi đầu để đối chiếu. Không tick/đóng issue, merge hay deploy.

Cleanup: clean Cargo trước fetch; khi build debug hết đĩa chỉ clean artifacts mới tạo rồi build
compact. Evidence UI #224 (2,3 GiB) được chuyển tạm và phục hồi về `target/ui-preview`. Worktree
#204 giữ nguyên diff 24 file (+2066/−105) và branch cũ; không chạy hàng đợi của nó. PostgreSQL
scratch #229 đã dừng, data/evidence còn giữ; hai PostgreSQL cũ vẫn chạy. PID Node #204 ban đầu
94236 không còn ở lần kiểm cuối, nguyên nhân `UNKNOWN`; task không gửi signal tới PID đó hay
khởi động lại. [environment.json](evidence/long-document-229/environment.json) ghi rõ boundary này.

### Tương thích sau khi develop merge #227

Sau baseline, fetch thấy `develop@69f71f0cb61228dbe5ceea775b3c34c237705922` đã merge #227.
Nhập base bằng merge commit `3d05fa601e8bf02b5a00e33612cdcd703cd9f600`, không rewrite các SHA
đã đo. `cargo xtask build` và `verify quick` trên source sau merge **PASS**. Chạy lại
`cargo test --locked -p vink-app --test document_budget --test postgres long_document -- --test-threads=1`
**PASS 3 pure + 1 PG/HTTP, 1 benchmark ignored**.

[Post-merge browser smoke](evidence/long-document-229/postmerge-browser.json) **PASS** trên ba case
`vi_nfd-30000`, `poem_tabs-200000`, `tiny_blocks-200000`: 9 Reader + 12 editor scenes, API thật,
snapshot/hash thật, save ACK/rejection, conflict và exact buffer/selection. Dataset scratch có
một revision mỗi work; không dùng smoke này để cập nhật bảng latency/history phía trên. Có hoạt
động build/test khác cùng thời gian; số timing trong raw smoke không phải benchmark so sánh.

Lượt setup smoke đầu để draft ở version 1 nên request “stale 1” vẫn hợp lệ; assertion FAIL đúng
vì fixture sai tiền đề. Đã khôi phục corpus và tăng version đơn điệu để kiểm stale thật rồi PASS;
không sửa runtime để làm test xanh. Quick từng bị artifact lock khi build chưa xong; đợi build
nhả lock rồi chạy tuần tự đã PASS. Các log lỗi vẫn giữ trong `/tmp/vink-229-evidence`.
Server và PostgreSQL scratch sau merge đã dừng, data còn giữ. Baseline đầy đủ 21 case vẫn thuộc
`5833624`/base `a3e6536`; không khẳng định đã đo lại toàn bộ hiệu năng UI sau #227.

## Đối soát follow-up #229

Audit ngày 16/11/2026 trên `develop@ca9369653a516715fea95ca71fbd818d54c15b0a`, sau merge #247.
Đã đọc issue #229/#230/#231 và PR #232; issue #229 không có comment, PR #232 không có review
submission/comment. Merge #232 là `a81333c71faf6033014a00ec26c9f7b90109d1d9`, head PR là
`13ff80e1d0dff554037c75c5cfd763baa63068f9`. 11 checksum trong manifest lịch sử khớp.
Raw lịch sử chứa đúng 21 case, 42 Reader/84 editor scenes; đây là evidence **đã chạy ở SHA cũ**,
không tự trở thành lần chạy latest develop. PR #248 của #230 còn draft/open khi audit;
loader tối ưu chưa có trong base này.

### Delta thực chất và provenance phép đo

`services/vink-app/tests/postgres/document_budget.rs` của #232 tạo `edited` một lần trước vòng
năm mẫu và không restore bản gốc giữa các save. Vì vậy 12/15 mẫu được gắn `begin/middle/end`
thực tế lặp nội dung đã lưu. Chuyển từ nhóm begin sang middle/end còn đồng thời bỏ edit nhóm
trước. Đây là **lỗi phân loại workload của harness**, không phải lỗi production save.
WAL/latency cũ là output thật của workload đó; không xóa hoặc gọi là benchmark giả.

Measurement v2 (`4f2883630ff27f36c7549e036e6b554166a86692`) reuse corpus, application, PgStore,
SQLx scratch DB và runner cũ. `app_save_samples` restore bản gốc trước **mỗi** sample; kiểm full
content/version trước save, full content/title/version sau commit. Hash v1 trước/sau và
`durable_exact_match` được ghi theo sample. Hai negative controls từ chối cùng payload bị gắn
nhãn changed và payload thay đổi bị gắn nhãn noop. Regression PG không ignored chạy cùng helper
trên thơ/tab và tiny blocks 30k (40 samples tổng). Source review cho thấy nếu bỏ restore,
assertion full baseline sẽ từ chối mẫu thứ hai của nhóm changed; mutation test bỏ restore chưa chạy.

Ranh giới đo v2: raw-input clone, baseline reset, hash phục vụ evidence và readback assertions
nằm **ngoài** app save timer/WAL delta. Timer gồm application validation, PgStore transaction
và commit. Timed load đọc bản gốc vừa restore. Reset làm warm cache và thêm churn ngoài cửa sổ;
relation totals chứa cả reset, không so trực tiếp với totals v1 như lợi ích performance.
Không tuyên bố speedup production từ việc đổi harness. CPU/history/compression/browser giữ
ranh giới cũ; năm mẫu báo median/max, không gọi là p95. Không đổi production code, limits,
schema, content hash, public cache policy hoặc persistence gate.

### Ma trận acceptance → nguồn → gap

Các file:line sau ghim ở source measurement v2. App là `services/vink-app`, Web là
`apps/vink-web`, PG là `services/vink-app/tests/postgres/document_budget.rs`; đường dẫn con
đọc theo owner đó. Acceptance giữ đúng issue, không mở rộng hồi tố sang SLA/thiết bị thật.

| AC nguyên issue (rút gọn) | PR #232 thực sự đã làm | Owner production / test | Audit trước delta | Kết luận follow-up |
| --- | --- | --- | --- | --- |
| AC-01: corpus thật; scalar/byte/block/line/envelope | 21 case, validator/serializer/hash, NFC/NFD và repeated corpus equality | `src/domain/document.rs:68`, `src/domain/revision.rs:10` trong App; `tests/document_budget.rs:9`; `tests/support/document_budget.rs:16`; PG `pure_measurements:26` | PASS lịch sử | PASS: 3 pure tests + 21/21 corpus equality; `followup/summary.json` |
| AC-02: PostgreSQL read/update/WAL/heap/TOAST/index/history | App/PgStore, EXPLAIN, 1/10/100 revisions; output thật | `src/persistence/postgres.rs:165`; PG `plan:212`, `app_save_samples:90`, `long_document_budget:443` | PARTIAL: nhóm changed-write trộn no-op | PASS local PG v2: 420 samples và exact durable readback; `followup/postgres.json`, `pg-focused-final.log`; delta đã merge qua #249 |
| AC-03: Reader/editor tách network/SSR/hydration/DOM/memory/snapshot | 42 Reader/84 editor scenes, loopback timings, V8/DOM proxy; ghi NOT_RUN mobile/OS IME | Web `src/document.rs:90`, `src/editor.rs:45`; `e2e/document-budget.mjs:7`; PG router SSR | PASS theo scope lịch sử; UI latest cần tái lập | PASS local Chrome/SSR: 42 Reader + 84 editor scenes; `followup/browser.json`; RSS/WASM peak NOT_RUN |
| AC-04: pglz/default/LZ4, old/new data, không production setting | Fresh identical datasets, insert/read/update/WAL/sizes; old pglz/new lz4 | PG `compression_probe:244`; scratch table `budget_toast_probe` | PASS lịch sử | PASS SQL scratch: `followup/compression.json`, old/new rows; không sửa production setting |
| AC-05: boundaries/guard, text/no truncation/conflict | limit−1/limit/limit+1; lỗi HTTP/409 không đổi durable text; UI giữ full buffer | App `http/api.rs:650`; PG `long_document_http_boundaries:386`; Web `e2e/document-budget.mjs:138` | PASS baseline/regression; F1 còn là product gap | PASS PG/browser regression: `followup/pg-focused-final.log`, `browser.json`; F1 giữ follow-up transport |
| AC-06: oversized cache fallback/gate | Rust + Memory/NoOp/IndexedDB miss, không durable ACK; online snapshot giữ nguyên | Web `public_persistence.rs:7,103,110,134,165`; pure test `document_budget.rs:62`; browser oversized case | PASS adapter/gate | PASS adapter/gate: oversized case trong `followup/browser.json`; không suy offline rollout PASS, #203 owner |
| AC-07: thresholds/raw evidence/operations/go-no-go | Report có thresholds đề xuất, raw output, rewrite/rollback costs, conditional JSONB/default | Report này + raw historical manifest | PARTIAL: diễn giải changed-save cần hiệu chỉnh | PASS report + v1/v2 provenance/raw/thresholds; `followup/verification.json`, `manifest.json`; report/harness đã merge qua #249 |

### Ba finding và ranh giới tiếp tục

| Finding | Phân loại, evidence đã có từ #232 | Thuộc #229 tới đâu | Hành động/owner tiếp theo |
| --- | --- | --- | --- |
| F1: 200k tiny blocks tạo request >2 MiB, HTTP 422 | Product/transport compatibility gap thật; non-ignored PG test và browser buffer/version regression đã có. Domain chấp nhận, Axum Json rejection được API ánh xạ thành Validation/body. Không phải thiếu test hay lỗi môi trường. | AC-05 yêu cầu đo/regression/đề xuất guard, không bắt tự đổi product/transport limit. | [#273](https://github.com/loveoverflowcom/vink/issues/273) sở hữu HTTP save trong budget hữu hạn; upper bound escaping/NFD/title/envelope, rejection semantics và chi phí tài nguyên. Không hạ limit vì cache quota. |
| F2: owner-state xử lý toàn history ở baseline | Hotspot read đã đo history 1/10/100 trước projection; source SELECT/decode/hash cũ không còn là owner-state hiện hành. | AC-02 cung cấp baseline và corpus. | #230/#248 đã tích hợp metadata projection; [trust map và before/after](publication-state-read.md). Command/worker vẫn kiểm full content. |
| F3: Reader dựng 200k paragraph | Hotspot render thật; `DocumentView` map mọi block; raw DOM/HTML/hydration đã đo, không chỉ giả thuyết. | AC-03/07 yêu cầu đo/đánh giá, không cấp quyết định virtualization. | [#274](https://github.com/loveoverflowcom/vink/issues/274) sở hữu ngân sách article và selection/find/copy/a11y; cần release + thiết bị đích. Không cắt paragraph, đổi thơ hoặc tự triển khai virtualization. |

#231/#253 đã tích hợp [RFC](../architecture/content-storage-rfc.md) đối chiếu sau #230;
proposal storage vẫn `PENDING`. Adoption cần
corpus/workload đại diện, release/thiết bị đích, đủ samples và chi phí manifest/dedup/assembly,
WAL/backup/rewrite/rollback. F1/F3 không tự được giải quyết bằng chunk storage. Giữ phương án
JSONB/default có điều kiện; chapter/CAS/offline/autosave chưa được phép mở trong task này.
[TOAST](https://www.postgresql.org/docs/17/storage-toast.html) và
[SET COMPRESSION](https://www.postgresql.org/docs/17/sql-altertable.html) đã đối chiếu lại:
đổi compression policy không tự rewrite giá trị cũ.

Quint SSOT không cần đổi: delta chỉ sửa workload và assertions của benchmark, không thay
semantics, authority, state transition, lifecycle hoặc production model. Decision query giữ
`USER-09@r1 PRODUCT_CONFIRMED`, snapshot
`6e77cb0a7f4eb67033c6e0df6d8a670a84eb61b3095a307ad0615b771c91dd9b`; `WR-04` là related-only
trong registry, trạng thái `DEFERRED` lấy từ PRD dòng 45. Không tạo signoff mới.

### Kết quả thực thi follow-up

[Raw evidence và commands](evidence/long-document-229/followup/README.md), source `4f288363`,
Mac M4/24 GiB/macOS 15.6, PostgreSQL 17.11, Chrome 157.0.8103.78, Rust 1.99.1/Node 22.15.0,
profile dev/test debug=0, incremental=0, jobs=3. Seed 229, năm mẫu mỗi kind; cold/warm và
measurement boundary trong `environment.json`/`postgres.json`. Không chạy build khác đồng thời
với phép đo. Host thiếu đĩa; có ba query chỉ đọc tiến độ scratch khi history chậm, giữ noise này
trong metadata. Những lượt compile/initdb lỗi môi trường được giữ riêng, không gán thành test PASS.

- PASS: 3 pure regressions; 4 focused checks gồm HTTP/PG, independent-sample regression và
  2 negative controls. Benchmark opt-in bị ignored trong focused run, rồi chạy riêng thật.
- PASS: full opt-in benchmark 570,45 giây, 21 case, history 1/10/100, plans/WAL/relation sizes,
  default/pglz/LZ4 + old/new rows, 42 Reader + 84 editor scenes. Không console error/exception.
- PASS: 21/21 corpus tái tạo khớp mọi scalar/byte/block/line/envelope/hash với raw #232.
  420 measured saves = 105 no-op + **315 thay đổi nội dung thật**; trước sửa chỉ 63/315 mẫu
  gắn nhãn changed thực sự đổi nội dung. Mọi sample mới kiểm exact durable content/version.
- PASS: `cargo xtask verify quick` đầy đủ, retry cuối dùng jobs=1 sau lỗi hết đĩa;
  [raw final](evidence/long-document-229/followup/quick.log) và lỗi ban đầu đều được giữ.
  Browser này là phase `document-budget`, không phải toàn lane Web; PostgreSQL là focused + probe, không phải toàn lane PostgreSQL.

| Case 200k / kind | Median ms (5 mẫu) | Max quan sát ms | Median WAL B |
| --- | ---: | ---: | ---: |
| Việt NFC / noop | 55,69 | 56,14 | 328.352 |
| Việt NFC / begin | 55,61 | 56,00 | 328.336 |
| Việt NFC / middle | 55,28 | 55,94 | 328.352 |
| Việt NFC / end | 55,16 | 56,39 | 328.336 |
| Tiny blocks / noop | 288,17 | 304,43 | 517.768 |
| Tiny blocks / begin | 284,41 | 307,65 | 517.768 |
| Tiny blocks / middle | 283,88 | 1.429,54 | 517.768 |
| Tiny blocks / end | 279,04 | 286,80 | 517.776 |

Max 1.429,54 ms được giữ, không bỏ outlier; năm samples không ước lượng p95 đáng tin cậy.
Owner-state 1/10/100 revision: Việt NFC **45,46 / 442,33 / 4.398,78 ms**; tiny blocks
**260,24 / 2.512,38 / 25.792,31 ms**. Đây là loader cũ, không phải kết quả sau #230.
Compression totals vẫn default=pglz=30.965.760 B, LZ4=43.933.696 B; giữ kết luận conditional
JSONB/default. Các bảng raw v1/v2 không là before/after optimization production.

**Kết luận AC trước merge #249 (lịch sử):** AC-01..AC-07 PASS theo phạm vi baseline local đã nêu.
AC-02/AC-07 cần delta follow-up này để thay phần diễn giải workload cũ trên develop.
Checklist có thể tick theo evidence của nhánh đã kiểm, nhưng #229 giữ OPEN chờ merge delta;
không tuyên bố develop đã chứa bản sửa ở thời điểm đó. Nay #249 đã merge; xem
[đối soát 17/11](long-document-reconciliation.md) cho ancestry, AC và follow-up hiện hành.
AC-01/03/04/05/06 đã có evidence #232 và được kiểm lại.
F1/F3 vẫn là product gaps đã được báo trong acceptance baseline, không biến thành AC mới.

**NOT_RUN:** full PostgreSQL/Web lanes, thiết bị mobile thật, OS Telex/VNI, Safari/Firefox,
screen reader, release/production load, cold OS/PG page cache, total process RSS/WASM peak và
Quint/Kani/CMP. Browser dùng CSS viewport 390/1440 và root font 100/200, input CDP;
V8/DOM counters chỉ là proxy, không suy SLA mobile. Không dùng GitHub CI làm evidence.

### Review trước merge #249

Review HEAD `233804451f2abb4a2b041c4406a549ed080b8153` trên base/merge-base
`ca9369653a516715fea95ca71fbd818d54c15b0a` không tìm thấy regression mới do delta này.
[Bundle review](evidence/long-document-229/review-249/README.md) ghi kiểm checksum/source,
tính lại raw summary và regression PostgreSQL mới: 4 PASS, probe opt-in ignored.
Mutation chỉ giữ lần restore đầu đã làm regression FAIL tại assertion baseline; hoàn nguyên
source byte-identical, build lại và 4 checks PASS. Gap mutation được nêu trong audit trước
đã được kiểm, không sửa lại evidence lịch sử hoặc coi đây là lần đo hiệu năng mới.
Commit review chỉ bổ sung docs/evidence; Quick và full probe dùng lại kết quả source không đổi.
