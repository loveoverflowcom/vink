# Auth Web: mật khẩu, mã email và trạng thái phiên

Trạng thái: **đã triển khai cho Web, kiểm chứng trong môi trường thử** (App + Identity thật, outbox
cục bộ, Chrome headless trên loopback). D-05 vẫn **Provisional**; tài liệu này không mở đăng nhập
production — các điều kiện còn mở nằm ở [ADR §9](../architecture/auth-authority.md) và
[runbook §3](auth-operations.md).

Phạm vi `AUTH-PASSWORD-WEB-01`: mật khẩu là lựa chọn đầu trên `/toi`; khách có thể tạo
tài khoản hoặc dùng mã email. Cả hai đi qua bút danh nếu máy chủ trả `needs_principal`.
Tài khoản dùng mã email có thể thêm mật khẩu bằng mã xác minh mới và giữ nguyên bút danh,
principal, bản nháp. Dùng [API Auth đã có](../../services/vink-app/contracts/http/v1/README.md#sign-in-d-05).
[Bố cục và tương tác](../design/auth-password-web.md) kế thừa trang Tôi.

Đối soát tại base `7d9dc54f63a2b759cc5979007dd4f665fe0ec86f`; snapshot registry SHA-256
`70a6a63ebbf19622599853050ca0910a3dbd99fdca9b4ceab4f87bfede908107`:

| Yêu cầu/quyết định hiện hành | Trạng thái và nguồn |
| --- | --- |
| ACC-03 / USER-08@r2 | Must — Pilot / `PRODUCT_CONFIRMED`: [PRD §14.6](../../prd/requirements.md#146-tài-khoản-acc-03), [DecisionDelta](../architecture/product-decisions-20261110.md) tại `af3c34b` dòng 41 |
| O-12@r2 | `PRODUCT_CONFIRMED`: [proposal](../../decisions/proposals/O-12.r2.toml), [signoff](../../decisions/signoffs/O-12.r2.toml). Server chuẩn hóa NFKC, đếm 10–128 code point cho mật khẩu mới, dữ liệu nhập tối đa 4096 byte; login giữ đường tương thích hash cũ |
| O-11@r1 | `OPEN` ở registry; 13/11 chủ sản phẩm xác nhận kênh email và luồng riêng ở [#181](https://github.com/loveoverflowcom/vink/issues/181). [Proposal r2](../../decisions/proposals/O-11.r2.toml) pending, chưa có runtime |
| D-05@r1 | `PROVISIONAL`: [decision log](../architecture/decisions.md) tại `73ab92f` dòng 47 và [ADR](../architecture/auth-authority.md). Gate production giữ nguyên |

## 1. Một nguồn trạng thái phiên

| Phần | Nơi | Vai trò |
|---|---|---|
| `SessionOwner`, `AuthError`, `text_survives` | [`session.rs`](../../apps/vink-web/src/session.rs) | Quyết định thuần (không HTTP, không reactive), có unit test |
| `Session` (`standing`, `principal`, `scope`) và vùng Tài khoản | [`account.rs`](../../apps/vink-web/src/account.rs) | Handle reactive do shell cấp một lần cho mỗi tab; chuyển câu trả lời giữa API và reducer |
| Client Auth | [`api.rs`](../../apps/vink-web/src/api.rs) | Gọi `/api/v1/auth/*`, phân loại lỗi theo status + `code` + `field` + `Retry-After` |

Quy tắc:

- **Chỉ server quyết định.** Trạng thái (`Unknown`, `Anonymous`, `NeedsPrincipal`, `Suspended`,
  `Active`, `Unavailable`) chỉ đổi theo câu trả lời của lần hỏi mới nhất; lần hỏi cũ trả lời muộn bị
  bỏ, nên một “anonymous” chậm không gỡ được lần đăng nhập vừa xong.
- **Generation.** `scope` vẫn có dạng principal/generation và chỉ tồn tại ở `Active`. Generation
  tăng khi principal đổi, hoặc khi đọc lại thành công sau một tín hiệu đổi phiên dù principal
  vẫn là người cũ. Bàn viết, Publication, thảo luận và ghi nhận dùng cùng scope; phản hồi của
  thao tác riêng tư trước đó bị bỏ theo cơ chế của từng view.
- **Vòng đời form và lượt ghi.** [AuthGate](../../apps/vink-web/src/account/flow.rs) giữ một lượt
  ghi trong tab cho cả đăng nhập, đăng ký, mã email, thêm mật khẩu, bút danh và đăng xuất.
  Form đang gắn mới được nhận kết quả. Tín hiệu đổi phiên vô hiệu form kể cả `needs_principal`
  → `needs_principal`; đóng form không nhả lượt ghi. Bỏ kết quả UI không ngăn Set-Cookie, nên
  [adapter phiên](../../apps/vink-web/src/account/browser.rs) đọc lại cookie thực tế khi một lượt
  có thể đã đổi phiên. Không bảo đảm thứ tự các thao tác độc lập giữa hai tab.
- **Lỗi tạm thời giữ trạng thái đã xác nhận.** Server tự xét quyền từng request; một lần `503` hay
  mất mạng không đăng xuất trang và không bỏ chữ đang viết.
- **Dữ liệu form.** Password và mã chỉ ở input và request đang xử lý; input đọc giá trị DOM lúc
  submit để hỗ trợ autofill. Không đưa chúng vào storage, URL, log, SSR hoặc tin nhắn giữa tab.
  SSR chưa biết phiên; trang hỏi sau hydration. Dữ liệu đã gửi không thể hủy bằng cách đóng form.
- **Khi nào hỏi lại:** sau hydration, khi cửa sổ lấy lại focus, khi một request riêng tư trả `401`,
  sau mỗi thao tác đăng nhập/đăng xuất/tạo bút danh, và khi tab khác báo có thay đổi. Tab khác chỉ
  nhận một tín hiệu `BroadcastChannel` **không mang dữ liệu** rồi tự hỏi server.

Nơi dùng nguồn này: avatar và vùng Tài khoản trong “Tôi”, Bàn viết (danh sách nháp, editor, thẻ
nháp), thảo luận và ghi nhận văn học trên trang đọc. Không view nào tự gọi `GET /auth/session`.

## 2. Luồng

1. **Tạo tài khoản** bằng tên đăng nhập và mật khẩu, hoặc **Đăng nhập** bằng mật khẩu. Cả hai đọc lại
   phiên bằng `SessionOwner`. `already_signed_in` đọc lại phiên hiện có; `username_taken` gắn lỗi
   tại ô tên. `invalid_credentials` là lỗi chung của form, không yêu cầu làm mới phiên.
2. **Dùng mã email** giữ hai bước xin mã (`202`: yêu cầu được ghi nhận) và nhập mã. Hạn lấy từ
   `expires_at`; mã sai cho nhập lại, mã đóng yêu cầu xin mã mới. Hai phương thức độc lập.
3. **Bút danh lần đầu.** Server trả `NeedsPrincipal` thì hiện form bút danh; `POST /auth/principal`
   lặp lại trả cùng principal. Chỉ `Active` có lối vào Bàn viết và vùng thêm mật khẩu.
4. **Thêm mật khẩu.** Đọc `GET /auth/password/enroll` khi mở vùng. Nếu chưa có, người dùng xin mã
   qua POST không có body. Server chọn email đã xác minh và gắn proof với phiên yêu cầu. Mã sai
   và trùng tên giữ proof; mã đóng yêu cầu mã mới. Thành công hoặc `already_enrolled` đọc lại trạng
   thái, không đổi phiên. `403` chung chỉ nói thao tác không đủ điều kiện; khóa tài khoản và đình
   chỉ có lý do riêng.
5. **Đăng xuất** chỉ được xác nhận khi nhận `204` đúng hợp đồng. Nếu máy chủ không xác nhận,
   nút và thông báo thử lại vẫn hiện trong phiên còn hiệu lực; một lần đọc lại có thể cho thấy
   cookie đã đổi. Không báo thành công dựa trên lỗi hoặc timeout.

## 3. Lỗi và đối soát

Thông điệp do Web viết; không hiện nội dung lỗi thô từ API. Lỗi trường, sai thông tin đăng nhập,
mất phiên, khóa/đình chỉ, giới hạn lượt thử, phương thức chưa cấu hình và dịch vụ tạm thời không
khả dụng có cách xử lý riêng. `429` có `Retry-After` chặn thao tác tương ứng tới hết thời gian chờ
kể cả khi đổi form; `501 authority_not_configured` chỉ áp dụng cho phương thức vừa dùng.

- **Đăng ký mất ACK:** có thể tài khoản đã được tạo dù GET phiên còn là khách. Có thể kiểm tra
  phiên hoặc chuyển sang đăng nhập với tên vừa chọn; không lặp POST tự động.
- **Đăng nhập mất ACK:** đọc lại phiên; nếu vẫn là khách, người dùng có thể chủ động thử lại sau
  đối soát. Sai mật khẩu không đi qua đường này.
- **Thêm mật khẩu mất ACK:** đọc lại cờ `enrolled`. `true` chỉ xác nhận tài khoản hiện có
  credential, không xác nhận username/password vừa gửi. `false` chỉ nói tại thời điểm đọc;
  người dùng có thể kiểm tra lại hoặc bắt đầu lại bằng mã mới, không replay proof cũ.
- Lỗi cho phép sửa tại chỗ giữ dữ liệu trong form đang mở. Đổi mode, đóng disclosure, rời trang,
  thành công hoặc đổi phiên thì hủy form tương ứng.

Ba lựa chọn kỹ thuật: giữ một vùng Tài khoản với form tách nhỏ; đặt enrollment sau bút danh để
đủ ngữ cảnh UI (backend vẫn hỗ trợ cả phiên chưa có principal); mở rộng TextField Leptos với
input native và nút hiện/ẩn. Không thêm route, đổi schema hoặc bổ sung renderer CMP. PR #176
đã có trong base: chỉ chạm section Auth ở API/contract/runner/feature-map; không sửa editor hay
luồng Publication. Consumer Publication tiếp tục nhận cùng dạng `Session::scope` với quy tắc
generation ở trên.

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

## Kiểm chứng

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

### AUTH-PASSWORD-WEB-01

Phase `auth-password` trong cùng lệnh dùng [runner Chrome](../../apps/vink-web/e2e/auth-password-smoke.mjs),
App DB riêng `vink_verify_web_password_auth` và Identity `native` trên scratch PostgreSQL. Không
có principal mẫu; mã đến outbox cục bộ. Phase email-code ở trên vẫn chạy độc lập, thêm ca thực
`501` khi thử mật khẩu trên deployment chỉ có email. Backend cũng có ca từ chối khi chỉ bật
password; trình duyệt chưa có phase riêng cho mode đó.

| AC | Cách kiểm và ranh giới |
|---|---|
| 01–02 | Chrome: đăng ký, bút danh, tạo/lưu/reload nháp; login giữ principal và nháp; sai mật khẩu là lỗi chung, không đọc lại phiên; `503` đăng xuất được dựng, `204` là thật |
| 03–04 | Chrome: nháp có trước enrollment, OTP sai và trùng tên giữ proof, đăng nhập lại đọc được nháp; cookie không đổi trong enrollment; nháp người khác trả `404`. Lane `postgres`: cùng subject qua hai phương thức, guard `already_signed_in`, quyền, expiry/reuse/wrong purpose, logout/disable, giao dịch cạnh tranh |
| 05–06 | Chrome: giữ và thả phản hồi login thật sau đổi mode, đăng ký/login/enrollment mất ACK sau khi App đã xử lý, và logout/login cùng principal ở tab khác với GET credential trả muộn. Ca đăng ký không có cookie dọn cookie thử ở CDP để mô phỏng đúng ranh giới; không thay kết quả ghi bằng response giả. Hồi quy mã email giữ thêm ca A → B và nháp riêng |
| 07–09 | Chrome: giá trị DOM không có sự kiện input, NFC/NFD, ký tự bổ sung, khoảng trắng, reveal/keyboard/IME mô phỏng; fixture và unit cho lỗi/đối soát; `429` thật trong phase email, `429` dựng cho nút mật khẩu. Lane `postgres` kiểm Unicode, raw byte cap, legacy hash, Origin/cookie/no-store và không log secret |
| 10 | Chrome: bàn phím và reduced motion; ma trận 320/390/1280 CSS px × sáng/tối × chữ gốc 100/200%; xem báo cáo chạy và ảnh local trong `target/verify-web/screenshots` |

## Giới hạn

- Loopback và Chrome headless; chưa kiểm HTTPS thật, edge/WAF, mail thật, Safari/Firefox,
  trình quản lý mật khẩu thật, IME tiếng Việt cấp hệ điều hành, screen reader hoặc thiết bị thật.
- Không có CMP/native auth, đổi mật khẩu/username/email, hay khôi phục mật khẩu trong lát này.
  D-05 vẫn Provisional; production vẫn khóa mặc định.
- Các view nháp còn đọc nối tiếp: lần đọc mới có thể đợi câu trả lời cũ đến hạn; dữ liệu danh
  sách thuộc scope cũ bị gỡ. Thảo luận đã có cơ chế bỏ lượt đọc riêng cũ khi đổi scope
  ([Reader reads](discussion-web-reads.md)).

## Đặc tả recovery #181 (chưa triển khai)

[Luồng Web → App → Identity](../architecture/password-recovery.md) và [LayoutSpec](../design/password-recovery.md)
đề xuất entry “Quên mật khẩu?” riêng trên `/toi`, không đổi các form đăng nhập/enrollment hiện có.
Trace tại `4c78601` xác nhận tài khoản password-only chưa có đường Web/App gắn email đã xác minh;
đó là dependency bắt buộc, không dùng mã sign-in để gộp tài khoản. Scope này chỉ có tài liệu và
O-11@r2 pending; chưa chạy browser/DB recovery và không kế thừa PASS Auth bên trên cho recovery.
