# Bằng chứng spike S-03 / D-05 — auth authority

Bằng chứng chạy thật cho nhánh `codex/identity-s03-spike` theo
[ADR D-05 (Provisional)](../architecture/auth-authority.md). Mỗi mục ghi lệnh, kết quả và giới
hạn. Credential test được che (`***`). SHA của commit chứa tài liệu này được báo ở PR, không ghi
vào chính commit.

**BASE_SHA:** `9dc4991620ef93b09c21fdba355a3c3f557da608` (`origin/develop`, PR #86). Mọi lệnh chạy
trên cây làm việc = base + diff của nhánh, sau sửa đổi code cuối cùng.

**Môi trường:** Linux 7.0.0-43-generic x86_64; Rust 1.99.1 (`rust-toolchain.toml`); Node 24.22.2;
Google Chrome 157.0.8103.38; Podman 5.7.0. PostgreSQL tạm: container
`docker.io/library/postgres:17.11-alpine` chỉ nghe `127.0.0.1:55463`,
`VINK_TEST_DATABASE_URL=postgres://vink_test:***@127.0.0.1:55463/postgres`; client `psql`/`pg_dump`
17.11 qua shim. Container đã xóa sau khi kiểm.

## Kết luận Go/No-Go

- **Go (provisional)** cho hướng A trong pilot: tiếp tục các lát cắt ở
  [ADR §10](../architecture/auth-authority.md#10-lát-cắt-tiếp-theo) trên seam này.
- **No-Go cho production**: thiếu mail transport, rate limit biên, UI + qualification trình duyệt,
  native, fence đình chỉ tại commit, review bảo mật độc lập và người chịu trách nhiệm bảo mật auth.
  Không bước nào dưới đây chứng minh xác thực production.

## Lane xtask

| Lệnh | Kết quả | Phạm vi |
| --- | --- | --- |
| `cargo fmt --all -- --check` | exit 0 (cũng là bước đầu của `quick`) | Định dạng |
| `cargo xtask verify quick` | exit 0; 9 bước PASS (gồm clippy server/wasm/preview, unit + contract, repository checks) | fmt, clippy, unit/contract Identity (27 lib + 1 bin) + App (147 lib: 10 test mới và 1 viết lại cho principal/auth/resolver; 1 bin mới), link/boundary/secret lint |
| `VINK_TEST_DATABASE_URL=… VINK_PG_BIN=… cargo xtask verify postgres` | exit 0 trên cây cuối; 28 bước PASS: store 6 test, App 52 test (5 mới), Identity 13 test (9 mới), provisioning/role, binary Identity, App + Identity, **S-03 e2e hai binary thật**, log không lộ secret, backup/restore | PostgreSQL thật, `provision.sh` thật, hai binary thật trên Unix socket thật |
| `VINK_TEST_DATABASE_URL=… cargo xtask verify web` | exit 0 trên cây cuối; 8 bước PASS (bundle không chứa URL/credential; SSR, hydration, editor, khởi động lại, ghi nhận văn học với LocalFixture). Một lần chạy trước bị dừng vì ổ đĩa máy đầy (không liên quan mã) | Hồi quy trình duyệt cho shell/editor/reader hiện có qua router đã thêm `/api/v1/auth/*`; **không** có UI đăng nhập để thử, lane dùng LocalFixture |

Lịch sử chạy `verify postgres` (giữ lại vì có phát hiện):

| Lần | Kết quả | Nguyên nhân / xử lý |
| --- | --- | --- |
| 1 | FAIL ở bước migrate Identity | xtask cố định danh sách migration một phiên bản; cập nhật kỳ vọng |
| 2 | FAIL ở `vink-app principal suspend` (exit 2) | Lệnh vận hành đọc cấu hình production cần `VINK_PUBLIC_ORIGIN`; test truyền đủ biến. Bước 1–4 của e2e đã PASS |
| 3 | không chạy (xtask không biên dịch: borrow) | Sửa xtask |
| 4 | PASS 28/28 | Trước khi thêm rào tiêu thụ mã và test khóa xác định |
| 5 | FAIL e2e: xin mã đầu tiên `503` (client hết hạn 1,5 s) | Log: thao tác ghi chậm trên đĩa vừa build. Bỏ `fsync` file outbox |
| 6 | FAIL e2e: hoàn tất đăng nhập `503` | Log Identity: `COMMIT` mất 1,48 s. **Phát hiện ngân sách thời gian**: tách hạn — `verify-credential` 2 s (client 1,5 s), thao tác ghi email-code 5 s (client 4,5 s); thông điệp `deadline_exceeded` không còn ghi cứng “2 seconds” |
| 7 | PASS 28/28 | Cây cuối |

## Kịch bản bảo mật

Mức bằng chứng: **U** unit/contract (`quick`), **P** PostgreSQL thật (`postgres`, `#[sqlx::test]`),
**E** hai binary thật + PostgreSQL đã provision + socket thật (`postgres`, bước
`S-03 email-code spike`). `NOT_RUN` = chưa có bằng chứng chạy.

| # | Kịch bản | Kết quả | Bằng chứng |
| --- | --- | --- | --- |
| 1 | Email chưa xác minh không có quyền | PASS | Không có trạng thái "chưa xác minh": tài khoản chỉ được tạo khi mã gửi tới địa chỉ được trả đúng (P `first_sign_in…`); chủ thể mới vẫn `needs_principal`, mọi route riêng tư `401` (U `bindings_and_suspensions…`, E bước 1) |
| 2 | Subject/user_id/role giả bị từ chối | PASS | Request Identity/App có trường lạ bị từ chối (U `email_code_requests_are_validated…`, `auth_shapes_and_errors…`); header `x-user-id`/`x-vink-principal` không đổi gì (U `identity_answers_never_grant…`, E bước 2); subject sai contract → `503` (U) |
| 3 | Signup đồng thời không nhân đôi principal | PASS | 6 lần đăng nhập đầu đồng thời một tài khoản; 8 lần trả lời cùng challenge một phiên; 4 lần trả lời bị ép chờ cùng khóa hàng rồi thả ra vẫn một phiên (P); 12 lần provisioning đồng thời một principal, không principal mồ côi (P `concurrent_first_requests…`) |
| 4 | Cùng email khác subject không tự nối | PASS | App không nhận email; subject khác → principal khác, schema cấm gắn subject thứ hai (P `distinct_subjects…`, `bindings_cannot_move…`); địa chỉ được giải phóng thành subject mới chưa bind (P, E bước 4) |
| 5 | Đổi email hoặc bút danh giữ principal | PASS | Đổi email có xác minh giữ subject (P, E bước 4); đổi bút danh giữ binding và nháp (P `a_pen_name_change…` — đổi bằng SQL có nhãn, chưa có lệnh đổi tên) |
| 6 | Credential sai/hết hạn/phát lại bị từ chối | PASS | Mã sai/hết hạn/hết lượt/đã dùng/handle khác (P `codes_are_one_time…`; U bảng quyết định vét cạn); phiên hết hạn/giả (P `expired_sessions…`); challenge đã dùng không phát phiên mới (E mỗi lần đăng nhập) |
| 7 | Logout chặn thao tác sau | PASS | P `logout_revokes_only_that_session…`; E bước 3 (GET và POST sau logout `401`, thiết bị khác vẫn chạy) |
| 8 | Revoke hoặc đình chỉ làm mất quyền | PASS | Disable thu hồi mọi phiên (P, E bước 6); đổi email thu hồi phiên khác (P); đình chỉ App `401` ngay request kế tiếp và chặn đăng nhập (U, P, E bước 5) |
| 9 | Request cũ không khôi phục quyền đã thu hồi | PASS (phần) | Credential đã thu hồi không bao giờ hợp lệ lại, enable không khôi phục phiên (P); fence tại commit cho request **đang chạy** khi bị đình chỉ: NOT_RUN / chưa hiện thực |
| 10 | Identity timeout/outage đóng an toàn | PASS | Identity giả chậm/hỏng/sai uid (U); Identity thật dừng → `503`, xin mã `503` (E bước 7) |
| 11 | RPC sai và version không hỗ trợ bị từ chối | PASS | U `malformed_oversized…`, `versions_paths…`; lý do từ chối lạ → `503` ở App (U `333333`); binary thật (`postgres`, bước contract có sẵn) |
| 12 | Provisioning lỗi không gán sai chủ | PASS | Candidate trùng principal có sẵn → lỗi, không binding (P); directory lỗi lúc hoàn tất đăng nhập → thu hồi phiên, không cookie (U) |
| 13 | Cô lập nháp/bình luận/ghi nhận giữa tài khoản | PASS (nháp) | Nháp qua phiên email-code: P, E bước 2. Bình luận/ghi nhận đi qua cùng `Actor`; test cô lập theo principal có sẵn chạy với resolver test, **chưa** chạy qua phiên email-code: NOT_RUN |
| 14 | Đọc công khai còn khi Identity sập | PASS | E bước 7 (`/api/v1/works`, tổng số ghi nhận, `/`, reader) |
| 15 | Secret vắng mặt trong log và bundle | PASS (phần) | Body không chứa handle/credential (U, E); log ba tiến trình và HTML công khai không chứa mã/handle/credential nào đã thấy (E); `Debug` che (U). Bundle WASM: không có mã nào đọc cookie (HttpOnly), không kiểm bằng phân tích bundle |

## Negative control

Mỗi đột biến được áp tạm vào mã sản xuất, chạy đúng test liên quan rồi khôi phục (đã kiểm nội dung
khôi phục). Kết quả mong đợi: test **FAIL**.

| Đột biến | Test | Kết quả |
| --- | --- | --- |
| `verify` của Identity bỏ kiểm `revoked` | `logout_revokes_only_that_session_and_is_idempotent` (P) | FAIL như mong đợi (phiên đã logout vẫn hợp lệ) |
| `bind` của App cho `Suspended` thành `Principal` | `only_an_active_binding_resolves_to_a_principal` + 3 `identity_tests` (U) | FAIL như mong đợi |
| Provisioning App `commit` thay vì `rollback` khi thua xung đột | `concurrent_first_requests_provision_exactly_one_principal_without_orphans` (P) | FAIL như mong đợi (principal mồ côi) |
| Bỏ `FOR UPDATE` khi đọc challenge | `concurrent_completions…` (P) | **Đột biến sống sót** ở lần đầu: 8 tác vụ không chắc chồng lấn. Đã sửa: thêm rào `consumed_at IS NULL` (kiểm `rows_affected`) và test xác định `completions_queued_behind_a_row_lock…` (giữ khóa hàng tới khi 4 lần trả lời cùng chờ, đếm qua `pg_stat_activity`) |
| Bỏ `FOR UPDATE` (rào giữ nguyên) | `completions_queued_behind_a_row_lock…` (P) | PASS — rào thứ hai vẫn chỉ cho một phiên (phòng thủ kép đúng thiết kế) |
| Bỏ cả `FOR UPDATE` và rào | `completions_queued_behind_a_row_lock…` (P) | FAIL như mong đợi: 4 phiên cho một mã |

Kịch bản lệnh nằm ngoài repository (scratchpad của phiên); có thể lặp lại bằng cách sửa đúng dòng
nêu trên và chạy `DATABASE_URL=… cargo test -p vink-identity --test postgres <tên>` hoặc
`cargo test -p vink-app --lib identity`.

## Giới hạn

- Mail: outbox là thư mục cục bộ; không SMTP/API, không đo deliverability. Ước giá nhà cung cấp mail
  chưa có.
- Không có UI đăng nhập; cookie được client HTTP gửi lại đúng như App đặt — **trình duyệt thật chưa
  chạy** (thuộc tính `__Host-`/`Secure` trên origin thật, SameSite, hai tab).
- Không rate limit theo IP/thiết bị; throttle chỉ theo địa chỉ.
- Đình chỉ kiểm khi resolve request, chưa fence tại commit của từng lệnh ghi.
- Native/CMP: NOT_RUN (chưa có client).
- B (Keycloak) và C (Supabase/Clerk): chỉ so bằng tài liệu chính thức; runtime NOT_RUN, không tạo
  tài khoản hay tài nguyên trả phí.
- Hiệu năng của lời gọi Identity mỗi request: chưa đo.
