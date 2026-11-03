# Bằng chứng RUST-CORE-01 — 02/11/2026

Phạm vi/source: `codex/rust-literary-domain-core` từ
`3a024091df6eb1da962ad3fa08750954c0b83a4f` (`develop` đã fetch và fast-forward, cây ban đầu
sạch). Kết quả dưới đây áp dụng cho code đi cùng bản ghi này, không phải nghiệm thu production.
[HOW/WHY và ownership](literary-domain-core.md); [lệnh/lane chuẩn](README.md).

## Môi trường

- Máy cloud của dot, Debian 13 x86_64; không dùng Mac, không thay production config.
- Rust/Cargo 1.99.1, rustfmt, clippy và target `wasm32-unknown-unknown` đúng pin repository.
  Toolchain cài riêng trong workspace, không sửa profile/system files.
- PostgreSQL/psql/pg_dump/pg_restore **17.11** từ gói Debian đã đối chiếu SHA-256; giải nén
  riêng trong workspace. Cluster disposable UTF-8/C.UTF-8, chỉ TCP loopback, SCRAM; test harness
  có quyền tạo database/role tạm. Không đưa URL/password vào repo.
- Mỗi shell có namespace process/network riêng; PostgreSQL phải khởi động, chạy test và dừng
  trong cùng shell. Helper ngoài repo thực hiện điều đó và dừng cluster khi lệnh kết thúc.
  Scratch database/role của lane đã cleanup.
- Sandbox không cho bind Unix socket. Lần thử escalation khởi động PostgreSQL không thực thi
  được vì lỗi thiết lập sandbox; không đọc/sửa đường dẫn runtime bị cấm và không retry escalation.
  PostgreSQL TCP-only chạy được bằng quyền sandbox thường. Không đổi/bỏ qua kiểm tra Identity.

## Kết quả thực thi

| Lệnh | Kết quả | Ranh giới / quan sát |
| --- | --- | --- |
| `cargo fmt --all -- --check` | PASS | Rust formatting |
| `cargo test --locked -p vink-app --lib domain::` | PASS | 15 tests thuần: document/title/version/ID/hash; NFC/NFD, Đ/đ, cấu trúc và size limits |
| `cargo test --locked -p vink-app --lib application::` | PASS | 4 tests với RecordingStore: normalized arguments, invalid không gọi store, thứ tự lỗi và outcomes |
| `cargo test --locked -p vink-app --doc` | PASS | 8 compile-fail examples cho type/serde/private-field/ID barriers |
| `cargo test --locked -p vink-app --lib` | FAIL | 37 pass, 3 test Identity socket thất bại tại `http/mod.rs:259` không sửa trong PR; `UnixListener::bind` trả `Operation not permitted (os error 1)` |
| `cargo test --locked -p vink-app --lib -- --skip http::identity_tests::` | PASS | 37 tests pass trên code cuối sau review; 3 test socket bị lọc rõ ràng, không thay kết quả suite đầy đủ |
| `cargo xtask verify quick` | FAIL | Token drift, fmt, cả ba clippy target và 8 doctests PASS; unit suite dừng ở ba test socket nêu trên. Repository checks phía sau NOT_RUN |
| `cargo xtask verify postgres` | FAIL | Phần App/PostgreSQL, shared store, provisioning và worker pass; Identity serve không bind được Unix socket. Các bước sau đó NOT_RUN; cleanup pass |
| `cargo test --locked -p vink-app --test postgres` | PASS | Rerun sau review trên PostgreSQL disposable: cả 14 tests pass; cluster dừng sạch |
| `git diff --check`; kiểm relative links trong bốn Markdown file đổi | PASS | Whitespace và 106 đích link tồn tại; không thay kết quả repository checks chưa chạy |

Trong lane PostgreSQL đã thực thi thành công:

- **PASS** cả 14 App integration tests (`cargo test --locked -p vink-app --test postgres`,
  với `DATABASE_URL` do lane lấy từ `VINK_TEST_DATABASE_URL`). Hai test mới kiểm nháp rỗng/
  chưa title và mỗi HTTP reject không đổi dữ liệu/version/timestamp. CAS tám writer có đúng
  một winner; title, document, version cuối khớp winner. Stale save không ghi, foreign actor
  không đọc/list/ghi, Unicode/khổ/thụt lề round-trip. Public API/list/SSR chỉ trả snapshot đã
  được trỏ; revision UPDATE và pointer sang work khác bị từ chối.
- **PASS** 6 shared-store migration/error-class tests và 4 Identity database tests.
- **PASS** build binaries, config exit codes, provision/adopt database cũ, rerun không đổi
  password, quyền runtime/migrator và ca âm quyền CREATE, worker start/SIGTERM.
- **FAIL** bước `identity serve on its socket: ready, honest capabilities, contract refusals,
  deadline`. Log thật: `cannot bind …/target/verify-identity/sock/identity.sock: Operation not
  permitted (os error 1)`. Đây là giới hạn môi trường ở owner không đổi, không phải bằng chứng
  Identity chạy thành công. Các bước tích hợp socket/backup-restore phía sau chưa chạy.

## Review và tương thích

Một lượt review độc lập trên toàn diff/callers không tìm thấy finding P1/P2 có thể hành động.
Reviewer đối chiếu constructor/serde, owner/version SQL, lỗi, hash, public read và consumer
của abstraction; SHA-256 golden vector được tính lại độc lập từ literal canonical bytes và
khớp. `git diff --check` pass. Không có sửa code theo finding cần chạy lại khác biệt.

Production delta chỉ thêm `DraftChange` và thay nhóm tham số port/PgStore bằng accessor tương
đương. `SaveDraft` public input Rust, HTTP contract/fixtures, migrations, Identity, frontend,
UI System và deployment không đổi. Chữ trước đây hợp lệ vẫn dùng cùng parser/limits. Không
thêm stored-hash parser khi chưa có consumer.

## Chưa được chứng minh

- Full `quick`/`postgres` không được gắn PASS nếu một bước bị chặn; cần môi trường hỗ trợ Unix
  socket để chạy trọn các kiểm tra Identity hiện có. Không sửa oracle để hợp thức hóa sandbox.
- Browser/IME thật, paste Word/Docs, mobile/screen reader: NOT_RUN; không đổi frontend.
- Auth authority, submit, moderation, publish/withdraw và Discourse: chưa triển khai, không
  được suy ra từ seed/test resolver hay việc lưu nháp thành công. Không có model Quint.
- Không merge, auto-merge hoặc deploy trong nhiệm vụ này.
