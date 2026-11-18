# Review độc lập PR #285 trên Mac — 17/11/2026

## Source và phạm vi

- PR ban đầu: `39641d5de06628ac84ad222db0ad6d2741f43550`.
- `origin/develop` sau #283: `9640bdef4b570fe0fc76e90e538de947c2428614`.
- Tích hợp bằng merge `949fc6c` trong clone riêng; không reset/stash/pull checkout chính.
- Sửa ba finding bằng `0543e341690190081e9940df5d84d435759df9a5`. Các lane dưới chạy trên
  source này sau reconcile develop; thay đổi tiếp theo chỉ là tài liệu/evidence. `source.json`
  ghim tree IDs của các owner build/runtime để kiểm lại tính đồng nhất.
- Đã đọc toàn bộ PR body, comments/reviews/threads (trống khi bắt đầu), issues #255/#263 và
  comments của chúng, diff và callers, AGENTS/skills repo, migration/backout, PRD, decision
  payload/semantic diff và evidence trước rebase. Không dùng Actions/status/workflow logs.

macOS arm64; PostgreSQL 17.11 ở **cluster scratch riêng**, `127.0.0.1:55585`; Chrome cục bộ.
Không kết nối DB người dùng/production. Không deploy, không đổi revision/hash/digest lịch sử.

## Findings

| Mức | Trigger và hậu quả | Sửa và bằng chứng |
| --- | --- | --- |
| P1 — đã sửa | Sau tích hợp #283, `snapshot_weight_bytes` còn đọc `document.blocks`; `verify quick` lỗi E0609. Fixtures cache cũng còn `BlockDto`. | Tính dung lượng theo UTF-8 source nguyên văn và một allocation; giữ nguyên budget 1 MiB/entry, 2 MiB tổng. Fixtures over-budget dùng NFD hợp lệ 200k NFC, không dựa vào overhead blocks đã bị xóa. `quick-before.log.gz` FAIL; `quick-final.log.gz` PASS. |
| P2 — đã sửa | Backout chấp nhận dòng chỉ NBSP/Unicode whitespace, tạo v1 line mà validator cũ từ chối; draft sau rollback không đọc được. | Guard bao phủ Unicode White_Space hợp lệ trong canonical source. Probe SQL tái hiện trước sửa; regression mới FAIL trước sửa và PASS sau sửa. `backout-unicode-before.log.gz`, `backout-regression-before.log.gz`, `canonical-postgres-after.log.gz`. |
| P2 — đã sửa | Helper corpus sửa một scalar đầu của NFD, để lại dấu tổ hợp. Ca 200k vượt budget NFC nên test thật FAIL `TooLong`; validator runtime từ chối đúng. | Thay trọn ký tự corpus cùng combining marks, giữ source xung quanh. Probe opt-in đếm vị trí theo NFC. `document-budget-before.log.gz` 2 PASS/1 FAIL → `document-budget-after.log.gz` 4 PASS. |
| P1 — còn chặn cutover/merge | DB có revision shape cũ luôn bị migration từ chối. Runtime mới không đọc legacy; đổi lịch sử cần thay document/hash và ba digest phụ thuộc. | Fail-closed là đúng và có PG test, nhưng chưa có đường nâng cấp được ủy quyền cho môi trường đã có publication. Không tắt trigger hoặc rehash. Giữ DRAFT, #255/#263 OPEN. |

SSOT cũng được sửa: phân biệt measurements blocks/v1 lịch sử với source canonical; không nói
30 ca Markdown đã replay toàn bộ qua PostgreSQL; `title: null` ở HTTP là absence, chỉ chuỗi
client bị từ chối cho Markdown; product replacement không tự cấp kiến trúc signoff.

## Bốn chiều

- **Sản phẩm:** `PRODUCT_CONFIRMED` — một `{format, source}`, không dual version; default tên riêng,
  Markdown H1 đầu tiên; AsciiDoc `DEFERRED`.
- **Kiến trúc:** `VDR-DOCUMENT-SOURCE-FORMAT@r2` **PENDING**, payload
  `478a09f823816984d5f92228046c9cf30ab23b78ffae70febf272dbc49643738`.
  `VDR-CONTENT-STORAGE@r1` vẫn **PENDING** và không đổi. Test/commit/merge không là signoff.
- **Triển khai:** canonical domain/persistence/API; Web default; Markdown nháp/snapshot riêng tư.
  Public Markdown bị gate đến #264; Bàn viết chặn Markdown và CR để tránh đổi nguồn âm thầm.
- **Kiểm chứng:** bảng dưới chỉ áp dụng source đã ghim và boundary thật đã chạy.

## Kết quả local

