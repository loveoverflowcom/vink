# Auth Web: đăng nhập mã email, bút danh và trạng thái phiên

Trạng thái: **đã triển khai cho Web, kiểm chứng trong môi trường thử** (App + Identity thật, outbox
cục bộ, Chrome headless trên loopback). D-05 vẫn **Provisional**; tài liệu này không mở đăng nhập
production — các điều kiện còn mở nằm ở [ADR §9](../architecture/auth-authority.md) và
[runbook §3](auth-operations.md).

Phạm vi: email → mã → bút danh lần đầu → Bàn viết → đăng xuất, trên các API Auth đã có
([API v1](../../services/vink-app/contracts/http/v1/README.md#sign-in-d-05)). Không thêm route,
bảng hay cookie mới.

## 1. Một nguồn trạng thái phiên

| Phần | Nơi | Vai trò |
|---|---|---|
| `SessionOwner`, `SignInFlow`, `AuthError`, `text_survives` | [`session.rs`](../../apps/vink-web/src/session.rs) | Quyết định thuần (không HTTP, không reactive), có unit test |
| `Session` (`standing`, `principal`, `scope`) và vùng Tài khoản | [`account.rs`](../../apps/vink-web/src/account.rs) | Handle reactive do shell cấp một lần cho mỗi tab; chuyển câu trả lời giữa API và reducer |
| Client Auth | [`api.rs`](../../apps/vink-web/src/api.rs) | Gọi `/api/v1/auth/*`, phân loại lỗi theo status + `code` + `field` + `Retry-After` |

Quy tắc:

- **Chỉ server quyết định.** Trạng thái (`Unknown`, `Anonymous`, `NeedsPrincipal`, `Suspended`,
  `Active`, `Unavailable`) chỉ đổi theo câu trả lời của lần hỏi mới nhất; lần hỏi cũ trả lời muộn bị
  bỏ, nên một “anonymous” chậm không gỡ được lần đăng nhập vừa xong.
- **Generation.** Mỗi lần principal được xác nhận thay đổi (A → không ai, không ai → B, A → B) sau
  lần xác nhận đầu, generation tăng. `scope` = principal + generation; mọi view riêng tư khóa theo
  `scope` và bỏ câu trả lời thuộc scope cũ. Quay lại đúng tài khoản cũ vẫn là generation mới.
- **Lỗi tạm thời giữ trạng thái đã xác nhận.** Server tự xét quyền từng request; một lần `503` hay
  mất mạng không đăng xuất trang và không bỏ chữ đang viết.
- **Không lưu gì ở trình duyệt.** Credential, địa chỉ email, mã không vào `localStorage`,
  `sessionStorage` hay SSR. HTML render phía server luôn là khách; phiên được hỏi sau hydration.
- **Khi nào hỏi lại:** sau hydration, khi cửa sổ lấy lại focus, khi một request riêng tư trả `401`,
  sau mỗi thao tác đăng nhập/đăng xuất/tạo bút danh, và khi tab khác báo có thay đổi. Tab khác chỉ
  nhận một tín hiệu `BroadcastChannel` **không mang dữ liệu** rồi tự hỏi server.

Nơi dùng nguồn này: avatar và vùng Tài khoản trong “Tôi”, Bàn viết (danh sách nháp, editor, thẻ
nháp), thảo luận và ghi nhận văn học trên trang đọc. Không view nào tự gọi `GET /auth/session`.

## 2. Luồng

1. **Xin mã** — `POST /auth/sign-in/challenges` (`202`, cookie giao dịch HttpOnly). Trang chuyển
   sang bước nhập mã và hiện thời hạn của mã.
2. **Nhập mã** — `POST /auth/sign-in/complete`. Thành công: hỏi lại phiên.
3. **Bút danh lần đầu** — phiên hợp lệ nhưng chưa có principal (`NeedsPrincipal`):
   `POST /auth/principal`. Gửi lặp lại trả `200` với cùng principal, không tạo tài khoản thứ hai;
   reload giữa chừng quay lại đúng bước này.
4. **Bàn viết** — `Active`: tạo, lưu, đọc lại nháp bằng các route nháp đã có.
5. **Đăng xuất** — `DELETE /auth/session` trả `204 No Content`; decoder không đọc JSON cho câu trả
   lời không có thân. Chỉ khi server xác nhận thu hồi (`204`) trang mới coi là đã đăng xuất. Câu
   trả lời khác hoặc không có câu trả lời: hiện **“Chưa đăng xuất xong”** — phiên có thể còn hiệu
   lực trên thiết bị này; nút “Đăng xuất” vẫn ở đó để thử lại và trang không tự chuyển sang khách.

## 3. Lỗi và cách phục hồi

Thông điệp do client viết, không lặp lại chữ của server. Địa chỉ email và mã đã gõ được giữ trong ô
nhập ở mọi trường hợp dưới đây.

| Câu trả lời | Ý nghĩa | Phục hồi trên trang |
|---|---|---|
| `422` trên `email` / `code` / `pen_name` | Dữ liệu nhập chưa hợp lệ | Sửa tại chỗ; mã không đủ 6 chữ số bị chặn trước khi gửi |
| `422 code_rejected` | Mã sai, challenge còn nhận lần thử khác | Nhập lại |
| `409 challenge_closed` | Mã hết hạn, đã dùng, hết lượt hoặc không còn | Xin mã mới |
| `403 account_disabled` | Identity khóa đăng nhập địa chỉ này | Liên hệ Vink; không thử lại |
| `403 principal_suspended` | App đình chỉ principal | Thông báo tạm khóa |
| `429` | Quá nhiều yêu cầu | Nút xin/gửi lại mã khóa theo `Retry-After` (giây, làm tròn lên); không có header thì “chờ ít phút” |
| `501 authority_not_configured` | Deployment chưa mở đăng nhập | “Vink chưa mở đăng nhập”; không có nút thử lại vô nghĩa |
| `503` | Identity hoặc directory tạm thời không trả lời | Giữ chữ đã nhập, thử lại sau |
| `401` | Phiên đã hết trước request này | Hỏi lại phiên, mời đăng nhập lại |
| Không có câu trả lời | Mạng; thao tác đổi phiên có thể đã hoặc chưa áp dụng | Hỏi lại phiên trước khi kết luận |

Gửi lại mã dùng đúng các lỗi này: khoảng chờ cục bộ chỉ là gợi ý, `Retry-After` của server thắng.

## 4. Chữ đã gõ và tách dữ liệu giữa tài khoản

`text_survives(typed_as, now)`: chữ ở lại trang khi vẫn là **cùng một người** — khách rồi đăng
nhập, phiên hết, đúng tài khoản đó đăng nhập lại. Chữ **không bao giờ** sang tài khoản khác, kể cả
khi giữa hai tài khoản có một lúc đăng xuất.

| Tình huống | Hành vi |
|---|---|
| Phiên hết khi đang viết nháp | Editor giữ nguyên chữ; lưu báo “Phiên đăng nhập đã hết”; đăng nhập lại đúng tài khoản rồi lưu lại là thành công. Không tự lưu vào storage |
| Đăng xuất ở tab khác | Tab đang viết giữ chữ chưa lưu, chuyển sang trạng thái khách |
| A → B trên cùng trình duyệt | Mọi tab: danh sách nháp và thông báo của A rời màn hình ngay khi scope đổi; editor được dựng lại từ lần đọc của B (chữ của A và mọi câu trả lời còn trên đường đi theo editor cũ); nháp của A trả `404` cho B |
| Câu trả lời của A về muộn | Bị bỏ vì khác scope; không cập nhật dữ liệu riêng tư của B |
| Thảo luận/ghi nhận văn học | Chữ bình luận ở lại cho cùng người; lệnh đang bay hoặc chưa chắc không còn được theo dõi và không bao giờ replay dưới phiên khác; receipt bị bỏ; số đếm và trạng thái riêng được đọc lại |
| Reload | Trạng thái dựng lại hoàn toàn từ server |

## 5. Kiểm chứng

`cargo xtask verify web` có thêm bước **Auth Web**: dựng database và role Identity tạm
(`vink_verify_web_identity`), chạy `vink-identity migrate` + `serve` thật với
`VINK_IDENTITY_AUTHORITY=email-code` và outbox cục bộ, chạy App thật **không** có
`VINK_DEV_PRINCIPAL`, rồi lái Chrome qua
[`auth-smoke.mjs`](../../apps/vink-web/e2e/auth-smoke.mjs). Mã được đọc từ file outbox
(`X-Vink-Code`), không qua mail thật.

| Kịch bản | Nguồn câu trả lời |
|---|---|
| Khách: SSR, “Tôi”, Bàn viết không lộ dữ liệu riêng | Thật |
| Người mới: xin mã, nhập mã, bút danh, reload giữa chừng, gửi lặp tạo principal → cùng principal | Thật |
| Tạo, lưu, reload, đọc lại nháp | Thật |
| Mã sai → `422`; sau 5 lần → `409` và xin mã mới | Thật |
| `429` kèm `Retry-After` (giới hạn theo mạng của App) | Thật |
| `501`, `503` khi xin mã (địa chỉ được giữ) | **Dựng** bằng `Fetch.fulfillRequest`, có nhãn trong log |
| Đăng xuất `503` → “chưa hoàn tất”, phiên vẫn còn | **Dựng** |
| Đăng xuất `204`; tab khác giữ chữ chưa lưu, lưu báo hết phiên | Thật |
| Người cũ quay lại: cùng principal, lưu lại thành công | Thật |
| A → B với hai/ba tab, câu trả lời của A bị giữ rồi thả muộn, reload | Thật (câu trả lời thật bị giữ ở trình duyệt) |

Quyết định thuần có unit test trong `session.rs` (`cargo test -p vink-web --lib --features ssr`).

## 6. Giới hạn

- Môi trường thử chạy `http://127.0.0.1`; Chrome chấp nhận cookie `__Host-`/`Secure` trên loopback.
  **HTTPS thật, domain thật, edge/WAF và thư tới hộp thư người thật chưa được chạy.**
- Chỉ Chrome headless. Chưa có Safari/Firefox, thiết bị mobile thật, screen reader, trình quản lý
  mật khẩu/autofill mã.
- Riêng thảo luận đã supersede/cancel private reads và release loading/mutation owner khi scope
  đổi ([Reader reads](discussion-web-reads.md)). Các view nháp hiện vẫn chạy nối tiếp: sau khi đổi tài khoản, lần đọc của tài khoản mới có thể
  phải chờ câu trả lời chậm của tài khoản cũ, tối đa bằng hạn request (30 s). Màn hình đã được dọn
  và ở trạng thái đang tải trong lúc đó; không có dữ liệu cũ nào được hiện.
- Chưa có đổi bút danh, đổi email, danh sách phiên/thiết bị trên UI.
- `/toi` vẫn dùng mã email và hiển thị “Vink không dùng mật khẩu”. Đăng ký, đăng nhập và thêm
  username/mật khẩu đã có ở backend. Giao diện mật khẩu và cập nhật câu trên còn thiếu.
  [Hợp đồng App](../../services/vink-app/contracts/http/v1/README.md#sign-in-d-05)
- CMP/native chưa có luồng đăng nhập.
- Đăng nhập production vẫn khóa (`VINK_AUTH_SIGN_IN` mặc định `disabled`); D-05 chưa Accepted.
