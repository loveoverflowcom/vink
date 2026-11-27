# #607 · Xác minh email khi tạo tài khoản

Cặp DB PostgreSQL 17.11 riêng trên Mac mini. App, Identity và Chrome chạy trên loopback. Outbox local thay cho hộp thư bên ngoài. Base develop `52958d910eeacc705d5cca60e35bb153a754958b`. Cổng production và dữ liệu đang phục vụ độc lập với các lượt kiểm này.

| AC | Owner và bằng chứng |
| --- | --- |
| 01 | [PRD ACC-03, DEL-02](../../../../prd/requirements.md#146-tài-khoản-acc-03), [ADR D-05](../../../architecture/auth-authority.md), [TOML](../../../../decisions/proposals/VDR-AUTH-VERIFIED-EMAIL.r1.toml) và hai hợp đồng v1. `decision validate` và `decision refs` xác nhận 157 revision, 100 tham chiếu khai báo, không có lỗi tham chiếu. Nguồn #607 xác nhận luật kinh doanh. Revision TOML còn chờ signoff đúng hash. |
| 02, 03 | [Test PostgreSQL đăng ký và nâng cấp](../../../../services/vink-identity/tests/postgres/registration.rs). Kiểm mã sai, hết hạn, dùng sai purpose hoặc mailbox, hai completion đồng thời, email đã có chủ, và một phiên duy nhất. Kiểm rollback tại commit nếu thiếu email hoặc thêm email vào tombstone. |
| 04 | [Rehearsal trên cặp DB thật](../../../../xtask/src/identity/verified_email_cutover.rs) dùng chính CLI inventory/retire của Identity và reconciler App qua Unix socket. Trước cutover có 3 subject, 2 vi phạm, một subject đã có principal và phiên. Retry còn 1 subject hợp lệ và 0 vi phạm. Migration từ chối trong khi App chưa xác nhận. Sau drain, App có 2 biên nhận đã purge; binding và handle đã cấp vẫn thuộc chủ cũ, địa chỉ và thời điểm xác minh của subject hợp lệ được bảo toàn. |
| 05, 06 | App trả email của principal và phiên hiện tại với `no-store`. Ảnh cài đặt phía dưới ghi nhận email thử đã xác minh. Browser kiểm address của người đăng ký bằng mật khẩu và tài khoản OTP sau enrollment, quan sát kích cỡ mục cài đặt, overflow, focus, contrast, IME, autofill, lỗi và mất ACK. |
| 07 | Web SSR thật, AuthGate trên /toi và Reader, chặn đăng nhập khi tắt cổng, session switching và xóa tài khoản qua hai phiên độc lập. Test account-deletion buộc App rollback, sau đó phục hồi để xử lý tiếp biên nhận, kiểm liên kết tác giả cũ nhận 410. |
| 08 | [Runbook cutover](../../auth-operations.md#cutover-email-bắt-buộc-607) ghi thứ tự đóng intake, sao lưu, đối chiếu, drain và migration. Lifecycle kiểm phân quyền riêng của owner, relay/replay Notifications và backup/restore từng owner. |

## Hình ảnh

- [Ảnh trước do chủ sản phẩm cung cấp](https://drive.google.com/file/d/1w8Odx3zWQK6vF-l0GYFqsayXnLZKu6ws/view) cho thấy hàng email trống và nút thêm email. Nguồn ảnh không cung cấp SHA xác thực.
- [Cài đặt đã đăng nhập, dark, 390 × 844, chữ 150%](account-settings-ssr-390-dark-150.png). SHA-256 `0fdc1904b7e64b2564a9f0ea0ec66d0be0bbe6b5bb6b27130fc4d493abc70d7b`.
- [Đăng ký khách, light, 390 × 900, chữ 100%](auth-password-registration-390-light-100.png). SHA-256 `8ae0deeee908b47cba2aed0a4cfe40777b9e3971f75be2707e722588a67e4e4d`.

Hai ảnh sau được tạo bằng Chrome trên cây nguồn làm việc trước commit `7033ae9a91b59e1f37df8aaba1f368065ed86b55`. Bản sửa `c20b0003` loại phản hồi gửi lại mã đến muộn sau khi người dùng đã quay lại. Bố cục và nội dung ở hai trạng thái chụp được giữ nguyên.

## Lệnh đã chạy

Tại checkout chính trên macOS, `CARGO_BUILD_JOBS=1` và `RUST_TEST_THREADS=1`. Các lane Rust bên dưới dùng `python3.13 -B tools/local-budget run --low-disk --`. Các command PostgreSQL chỉ nhận URL cụm scratch dành riêng cho #607.

| Lệnh | Phạm vi và kết quả |
| --- | --- |
| `cargo xtask verify postgres lifecycle` | PASS. CLI và hai tiến trình thật chạy rehearsal trên schema nguồn đến `20261205000000`, rồi áp migration mới. 195 s, RSS lấy mẫu 2.718.176 KiB. |
| `cargo xtask verify postgres identity` | PASS, 55/55. Bao gồm migration, challenge/outbox, đăng ký song song, enrollment và xóa. |
| `cargo xtask verify postgres app` | PASS, 319 passed, 7 phép đo/benchmark opt-in được bỏ qua. |
| `cargo test --locked -p vink-app -p vink-identity -p vink-web --lib` | PASS, lần lượt 445, 45 và 325 test. Một test Web opt-in được bỏ qua. |
| `cargo clippy --locked -p vink-identity -p vink-app -p vink-web -p xtask --all-targets -- -D warnings` | PASS. |
| `target/debug/xtask verify web --scenario auth --scenario password --scenario account-deletion --capture full` | PASS với real Chrome, App, Identity, PG và outbox. Runner browser nguyên bản chạy riêng với khóa slot và đo RSS cây tiến trình. Đỉnh lấy mẫu 11.477.120 KiB. Màn guest kiểm 320, 390, 834, 1440 ở 100/150/200%, light/dark. |
| `cargo fmt --all -- --check`, `git diff --check`, `decision validate` và `decision refs` | PASS. Hai script smoke disposable được kiểm cú pháp bằng Bash và Node. Image OCI và đường edge ngoài browser scratch chưa được chạy trong lượt này. |

Rehearsal lifecycle được triển khai ở `7033ae9a`. Các lane PostgreSQL đã chạy trên nội dung tương ứng của phần Identity/App trong `c20b0003`. Clippy và unit tests đã chạy cùng phần nguồn ở `29c83796`. Revision sau đó chỉ thêm dữ liệu fixture va chạm chuẩn hóa và làm rõ thứ tự backup so với `c20b0003`. Các kết quả bổ sung tại head được ghi trong PR và issue.