| Check | Kết quả | Raw evidence |
| --- | --- | --- |
| `verify quick` | PASS 13/13; App 342, Web 240 + 1 ignored, xtask 83 + 1 ignored; 33 literary doctests | `quick-final.log.gz` |
| `cargo test -p vink-app --test document_budget` | PASS 4/4, sau sửa fixture NFD | `document-budget-after.log.gz` |
| `cargo test -p vink-app --test postgres canonical_document` | PASS 6/6, 211 filtered out | `canonical-postgres-after.log.gz` |
| `verify postgres` | PASS 29/29 bước; store 6, App 211 + 6 ignored, Identity 48; role isolation/migration/backup/restore thật | `postgres-final.log.gz` |
| `verify web --scenario main` | PASS 6/6 bước, 5 867 assertions; gồm chặn nháp Markdown/CR, save/reload/conflict và IndexedDB | `web-main-final.log.gz` |
| `verify kani` | PASS 3 expressions + 33 domain harnesses, 0 failure, pinned Kani 0.67.0 | `kani-final.log.gz` |
| `decision query` / `diff` / `validate` | PASS; 128 revisions, 88 references, snapshot `e6e8f231bbc84ab3e69364436a0c949a62345b214350745e6d290712ded4c2ca`; r2 PENDING | `decision-*.json`, `decision-validate.log` |
| Python golden vectors | PASS 2/2; literal byte lengths + u64 big-endian + SHA-256; không gọi parser Rust | `check_hash_vectors.py`, `hash-vectors.log` |

Focused regressions chạy trước commit nhưng trên cùng nội dung file đã commit ở `0543e34`;
full lanes chạy sau commit đó. Các failure trước sửa được giữ riêng, không gộp vào PASS cuối.
Một lượt quick đầu bị dừng trước tests do ngân sách đĩa; chỉ dừng tiến trình của lượt review và
clean target riêng rồi chạy debug=0. Browser lần đầu FAIL ở wasm-bindgen do hết chỗ đĩa,
trước assertions (`web-main-disk-failure.log.gz`); xóa các binary test đã hoàn tất trong clone
review rồi chạy lại cùng source, PASS toàn lane. Không xóa dữ liệu/worktree/target của phiên khác.
Kani preflight ban đầu FAIL vì thiếu launcher `cargo-kani` trên PATH
(`kani-missing-launcher.log.gz`). Bundle 0.67.0 và nightly tương ứng đã có sẵn; dựng launcher
từ crate cache 0.67.0 bằng `cargo build --offline --locked --release --bin cargo-kani`
với manifest cache và target-dir riêng qua local-budget (`kani-proxy-build.log.gz`), chép binary
vào thư mục `review-tools/bin` riêng rồi thêm vào PATH của lệnh. Không cài lại bundle/toolchain
hoặc sửa PATH toàn máy. `cargo kani --version` trả đúng `cargo-kani 0.67.0`.

## Lệnh và môi trường

Các lệnh Cargo dùng `CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0`.
Heavy Cargo dùng `./tools/local-budget run --`, một heavy slot, 2 jobs và 2 libtest threads.
Browser lane chưa được launcher hỗ trợ, chạy trực tiếp với `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2`,
không có heavy lane khác đồng thời. `VINK_TEST_DATABASE_URL` trỏ cluster scratch riêng;
`VINK_PG_BIN`/PATH chọn PostgreSQL 17. `VINK_CHROME` chọn Chrome cục bộ, npm cache riêng.

```sh
./tools/local-budget run -- cargo xtask verify quick
./tools/local-budget run -- cargo test -p vink-app --test document_budget
./tools/local-budget run -- cargo test -p vink-app --test postgres canonical_document
./tools/local-budget run -- cargo xtask verify postgres
cargo xtask verify web --scenario main
./tools/local-budget run -- cargo xtask verify kani
target/debug/xtask decision query VDR-DOCUMENT-SOURCE-FORMAT@r2
target/debug/xtask decision query VDR-CONTENT-STORAGE@r1
target/debug/xtask decision diff VDR-DOCUMENT-SOURCE-FORMAT@r1 VDR-DOCUMENT-SOURCE-FORMAT@r2
target/debug/xtask decision validate
python3 check_hash_vectors.py
```

## NOT_RUN và điều kiện còn thiếu

- Mọi migration trên DB thật/production và chuyển lịch sử bất biến: **NOT_RUN/BLOCKED**.
  Cần chủ sở hữu ủy quyền riêng phương án lịch sử, implementation/audit/backup/backout và kiểm
  trên bản sao đại diện trước cutover; quyền merge không bao gồm quyền migration dữ liệu thật.
- Exact-revision signoff r2: **chưa có**; không tạo file signoff.
- `verify discussion`, `verify preview`, các Web scenario ngoài `main`, CMP/native,
  OS IME thật, screen reader, visual inspection mới và benchmark opt-in #229/#231/#237:
  **NOT_RUN**. Browser assertions không tự chứng minh những boundary này.
- Rust/TS parser differential: **NOT_APPLICABLE** đến khi #256 có parser TS.
- Kani hiện chỉ có harness expressions/discussion/endorsement/taxonomy; không có proof của
  Markdown parser, canonical migration hoặc source byte preservation.

Các AC composite của #255/#263 còn migration/signoff và editor/Reader dependencies; không
đánh dấu hoàn tất hoặc đóng issue. Giữ PR draft. Bảng trước rebase trong README cha là evidence
lịch sử, không được suy thành PASS cho source của lượt này.
