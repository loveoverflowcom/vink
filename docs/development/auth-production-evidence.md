# Bằng chứng đường production của D-05 (10/11/2026)

Bằng chứng cho nhánh `codex/identity-production-backend` theo [ADR D-05 §11](../architecture/auth-authority.md#11-đường-production-10112026)
và [runbook](auth-operations.md). Kế tiếp [bằng chứng spike S-03](auth-authority-s03-evidence.md).
**Không bước nào dưới đây là đăng nhập production**; gửi thật chỉ tới hộp thư mô phỏng của Resend (#29): D-05 vẫn Provisional,
đăng nhập production khóa bằng `VINK_AUTH_SIGN_IN`.

**Base:** `origin/develop@755a635`. **Môi trường:** macOS 26 (Darwin 25.5, arm64), Rust 1.99.1
(`rust-toolchain.toml`), PostgreSQL 17.11 (Homebrew, cluster tạm `127.0.0.1:55432`, đã dừng sau khi
kiểm), Google Chrome cho `verify web`. Credential test được che.

## Mức bằng chứng

**U** unit/contract (`verify quick`, không DB) · **S** adapter SMTP với relay giả qua TCP loopback
(giao thức thật, không TLS) · **P** PostgreSQL thật (`#[sqlx::test]`, `verify postgres`) · **E** hai
binary thật + DB đã provision + Unix socket thật (`verify postgres`, bước D-05) · **NOT_RUN** chưa có.

## Lane

| Lệnh | Kết quả | Ghi chú |
| --- | --- | --- |
| `cargo xtask verify quick` | PASS 9/9 | fmt, clippy server/wasm/preview, doctest biên kiểu, unit + contract (Identity 32 lib + 2 bin; App 235 lib), repository checks |
| `VINK_TEST_DATABASE_URL=… cargo xtask verify postgres` | PASS 28/28 | Gồm App 96 test PostgreSQL (mới: `abuse`, `commit_fence`, fence discussion/endorsement), Identity 22 test (mới: `delivery`), provisioning/role thật, **bước D-05 hai binary thật** (khóa kích hoạt, worker outbox, giới hạn mạng sau proxy tin cậy, hồi quy S-03, SMTP loopback với xoay mật khẩu và relay phục hồi), log không lộ secret, backup/restore |
| `cargo xtask verify kani` | PASS (App 28 harness + expressions) | Không đổi chính sách thuần có harness; chạy để chắc manifest không lệch |
| `VINK_TEST_DATABASE_URL=… VINK_CHROME=… cargo xtask verify web` | PASS 9/9 | Hồi quy trình duyệt cho shell/editor/reader (đường lưu nháp nay đi qua fence trong transaction) |

Lịch sử chạy `verify postgres`: lần 1 FAIL ở test SSR `server_html_has_public_content_and_never_private_drafts`
— **lỗi có sẵn trên `develop`** (đã chạy lại trên `origin/develop@755a635` nguyên bản: cùng FAIL):
chuỗi "Đã tải 1 tác phẩm · tối đa 12." đổi thành "N tác phẩm trên trang này." ở `bfd055f` (#100) mà
test chưa cập nhật; PR này sửa assertion. Lần 2 FAIL ở bước binary vì worktree dùng
`CARGO_TARGET_DIR` khác chỗ xtask tìm binary (môi trường, không phải mã). Lần 3 PASS 28/28 trên cây
cuối cùng trước khi bổ sung kịch bản lease. Lần 4 PASS 28/28 trên cây cuối (sau seam `MailSender`, preset Resend, `mail test`, phân loại lỗi TLS/xác thực).

## Kịch bản

| # | Kịch bản | Kết quả | Bằng chứng |
| --- | --- | --- | --- |
| 1 | Challenge và ý định gửi commit cùng nhau; bị throttle thì không để lại gì | PASS | P `a_challenge_commits_with_its_delivery_and_the_worker_sends_it_once` |
| 2 | Mạng gửi thư nằm ngoài transaction; request không phụ thuộc relay | PASS | P (worker claim → commit → gửi); E relay `421`: API vẫn `202`, mã tới sau khi relay phục hồi |
| 3 | Lỗi tạm thời: backoff có giới hạn, phục hồi cùng mã | PASS | U `outcomes_are_bounded_and_backoff_grows_to_its_cap` (vét cạn lần × loại lỗi); P `transient_failures_back_off…` |
| 4 | `5xx` dừng ngay; tối đa 5 lần; mã bị xóa ở trạng thái cuối | PASS | S/P `…permanent_ones_end_at_once`, `attempts_are_bounded…` |
| 5 | Mã đã dùng/hết hạn không bao giờ được gửi | PASS | U `used_or_expired_codes_are_never_sent`; P (worker sập > 10 phút → `cancelled`) |
| 6 | Worker crash / lease / gửi trùng | PASS (at-least-once) | P `a_crashed_worker_is_recovered_after_its_lease_at_least_once`: trong lease không ai gửi; hết lease gửi lại; claim cũ không ghi đè; crash sau khi relay nhận → đúng một bản trùng cùng mã, cùng `Message-ID` |
| 7 | Nhiều worker/instance không gửi một claim hai lần | PASS | P `concurrent_workers_never_send_one_claim_twice` (40 thư, 4 worker, mỗi thư đúng 1 lần) |
| 8 | Worker thức ngay sau commit, dừng sạch khi shutdown | PASS | P `the_running_worker_wakes_on_commit_and_stops_on_shutdown` |
| 9 | SMTP: phân loại 4xx/5xx/timeout/không kết nối; không giữ văn bản relay (có địa chỉ) | PASS | S `smtp_delivers_and_classifies_relay_answers` |
| 10 | Mật khẩu SMTP chỉ qua file `0600`, đọc lại mỗi lần gửi (xoay không restart), không vào log/Debug | PASS | U `smtp_settings_validate…`, `smtp_requires_tls_off_loopback…`; E xoay file giữa hai lần đăng nhập, relay thấy mật khẩu mới; log không chứa mật khẩu |
| 11 | TLS bắt buộc ngoài loopback; cấu hình sai thoát `2` không in secret | PASS | U config; E (binary thật khởi động với SMTP loopback) |
| 12 | Giới hạn theo mạng đếm chính xác khi đồng thời, chung giữa hai process | PASS | P `concurrent_requests_from_two_processes_are_counted_exactly` (40 request, 2 pool, đúng 10 qua) |
| 13 | Xoay địa chỉ trong một /64 không thoát giới hạn | PASS | U `networks_group_ipv6_by_64…`; P cùng test |
| 14 | `X-Forwarded-For` giả từ client trực tiếp bị bỏ qua; sau proxy tin cậy lấy hop ngoài cùng bên phải | PASS | U `forwarded_for_is_believed_only_from_trusted_proxies`, `forwarded_for_selects_the_bucket_only_behind_a_trusted_proxy`; P `the_edge_answers_429…` (header giả không tạo bucket mới) |
| 15 | Bị từ chối thì không gọi Identity; bộ đếm sập = `503`; thiếu peer = `503` | PASS | U `sign_in_is_counted_per_network_before_identity_and_fails_closed`; P `the_edge_answers_429_with_retry_after…` |
| 16 | `429` + `Retry-After` qua binary thật | PASS | E (11 lần xin mã từ một client sau proxy: lần 11 `429`, `Retry-After` 1–900; client khác không bị ảnh hưởng) |
| 17 | Trần toàn cục (mềm) và theo tài khoản (chính xác) | PASS | P `global_and_per_account_ceilings_hold` |
| 18 | Đổi email không dò được địa chỉ đã đăng ký | PASS | P (`a_verified_email_change…`: địa chỉ của người khác trả như địa chỉ trống, không có thư/không có hàng delivery) |
| 19 | Khóa kích hoạt production | PASS | U `sign_in_activation_is_explicit…`, `disabled_sign_in_keeps_logout…`; E App production không `VINK_AUTH_SIGN_IN` → `501` |
| 20 | Đăng nhập lại thu hồi phiên cũ của trình duyệt | PASS | U `disabled_sign_in_keeps_logout_and_a_new_sign_in_replaces_the_old_session` |
| 21 | Đình chỉ đua với ghi được bảo vệ: nháp | PASS | P `draft_writes_are_refused_at_commit_once_suspended`, `draft_saves_and_suspensions_serialize_on_the_principal` (cả hai thứ tự, `pg_blocking_pids`) |
| 22 | Đình chỉ tại commit: xóa bình luận, đổi chế độ, kiểm duyệt | PASS | P `every_discussion_write_is_refused_at_commit_once_suspended`, `a_moderation_decision_waits_for_a_pending_suspension_and_refuses` |
| 23 | Đình chỉ tại commit: ghi nhận văn học | PASS | P `a_suspended_reader_cannot_endorse_and_a_pending_suspension_is_waited_for` |
| 24 | Không deadlock do fence mới | PASS (trong phạm vi test) | Toàn bộ test đồng thời có sẵn của discussion/endorsement/taxonomy chạy lại với fence mới; thứ tự khóa ghi ở module comment mỗi adapter |
| 25 | Prune chỉ xóa bản ghi đã đóng; idempotent | PASS | P `prune_deletes_only_closed_records`, `prune_and_rollback_touch_only_rate_limit_windows` |
| 26 | Rollback không mất tài khoản/phiên/principal | PASS | P `the_rollback_script_drops_only_the_outbox`, `prune_and_rollback…` (chạy đúng file `migrations-rollback/*.down.sql`) |
| 27 | Hồi quy luồng S-03 (đăng ký, binding, cô lập, logout, đổi email, đình chỉ, disable, Identity sập, đọc công khai) | PASS | E bước D-05 (mã đọc từ outbox qua worker) |
| 28 | Contract v1 và SSR công khai không hồi quy | PASS | U fixtures; P toàn bộ `tests/postgres.rs`; `verify web` |
| 29 | Gửi thật qua Resend: `mail test` (binary thật, TLS thật) | PASS (relay chấp nhận) | `smtp.resend.com:587` STARTTLS, user `resend`, API key từ file `0600`, From `onboarding@resend.dev` (sender thử của Resend), To `delivered@resend.dev` (hộp thư mô phỏng của Resend) |
| 29b | Gửi thật qua Resend: luồng đầy đủ `serve` → challenge → outbox → worker | PASS | Contract `202` sau 3,5 ms (không chờ mạng); thư `sent` ở lần 1 sau ~4,5 s (gồm vòng poll), mã đã xóa, `mail status` `pending 0`; readiness `smtp`/`not_accepted`; log không chứa key, mã hay địa chỉ; SIGTERM dừng worker gọn |
| 29c | Chứng chỉ giả bị từ chối | PASS (phát hiện môi trường) | Mạng văn phòng: Fortinet chặn TLS cổng 465 và ký lại chứng chỉ → client từ chối, báo `tls` (sau khi sửa phân loại; trước đó báo nhầm `connect`). Key sai → `configuration` (535, có retry) |
| 29d | Thư tới hộp thư người thật, domain Vink đã xác minh SPF/DKIM/DMARC, không vào spam | **NOT_RUN** | Domain gửi của Vink chưa xác minh ở Resend; cần hộp thư thử do đội sở hữu (runbook §4) |
| 30 | Trình duyệt thật đăng nhập (cookie `__Host-`, HTTPS) | **NOT_RUN** | Chưa có UI đăng nhập |

## Negative control

Mỗi đột biến áp tạm vào mã sản xuất, chạy đúng test liên quan, rồi khôi phục (kiểm `git diff` rỗng).

| Đột biến | Test | Kết quả |
| --- | --- | --- |
| Bỏ fence khi lưu nháp | `draft_writes_are_refused_at_commit_once_suspended` (P) | FAIL như mong đợi |
| Bỏ fence ở quyết định kiểm duyệt | `every_discussion_write_is_refused_at_commit_once_suspended` (P) | FAIL như mong đợi |
| Bỏ fence ở ghi nhận | `a_suspended_reader_cannot_endorse…` (P) | FAIL như mong đợi |
| `finish` bỏ kiểm `lease_token` | `a_crashed_worker_is_recovered…` (P) | **Sống sót lần đầu** (hàng đã `sent` nên điều kiện trạng thái che đi). Đã thêm kịch bản claim cũ đua claim mới khi hàng còn `pending` → FAIL như mong đợi |
| Claim không khóa hàng (bỏ `FOR UPDATE SKIP LOCKED`) | `concurrent_workers_never_send_one_claim_twice` (P) | FAIL như mong đợi |
| Đổi email vẫn gửi thư tới địa chỉ của người khác | `a_verified_email_change…` (P) | FAIL như mong đợi |
| Tin `X-Forwarded-For` từ mọi peer | `forwarded_for_*` (U) | FAIL như mong đợi |
| Bộ đếm sập thì cho qua (fail-open) | `sign_in_is_counted_per_network…` (U) | FAIL như mong đợi |
| Bỏ trần toàn cục | `global_and_per_account_ceilings_hold` (P) | FAIL như mong đợi |

Kịch bản đột biến nằm ngoài repository (scratchpad của phiên), áp bằng thay chuỗi chính xác và khôi
phục từ bản sao (đã so nội dung).

## Đo đạc (cục bộ, không phải SLA)

Máy dev, PostgreSQL cục bộ, debug build — chỉ để thấy bậc độ lớn, không phải SLA:

| Điểm đo | Kết quả |
| --- | --- |
| Đếm rate limit (một upsert), 200 lần tuần tự | p50 0,12 ms · p95 0,39 ms · max 0,73 ms |
| 40 lần đếm đồng thời qua 2 pool (cùng một khóa, tranh khóa hàng) | 23,7 ms tổng; đếm đúng 40 |
| Commit challenge → relay loopback nhận thư (worker đang chạy, đánh thức bằng Notify) | 5–27 ms |
| 40 thư, 4 worker, relay loopback (40 kết nối SMTP) | ~35 ms |

Không đo RAM riêng (worker giữ tối đa một batch 16 thư; không có cache). Fence thêm một `SELECT …
FOR UPDATE` trên hàng principal mỗi lệnh ghi; mọi test đồng thời có sẵn vẫn PASS, không quan sát
thấy deadlock.

## Giới hạn và rủi ro tồn dư

- **At-least-once:** crash sau khi relay nhận có thể gửi trùng một lần (cùng mã). Thư relay đã nhận
  vẫn có thể bị mất/vào spam; chưa xử lý bounce/complaint.
- **Trần toàn cục mềm:** request đồng thời có thể vượt tối đa bằng số request đồng thời.
- **Khóa theo mạng:** người dùng chung NAT/văn phòng có thể bị một người lạm dụng làm chặn tới 15
  phút; trần theo địa chỉ (3/15 phút) vẫn có thể bị người khác dùng cạn (có từ spike, nay bị giới
  hạn thêm theo mạng).
- **Hai database:** disable ở Identity không fence được request App đang chạy; đình chỉ ở App thì có.
- **Không đo thống kê thời gian** cho enumeration; đổi email sang địa chỉ người khác bỏ qua một lệnh
  INSERT (chênh vi giây).
- **Không chống DDoS thể tích** trong App; cần edge/WAF.
- Đường TLS thật (rustls + webpki roots) đã chạy với Resend qua 587; cổng 465 chưa chạy được từ mạng
  văn phòng do firewall chặn TLS. Thư mới tới hộp thư mô phỏng của Resend; deliverability vào hộp
  thư thật với domain Vink chưa kiểm.
