# Phép đo storage #231

## Tiêu chí ghi trước khi đo

So sánh PostgreSQL scratch trên cùng lịch sử và compression hiện hành. Điều kiện bắt buộc là
round-trip đúng document v1, dấu tiếng Việt, thứ tự khổ/dòng và hash nghiệp vụ. Phương án A
(JSONB) là mặc định trong khi thiếu bằng chứng workload đại diện hoặc chi phí vượt ngân sách.
Tín hiệu để tiếp tục kiểm chứng B là giảm ít nhất 20% tổng relation và WAL của lịch sử có nhiều
body trùng thật. Tín hiệu để tiếp tục kiểm chứng C là giảm ít nhất 30% cả hai so với phương án
đơn giản nhất đạt mục tiêu, kể cả chi phí manifest/index. Trước khi đề xuất áp dụng, cần lặp
kết quả trên workload đại diện, giữ p95 đọc/tổng ghi trong 10% của baseline hoặc có ngân sách
đích chấp nhận đánh đổi cụ thể, đồng thời qua kiểm tra integrity và vận hành. Các số này là tiêu
chí kỹ thuật tạm thời của RFC, không phải SLA hay policy sản phẩm.

Dừng thử nghiệm sau bốn corpus, một lịch sử 24 revision mỗi corpus và bốn layout. Kết quả này
có thể bác bỏ một giả thuyết hoặc chỉ ra ca cần đo tiếp. Tỷ lệ edit/revision thật, RSS từng layout,
latency release/production, byte HTTP và chi phí GC/migration cần evidence riêng.

## Tái lập

