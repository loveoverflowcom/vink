# D-05 / S-03 — Lựa chọn auth authority

**Ngày:** 07/11/2026 · **Base:** `develop@9dc4991620ef93b09c21fdba355a3c3f557da608` ·
**Trạng thái:** **Đề xuất cho pilot (Recommended / Provisional)**, chưa Accepted. Spike chạy được
cục bộ và đổi D-05 từ “Cần spike + lựa chọn” sang “Đề xuất A, chờ chủ sản phẩm và các blocker ở
§9”. **Production authentication: NOT_IMPLEMENTED** (không có mail transport thật, rate limit ở
biên, qualification trình duyệt/mobile hay review bảo mật độc lập).

Bằng chứng chạy và ma trận kịch bản: [auth-authority-s03-evidence](../development/auth-authority-s03-evidence.md).
Ranh giới database/process có từ trước: [identity-postgres-boundaries](identity-postgres-boundaries.md).

## 1. Câu hỏi quyết định

Vink cần một authority xác thực cho MVP/Pilot sao cho: principal Vink ổn định và độc lập
provider; xác thực tách khỏi phân quyền nghiệp vụ; có email đã xác minh; logout/revoke/đình chỉ
không cấp quyền sai; mở rộng được Web + Android/iOS; chi phí và vận hành hợp lý; không biến Vink
thành hệ thống auth quá phức tạp (ACC-01, ACC-02, INV-05).

Phạm vi pilot dùng để cân: nhóm người lớn được mời (D-11 đề xuất), một website Leptos SSR, chưa có
app native sản phẩm, chưa cần SSO doanh nghiệp hay đăng nhập mạng xã hội, một nhóm vận hành nhỏ.

## 2. Hiện trạng trước spike

| Lớp | Có sẵn tại base | Thiếu |
| --- | --- | --- |
| `vink-identity` | Process riêng, DB `vink_identity` (chỉ migration baseline), Unix socket, caller = peer uid, contract v1 `verify-credential` luôn `501 authority_not_configured` | Authority, tài khoản, credential, session |
| App `identity` | `IdentityResolver` → `Principal`/`Anonymous`/`Unavailable`; `IdentityService` gửi `Authorization: Bearer` sang Identity và **không bao giờ** map subject → principal | Binding, đình chỉ, carrier cookie, đường đăng nhập |
| App HTTP | `Actor` extractor, kiểm `Origin` cho mọi phương thức ghi, `LocalFixture` chỉ ở local + loopback | Endpoint đăng nhập/đăng xuất |

Mọi route riêng tư production trả `401`; đọc công khai/SSR không gọi Identity.

## 3. Tiêu chí và kịch bản chung

Mười tiêu chí của yêu cầu được áp như nhau cho ba hướng: bảo mật và email đã xác minh; độ phức
tạp hiện thực; tích hợp Rust/Axum; độ khớp với `vink-identity`; principal ổn định và account
linking; đúng logout/revoke/ban; Web SSR và CMP/mobile tương lai; tái lập cục bộ; chi phí vận hành;
riêng tư và độc lập provider.

Kịch bản chấp nhận dùng chung (chi tiết và kết quả ở file bằng chứng): email chưa xác minh không
có quyền; subject/user_id/role giả bị từ chối; signup đồng thời không nhân đôi principal; cùng email
khác subject không tự nối; đổi email/bút danh giữ principal; credential sai/hết hạn/phát lại bị từ
chối; logout chặn thao tác sau; revoke/đình chỉ mất quyền; request cũ không khôi phục quyền;
Identity timeout/outage đóng; RPC sai/khác version bị từ chối; provisioning lỗi không gán sai chủ;
cô lập nháp/bình luận/ghi nhận giữa tài khoản; đọc công khai còn khi Identity sập; secret không vào
log/bundle.

## 4. Ba ứng viên (một cho mỗi hướng)