Chạy từ repository root, dùng PostgreSQL scratch riêng như trong
[#229](../../long-document-budget.md#tái-lập). Với role, port và output của máy đang chạy:

```sh
export DATABASE_URL="postgres://${USER}@127.0.0.1:55231/postgres"
export VINK_CONTENT_STORAGE_OUTPUT=/tmp/vink-content-storage-postgres.json
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0
./tools/local-budget run -- cargo test --locked -p vink-app --test postgres content_storage_rfc:: -- --test-threads=1
./tools/local-budget run -- cargo test --locked -p vink-app --test postgres content_storage_rfc::content_storage_probe -- --ignored --exact --nocapture --test-threads=1
```

[Test #231](../../../../services/vink-app/tests/postgres/content_storage_rfc.rs) dùng
[corpus #229](../../../../services/vink-app/tests/support/document_budget.rs), seed 229. Hai case
Việt nhiều đoạn lấy các đoạn 600 scalar sau NFC từ nguồn NFD của #229. Case thơ/tab dùng khổ bốn
dòng, case Việt NFC giữ một đoạn lớn. Mỗi history có initial, title-only, chèn ở đầu/giữa/cuối,
xóa, reorder, undo, và 16 lần sửa nhỏ liên tiếp. Mọi layout lưu đúng 24 revision với title riêng.
Cùng validator/serializer/hash thật, đo lại bằng dữ liệu tổng hợp.

Các bảng logged scratch được tạo mới cho từng layout. A lưu một JSONB/revision. B lưu JSONB
nguyên văn theo hash có FK. C lưu JSONB array block, khóa payload và bảng thứ tự riêng. C dùng một
query lắp lại JSONB theo ordinal. Greedy 60 KiB là control. Cả B/C dùng in-memory set của duy nhất
một writer để chỉ gửi payload mới, bỏ chi phí tìm kiếm/so collision qua DB. Dùng cùng pglz, commit
đồng bộ. DDL tạo bảng, query LSN, fixture, kiểm hash và validation v1 chung nằm ngoài timing.
`prepare_ms` đo chunk/serialize payload/hash/reuse sau full-document serialize chung.
`commit_ms` gồm SQLx pool + insert + commit. `read_ms` gồm SELECT + join/assembly + SQLx
decode, chưa gồm parse/NFC/hash sau đọc. Mọi read đều đối chiếu exact normalized document và hash
v1. Phần schema ứng viên và các tình huống sản phẩm được mô tả trong [RFC](../../../architecture/content-storage-rfc.md).

- **Byte:** `full_json_bytes` là compact canonical JSON, `payload_bytes` gồm bytes của format
  payload tương ứng. `new_payload_bytes` là compact payload thêm vào kho; reuse tính theo
  payload đã biết trong history đó. `payload_column_bytes` phản ánh byte column đã lưu, còn
  `payload_jsonb_text_bytes` là JSONB xuất text. Cộng `total_bytes` để lấy heap/TOAST/index và
  fork phụ. Chênh lệch giữa relation và column phản ánh cả overhead/padding. WAL là delta LSN
  của cluster scratch theo transaction, có thể gồm background noise.
- **Thời gian:** p50 là median, p95 nearest rank của 24 thao tác hỗn hợp theo layout/toàn history.
  Với ghi, cộng `prepare_ms + commit_ms` của từng mẫu trước khi sắp xếp; p50 lấy trung bình
  mẫu thứ 12 và 13, p95 lấy mẫu thứ 23 (đếm từ 1). Đọc dùng cùng cách trên `read_ms`.
  Chỉ làm tròn đến hai chữ số thập phân sau khi tổng hợp, không cộng các percentile riêng.
  Mỗi revision đọc một lần ngay sau commit. Đây là cửa sổ warm/trộn loại edit, chưa kiểm cold
  cache, concurrent load, app end-to-end, full protocol bytes hoặc production.
- **Bộ nhớ:** lượt đầu chạy trực tiếp binary test dưới `/usr/bin/time -l`, max RSS 69.844.992 B
  cho toàn process test gộp bốn layout; PG backend, peak theo layout và thiết bị đích `NOT_RUN`.

## Kết quả chạy

Ngày 16/11/2026, `arm64` Mac16,10, RAM 24 GiB, macOS 15.6, Cargo 1.99.1, PostgreSQL 17.11
Homebrew, UTF-8/pglz, shared buffers 128 MiB, fsync/synchronous_commit/full_page_writes on,
wal_compression off. Base `f0ef35193b2bef2fc9ded9f66b1482a4557ac03e`. SHA-256 file probe lúc đo
`208477807cf6fd23a212cde39e684fdb6a7059bb9ad8f7f13035f6c01c5deac8` (trước khi bổ sung ca chèn ở biên trong
và test constraint ở cuối file), corpus
`4d5d36971ef4041bb15b4ae928b11e19318098b456e2b9afa74e8c93a6bdf678`.
Binary dùng profile `test` **unoptimized**, tắt debug/incremental theo lệnh trên; số đo prep
không đại diện CPU cost của bản release.

- **PASS** [Lượt 1](postgres.json) chạy test binary từ `cargo test`, 33,99 s; **PASS**
  [lượt 2](repeat.json) qua command Cargo trên, 33,18 s. Mỗi lượt có 384 capture/read round-trip.
  Lượt đầu dùng trong bảng RFC. Lượt 2 khớp hash v1, payload/ref count và logical/stored column
  bytes của mọi mẫu. Tổng relation lệch một page 8 KiB ở B của Việt nhiều đoạn 190k. Tổng WAL
  từng layout lệch tối đa 184 B. C p95 prep+commit 190k Việt nhiều đoạn là 25,20 và 19,41 ms;
  A 9,05 và 9,62 ms. Tín hiệu dung lượng và vượt ngưỡng ghi lặp lại.
- **PASS** control pure `content_storage_boundary_shift`: chèn đầu/giữa/cuối, đủ đúng một nhóm,
  delete/reorder, empty/repeated/oversized block. Số greedy nhóm reuse lần lượt 0/0/3/3 ở bốn
  ca insert ban đầu, 0 khi xóa đầu và 3 khi xoay nguyên nhóm. Tại biên phía trong, chèn 3
  block giữ 1 nhóm, chèn đủ 40 block giữ cả 3 nhóm.
- Lượt build đầu báo SQLx 0.9 từ chối string SQL động. Đổi sang literal theo variant và build
  lại. Lượt PG setup đầu thiếu role trong URL nên SQLx dùng `anonymous` và dừng trước phép đo.
  Thêm role đúng của cluster scratch, chạy lại hai lượt thành công.

### Kiểm tra cuối trên base mới

Nhánh được rebase lên `8424a39be933e15ea376d868ad10088ee59f0083` (#252). Delta upstream thêm
launcher local và cập nhật xtask process; code App/corpus dùng trong phép đo giữ cùng input.

- **PASS** 11 test lọc theo `content_storage_rfc::`, `conditional_reads::`, `publication_state::`
  trên binary integration test. Một benchmark opt-in ignored ở lượt này. Constraint scratch
  chặn thiếu chunk, trùng ordinal và xóa payload có reference; connection đọc không thấy
  revision trước commit, sau commit lắp đúng body. Các FK ghép/GC/atomic public pointer của
  schema đề xuất được đánh giá qua kế hoạch riêng trong RFC.
- **PASS** `cargo fmt --all -- --check`, `cargo xtask decision validate`, kiểm file/heading links
  và `git diff --check`. Registry sau proposal có 126 revisions, snapshot
  `9dbad85666397d5d9b76d811440fc80ecf12f034328dea0ec7c22114e3596cb3`.
- Lượt link lại bằng `cargo test` trả lỗi disk trong bước strip trên Mac. Build đúng test
  bằng `cargo rustc --locked -p vink-app --test postgres --profile test -- -C strip=none`
  với cùng flags debug/incremental, rồi chạy trực tiếp binary `postgres-*` với ba filter
  ở trên và `--test-threads=1` trên cùng cluster scratch.
- Full lanes, browser/CMP, benchmark release/production và GitHub CI `NOT_RUN` trong #231.

### Đối soát review PR #253

Review head `345725f4dabe6288edd720aff4c0b5c297e7faa8`, base/merge-base
`8424a39be933e15ea376d868ad10088ee59f0083`. So source xác nhận App và corpus không đổi từ
base phép đo `f0ef351` đến base PR. Review không phát hiện lỗi hồi quy ở probe; commit bổ sung
chỉ hiệu chỉnh tài liệu và ghi bằng chứng đối soát, proposal vẫn `PENDING`.

- **PASS — phân tích raw:** đọc đủ 768 mẫu của hai lượt bằng Python, tính lại relation/WAL,
  median và nearest-rank p95. Mỗi layout có 24 revision liên tiếp; hash v1 khớp giữa bốn layout;
  `payload_bytes = new_payload_bytes + reused_payload_bytes` ở mọi mẫu. Hai lượt khớp toàn bộ
  trường phi thời gian/WAL, payload row/column bytes; WAL tổng lệch tối đa 184 B.
- Hiệu chỉnh bốn ô của bảng RFC: C/30k p95 ghi `4,41 → 4,40`; B/190k p50 ghi
  `10,74 → 10,73`; C/190k p50 ghi `17,12 → 17,11`; C/thơ p95 đọc `5,60 → 5,59` ms.
  Các số raw và kết luận chọn A được giữ nguyên.
- **PASS — tài liệu:** `cargo fmt --all -- --check`, `cargo xtask decision validate`
  (qua launcher local-budget, snapshot không đổi), kiểm liên kết/file/heading và
  `git diff --check`.
- **NOT_RUN trong lượt review — PostgreSQL/App:** artifact test cũ không còn, đĩa còn khoảng
  3 GiB nên không build lại integration binary. Hai lượt scratch và 11 test ở trên là bằng
  chứng lịch sử của PR, không phải lần chạy mới. Muốn đo lại cần đủ chỗ build rồi chạy các
  lệnh tái lập; release/production, browser/CMP và GC/binding của schema ứng viên vẫn chưa đo.

SHA-256 của raw được đối soát:

| File | SHA-256 |
| --- | --- |
| [postgres.json](postgres.json) | `54d1fa7c3f1359c65d89b3e570fff3a3a32e26a8f2d1ee91e8acccc2c1edb8df` |
| [repeat.json](repeat.json) | `9c5ac1c0c19a728b5da8a9fa23c1747415ee224ad2c313d48783f982c068cef6` |