| Hướng | Ứng viên chọn để so | Vì sao là đại diện của hướng |
| --- | --- | --- |
| A. Native Rust | Tài khoản trong `vink-identity`, **mã email một lần** (không mật khẩu) + **phiên opaque phía server**; mật khẩu Argon2id để sau | Khớp process/DB đã có; thư viện Rust duy trì tốt (`sha2`, `subtle`, `getrandom`; `argon2` 0.6.0 và `webauthn-rs` 0.6.2 cho bước sau theo crates.io 07/11/2026) |
| B. OIDC tự host | **Keycloak 26.x** (bản mới nhất trên trang tải: 26.8.2; image `quay.io/keycloak/keycloak`) | Trưởng thành, có self-registration, email verification, back-channel logout, chạy trên PostgreSQL. **Kanidm**: tự đăng ký công khai mới được nhắc tới là “tính năng signup mới” (issue #4666, 01/11/2026), chưa xác nhận bản stable; Cantos (cùng tổ chức) dùng Kanidm làm issuer nhưng phải tự xây luồng enrollment bằng Firebase và tới audit 17/10/2026 vẫn thiếu đăng ký native ([cantos#174](https://github.com/loveoverflowcom/cantos/issues/174)). **ZITADEL**: AGPL-3.0 từ v3, theo nguồn thứ ba v4 tách dịch vụ login riêng — thêm một service nữa |
| C. Managed | **Supabase Auth** | Mã nguồn mở (GoTrue) nên có đường tự host, region chọn được, JWKS bất đối xứng. **Clerk**: 50.000 MRU miễn phí, Pro $25/tháng, $0,02/MRU vượt mức; trang giá không nêu chọn vùng dữ liệu |

Giá và giới hạn trích từ trang chính thức truy cập 07/11/2026: Supabase Free 50.000 MAU, project
Free tạm dừng sau 1 tuần không hoạt động; Pro từ $25/tháng gồm 100.000 MAU, $0,00325/MAU vượt mức.
Mail mặc định của Supabase chỉ gửi tới thành viên team, 2 thư/giờ, “not meant for production use”:
mọi hướng đều cần nhà cung cấp mail riêng.

## 5. So sánh

Chất lượng bằng chứng: **[R]** chạy trong spike này; **[D]** tài liệu chính thức đã đọc; **[I]**
suy luận kiến trúc, chưa chạy.

| Tiêu chí | A. Native (mã email + phiên server) | B. Keycloak | C. Supabase Auth |
| --- | --- | --- | --- |
| Email đã xác minh | Mỗi lần đăng nhập chứng minh kiểm soát hộp thư [R]. Không phải authenticator theo NIST 800-63B (email “SHALL NOT” dùng cho OOB) — tương đương mật khẩu + reset qua email [D] | Có verify email, MFA, phát hiện brute force [D] | Có verify email, MFA [D] |
| Bề mặt tấn công do Vink giữ | Nhỏ nhưng **của Vink**: throttle, thử sai, enumeration, mail, phục hồi [R]/[I] | Nhỏ ở code Vink; lớn ở vận hành JVM + nâng cấp [I] | Nhỏ ở code; phụ thuộc vendor [I] |
| Độ phức tạp hiện thực | ~1 module chính sách + 1 store + 5 thao tác contract; đã chạy [R] | RP OIDC (code + PKCE, JWKS, back-channel logout) + theme login FreeMarker tiếng Việt [I] | Gọi REST + kiểm JWT/JWKS; vẫn cần phiên Vink riêng [I] |
| Rust/Axum | Thuần Rust, không SDK [R] | `openidconnect` 4.1.0 [D] | REST + thư viện JWT [I] |
| Khớp `vink-identity` | Identity giữ tài khoản/credential/phiên đúng như ranh giới định ra [R] | Identity thành RP/verifier; Keycloak là owner dữ liệu thứ ba [I] | Identity thành verifier; dữ liệu người dùng ở ngoài [I] |
| Principal ổn định, linking | Subject = id tài khoản, không đổi khi đổi email; App binding `(authority, subject)` [R] | `sub` ổn định trong realm [D]; binding như A [I] | `sub` = uuid người dùng [D]; binding như A [I] |
| Logout/revoke/ban | Thu hồi là UPDATE một hàng, hiệu lực ở request kế tiếp; disable/đổi email thu hồi trong cùng transaction [R] | Phiên Vink tức thời; disable phía Keycloak cần back-channel hoặc introspection [D]/[I] | Access JWT còn hiệu lực tới `exp` sau khi đăng xuất; ban tạm không thu hồi phiên đang có [D] |
| Web SSR + CMP/mobile | Cookie `__Host-` HttpOnly cho Web [R]; cùng API mã + Bearer opaque trong Keychain/Keystore cho native [I] | AppAuth/system browser (RFC 8252) [I] | SDK Kotlin/Swift; CMP cần adapter [I] |
| Tái lập cục bộ | `cargo` + PostgreSQL + thư mục outbox [R] | Container Keycloak + DB thứ ba [I] | Supabase CLI (nhiều container) hoặc GoTrue riêng [I] |
| Chi phí định kỳ | Hạ tầng sẵn có + nhà cung cấp mail (chưa chọn, chưa ước giá) | Máy chủ JVM + backup realm + nâng cấp; chưa ước giá | $0 tới giới hạn Free (project bị pause khi không hoạt động), Pro từ $25/tháng [D] + mail |
| Riêng tư/độc lập | Dữ liệu ở PostgreSQL của Vink; chỉ relay mail thấy địa chỉ [I] | Tự host, dữ liệu ở Vink [I] | Xử lý bởi bên thứ ba; Luật BVDLCN 91/2025/QH15 (hiệu lực 01/01/2026) yêu cầu hồ sơ đánh giá chuyển dữ liệu ra nước ngoài theo nguồn pháp lý thứ ba — chưa có tư vấn pháp lý [D thứ ba]/[I] |

## 6. Quyết định đề xuất

**Chọn A cho pilot:** `vink-identity` là authority native với đăng nhập bằng mã email một lần và
phiên opaque phía server; App sở hữu principal, binding subject → principal, đình chỉ và mọi
quyền nghiệp vụ.

Lý do theo bằng chứng:

1. Phần authority thực sự phải làm ở Vink hẹp: chứng minh kiểm soát hộp thư lúc đăng nhập và giữ
   phiên. Principal, binding, đình chỉ, kiểm duyệt và quyền xuất bản thuộc App trong **mọi**
   hướng. B và C chỉ thay phần hẹp đó nhưng thêm một lớp phiên thứ hai và bài toán đồng bộ thu hồi.
2. Thu hồi đúng ngay: logout, disable và đổi email thu hồi phiên trong cùng transaction với thay
   đổi trạng thái, có test đồng thời trên PostgreSQL thật. Với C, JWT đã phát còn hiệu lực tới khi
   hết hạn theo chính tài liệu Supabase.
3. Không có mật khẩu để lưu, bị dùng lại hay bị nhồi; luồng reset (vốn cũng là mã email) trùng với
   đăng nhập, nên trần bảo mật vẫn là hộp thư như một hệ mật khẩu + reset, nhưng ít bề mặt hơn.
4. Pilot là nhóm được mời, giao diện tiếng Việt riêng: trang login hosted của B cần theme; C vẫn cần
   UI riêng. A dùng chung UI Vink.
5. Dữ liệu tài khoản ở hạ tầng Vink, giảm câu hỏi chuyển dữ liệu xuyên biên giới cho pilot.

**Không chọn B (Keycloak)** cho pilot: thêm một dịch vụ JVM, một database và vòng nâng cấp cho nhóm
nhỏ, cộng đồng bộ phiên/thu hồi; lợi thế chính (SSO, liên kết danh tính, MFA sẵn) chưa là nhu cầu
pilot. Giữ làm đích di chuyển nếu cần SSO/MFA/federation (§8). Kanidm và ZITADEL bị loại vì lý do
ở §4.

**Không chọn C (Supabase/Clerk)** cho pilot: thu hồi không tức thời với JWT, phụ thuộc vendor cho
vòng đời danh tính, câu hỏi pháp lý về chuyển dữ liệu chưa có tư vấn, và vẫn phải tự xây phiên Vink
+ mail. Lợi thế “ít vận hành” nhỏ hơn với phạm vi hẹp ở điểm 1.

**Mật khẩu (Argon2id) để sau, không loại:** nếu pilot cho thấy mail chậm/khó nhận, thêm mật khẩu
như một credential khác của **cùng tài khoản** (subject không đổi): `argon2` 0.6.0, tham số tối thiểu
OWASP m=19 MiB, t=2, p=1, lưu chuỗi PHC, rehash khi đăng nhập nếu tham số đổi. **Passkey**
(`webauthn-rs`) là bước tăng cường chống phishing ưu tiên hơn mật khẩu.

## 7. Kiến trúc của spike

```mermaid
sequenceDiagram
    participant B as Trình duyệt
    participant A as vink-app (Axum)
    participant I as vink-identity (Unix socket)
    participant M as Outbox cục bộ
    B->>A: POST /api/v1/auth/sign-in/challenges {email} + Origin
    A->>I: POST /identity/v1/sign-in/challenges
    I->>M: thư chứa mã (0600)
    I-->>A: 202 {challenge (bí mật), expires_at}
    A-->>B: 202 + Set-Cookie __Host-vink_sign_in (HttpOnly, Strict)
    B->>A: POST /api/v1/auth/sign-in/complete {code} + cookie
    A->>I: POST /identity/v1/sign-in/complete
    I-->>A: authenticated {subject, session.credential}
    A->>A: binding (authority, subject) + đình chỉ
    A-->>B: {state} + Set-Cookie __Host-vink_session (HttpOnly, Lax)
    B->>A: request riêng tư + cookie
    A->>I: verify-credential (mỗi request)
    I-->>A: subject | rejected
    A->>A: binding → Principal | Unbound | Suspended
```

| Owner | Giữ | Không bao giờ |
| --- | --- | --- |
| `vink-identity` (DB `vink_identity`) | `accounts`, `account_emails` (1 địa chỉ đã xác minh/tài khoản), `email_challenges` (chỉ verifier), `sessions` (chỉ hash) | Principal, quyền, tác phẩm; đọc bảng App |
| App (DB `vink`) | `principal_bindings` (bất biến, PK `(authority, subject_id)`, 1 subject/principal), `principal_suspensions`, mọi quyền | Email, mã, credential, hash phiên |

**Phiên do Identity phát (điều chỉnh so với ranh giới 02/11).** Ranh giới cũ đề xuất App giữ
“session ứng dụng”. Spike đặt phiên ở Identity vì: bearer secret và trạng thái credential cùng một
DB/role nhạy cảm; disable và đổi email thu hồi phiên **trong cùng transaction** — nếu phiên ở App,
mỗi thay đổi đó thành đồng bộ xuyên owner không nguyên tử, đúng rủi ro đã nêu cho B/C. Đổi lại: mỗi
request riêng tư thêm một lời gọi Unix socket + một lookup có index (chưa đo hiệu năng), và Identity
sập thì request riêng tư `503` (đóng an toàn; đọc công khai không bị ảnh hưởng — đã chạy). Phân
quyền (binding, đình chỉ) vẫn ở App và được đọc ở mỗi request.

**Kết quả có kiểu.** Identity: `authenticated` / `rejected{reason}` / `501` / `503`. App
`Resolution`: `Principal` / `Anonymous` / `Unbound(subject)` / `Suspended` / `Unavailable`; chỉ
`Principal` được hành động, `Unavailable` không bao giờ thành ẩn danh hay `Unbound` (tránh mời tạo
principal thứ hai). `GET /api/v1/auth/session` cho client biết trạng thái; route riêng tư trả `401`
cho ba trạng thái không-principal.

**Contract.** Identity RPC: thêm thao tác và trường **trong v1** theo quy tắc tương thích (chỉ
thêm thao tác/trường phản hồi; request mới đều `deny_unknown_fields`); fixture cũ không đổi ý
nghĩa ([contract v1](../../services/vink-identity/contracts/rpc/v1/README.md)). App HTTP v1 thêm
nhóm `auth` ([API v1](../../services/vink-app/contracts/http/v1/README.md#sign-in-d-05s-03-spike)).

**Phiên Web.** Cookie `__Host-vink_session` (`Secure; HttpOnly; Path=/; SameSite=Lax`), giá trị
256 bit ngẫu nhiên, chỉ lưu SHA-256; hết hạn tuyệt đối 30 ngày (NIST 800-63B-4 AAL1: tổng ≤ 30 ngày),
không gia hạn trượt. CSRF: mọi phương thức ghi của API đã bắt `Origin` khớp `VINK_PUBLIC_ORIGIN`.
Session fixation: credential luôn do server phát mới khi đăng nhập, client không đề xuất được.
Challenge handle ở cookie `__Host-vink_sign_in` `SameSite=Strict`; mã lưu dạng
`SHA-256(domain, bí mật client giữ, mã)` nên mã gõ ở trình duyệt khác (thiếu handle) là mã sai.
Logout chỉ xóa cookie khi Identity đã thu hồi; khi Identity sập, cookie giữ lại để thử lại
(trong lúc đó phiên không xác minh được nên không cấp quyền).

**Mobile (chưa làm).** Cùng hai thao tác mã; client native nhận credential qua kênh riêng (không
cookie) và gửi `Authorization: Bearer`; cần quyết định cách chống CSRF/Origin cho client native và
nơi lưu (Keychain/Keystore). Không có magic link nên không cần universal link.

## 8. Đánh đổi bảo mật, vận hành và chiến lược thoát

| Rủi ro | Giảm thiểu trong spike | Còn thiếu |
| --- | --- | --- |
| Đoán mã | 6 chữ số, 5 lần sai đóng challenge, 10 phút, mã gắn bí mật client | Rate limit theo IP/thiết bị ở biên |
| Spam mã tới hộp thư người khác | 3 mã/địa chỉ/15 phút, khóa advisory chống vượt đồng thời | CAPTCHA/biên, chặn domain, theo dõi lạm dụng |
| Enumeration | Xin mã không đọc bảng tài khoản; trả lời và thư giống nhau | Thời gian phản hồi chưa đo |
| Mất hộp thư | — | Quy trình phục hồi thủ công, thông báo về địa chỉ cũ khi đổi email |
| Chiếm hộp thư = chiếm tài khoản | Như mọi hệ reset bằng email | Passkey; thông báo đăng nhập mới |
| Lộ DB Identity | Chỉ verifier/hash, không có mã hay credential | Retention/xóa phiên cũ, rotation |
| Request đang chạy khi bị đình chỉ | Đình chỉ kiểm ở mỗi request | Fence tại commit cho mọi lệnh ghi (như grant kiểm duyệt) |

**Chi phí vận hành A:** thêm một nhà cung cấp mail (deliverability, SPF/DKIM/DMARC, bounce), quy
trình phục hồi tài khoản, giám sát lạm dụng; không thêm service hay database. Chưa có ước giá.

**Thoát/di chuyển:** subject luôn là `(authority, id)` và App chỉ biết binding. Chuyển sang OIDC
(Keycloak/managed) = thêm authority mới chạy song song; người dùng cũ liên kết **có xác minh** (đăng
nhập authority cũ trong cùng luồng) trước khi App thêm binding thứ hai — khi đó mới nới ràng buộc
một subject/principal bằng migration có chủ đích. Không liên kết tự động theo email (App không thấy
email). Dữ liệu tài khoản nằm trong PostgreSQL của Vink, xuất được bằng `pg_dump` theo owner.

## 9. Blocker trước khi Accepted

1. Chủ sản phẩm chấp nhận UX đăng nhập chỉ bằng mã email cho pilot (hoặc yêu cầu thêm mật khẩu).
2. Mail transport production + nhà cung cấp, deliverability và chi phí thật.
3. Rate limit ở biên/IP và giám sát lạm dụng; quy trình phục hồi và thông báo đổi email.
4. Qualification trình duyệt (cookie `__Host-` trên origin thật, HTTPS) và UI đăng nhập; quyết định
   carrier/CSRF cho native.
5. Fence đình chỉ tại commit cho các lệnh ghi; retention phiên/challenge; review bảo mật độc lập.
6. Một người/nhóm được giao chịu trách nhiệm bảo trì bảo mật auth (vá, sự cố, phục hồi tài khoản) —
   điều kiện tech stack đặt ra cho native auth.

## 10. Lát cắt tiếp theo

1. **Production authority:** mail transport thật, gửi **bất đồng bộ** (challenge và thư chờ gửi ghi
   cùng transaction, worker gửi và thử lại) vì lời gọi contract có hạn 2 s và client App bỏ cuộc sau
   1,5 s — spike ghi file đồng bộ và đã thấy một lần vượt hạn khi đĩa bận (xem bằng chứng); rate
   limit biên, retention, thông báo đổi email, fence đình chỉ tại commit; giữ contract v1.
2. **Phiên + API được bảo vệ:** UI đăng nhập/đăng xuất Leptos, `GET /auth/session` trong shell, đổi
   bút danh, thử trình duyệt (cookie, Back/reload, hai tab).
3. **Native/CMP:** carrier Bearer cho app, lưu trữ an toàn, qualification Android/iOS.

## Nguồn (truy cập 07/11/2026)

- NIST SP 800-63B (rev. 4): <https://pages.nist.gov/800-63-4/sp800-63b.html> — email không dùng cho
  xác thực out-of-band; mã xác nhận/khôi phục qua email không thuộc lệnh cấm; phiên ≥ 64 bit,
  cookie `Secure`, `HttpOnly`, `__Host-`, `SameSite`; AAL1 tổng ≤ 30 ngày.
- OWASP Password Storage Cheat Sheet: <https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html>.
- Supabase: giá <https://supabase.com/pricing>, SMTP <https://supabase.com/docs/guides/auth/auth-smtp>,
  khóa ký JWT <https://supabase.com/docs/guides/auth/signing-keys>, đăng xuất
  <https://supabase.com/docs/guides/auth/signout>, quản lý người dùng
  <https://supabase.com/docs/guides/auth/managing-user-data>.
- Clerk: <https://clerk.com/pricing>.
- Keycloak: <https://www.keycloak.org/downloads>.
- Kanidm: <https://github.com/kanidm/kanidm/issues/4666>, <https://github.com/kanidm/kanidm/issues/2287>.
- ZITADEL changelog (đổi giấy phép sang AGPL-3.0 ở v3): <https://zitadel.com/changelog>; chi tiết v4
  tách dịch vụ login chỉ có từ nguồn thứ ba, chưa đối chiếu tài liệu chính thức.
- Luật Bảo vệ dữ liệu cá nhân 91/2025/QH15 (tóm tắt của các hãng luật, không phải văn bản gốc):
  <https://tilleke.com/insights/vietnams-new-personal-data-protection-law-a-closer-look>.
- crates.io API cho phiên bản `openidconnect`, `argon2`, `webauthn-rs`.
