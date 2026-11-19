# Auth R2 trong Tôi · #327

Phạm vi: Web trên `/toi`. [LayoutSpec và mốc R2](../../auth-password-web.md) là thiết kế đã duyệt;
ảnh bên dưới là **Chrome chụp từ App chạy thật**, không phải bản thiết kế hay ảnh production.

- Base: `f0e3ca60fe84baad040f3dbc516ea8feb96d9691`.
- [Source trước khi thêm evidence](https://github.com/loveoverflowcom/vink/commit/1f2edc13233fe8202448d7cb8612ecce0ed7a83d);
  Git tree `d40d355841abfc4fe5b98a98a2bbf126dfc2706b` (cùng tree với checkout đã chạy).
- App và Identity trên loopback, PostgreSQL 17.11 scratch, email thử qua outbox cục bộ. Chrome 157 headless
  qua CDP, viewport cao 900 CSS px, DPR 1. Chữ 200% là phóng font gốc, không phải zoom trình duyệt.
- Lệnh chính: `cargo xtask verify web --scenario auth --scenario password`.
  Hai compiler jobs, hai test threads; browser lane không thuộc phạm vi của launcher local-budget.
  PASS `auth` và `password`; 36 tổ hợp form/viewport/theme/cỡ chữ, 12 kiểm nhóm nút ở mobile 200%.
  Đường tắt đăng nhập kiểm riêng với App cấu hình đóng, không dùng lỗi 501 để thay thế.

Logs local ngày 19/11/2026 (SHA-256, không đưa log server/outbox vào Git):

- `final-auth-password.log`: `ccf77c3f3a4122ea52dfe77591c7ff55795c8afaa8f3a1e426f8b25e865d5204`.
- `final-closed.log`: `f87b3b37041bf82e273a5c66f7340ffd5168914ca75c5811c4760aa980710112`.

| AC | Kiểm tại ranh giới nào |
| --- | --- |
| 01 | Chrome: một cột, logo R1 sáng/tối, Đăng nhập → Tạo tài khoản / Dùng mã email; Enter ở Giao diện đưa focus tới phần cài đặt thật, Tab kế vào radio. Khi App tắt đăng nhập: thông báo, không có form và Giao diện vẫn dùng được. |
| 02 | Đăng ký thật → đọc lại phiên `needs_principal` → chỉ một form Bút danh; reload vẫn đúng bước. Người đã có principal đăng nhập lại không bị hỏi bút danh. Không thêm recovery hoặc bước tiến trình. |
| 03 | Chrome + hai service: autofill mô phỏng bằng giá trị DOM không phát input event, hiện/ẩn mật khẩu, validation và focus; mất ACK sau khi server xử lý không replay, đổi mode và phản hồi muộn, ranh giới phiên. `501` mật khẩu thật ở deployment chỉ bật email; một số lỗi khác được dựng có nhãn trong runner. |

## Ảnh và đối chiếu

| Vùng | Ảnh viewport thật | Nhận xét |
| --- | --- | --- |
| Đăng nhập mobile, tối, 100% | [390×900](login-390-dark-100.png) | Logo/tiêu đề trước hai trường tonal; không còn thẻ trạng thái. Một CTA chính, dòng tạo tài khoản, nút email viền rồi Giao diện. |
| Bút danh mobile, sáng, 100% | [390×900](pen-name-390-light-100.png) | Một trường và trợ giúp, một nút Lưu. Thoát phiên vẫn tới được khi cuộn, không chen vào nhiệm vụ chính. |
| Đăng nhập desktop, sáng, 100% | [1280×900](login-1280-light-100.png) | Cột 384 CSS px căn giữa phần còn lại sau rail, Giao diện ở góc trên bên phải. |
| Đăng nhập mobile, tối, 200% | [đầu trang](login-320-dark-200.png), [sau khi cuộn](login-320-dark-200-end.png) | Không buộc toàn form vào một màn. Nút và nhãn xuống dòng; kiểm điểm nhấn được tại 320 và 390, không suy từ ảnh. |

Ma trận: 320/390/1280 × sáng/tối × chữ 100/200 cho đăng nhập, đăng ký, bút danh. Kiểm form
dọc, logo đúng asset, font đã tải, tràn ngang ≤1 CSS px, tương phản nhãn và trợ giúp. Các ảnh có
thanh cuộn Chrome, kích thước khung và giá trị input khác concept; không phải so tuyệt đối từng pixel.

| Chiều | Kết luận trong phạm vi local |
| --- | --- |
| Functional | PASS — Đăng nhập, đăng ký, email, bút danh, tài khoản cũ và App tắt đăng nhập. |
| Geometry | PASS — Ma trận phía trên; form một cột và các nút co giãn, không tràn ngang quá 1 CSS px. |
| Visual | PASS (self-review) — Đã mở ảnh thật và đối chiếu phân cấp với ba ảnh thiết kế R2 100%. Sau vòng đầu, giữ lại Đăng xuất nhưng đặt sau khung bút danh trên mobile. Không nhận ảnh mới làm golden. |
| Accessibility/Responsive | PASS ở Chrome cho phóng chữ, nhãn, độ tương phản được đo và Tab/Enter/focus-visible; screen reader và hệ điều hành thật NOT_RUN. |
| Interaction | PASS — Đổi phương thức, đọc DOM lúc submit, hiện/ẩn mật khẩu và phản hồi muộn kiểm bằng thao tác browser riêng, không suy từ PNG. |

Không kiểm thiết bị thật, IME cấp hệ điều hành, password manager bên thứ ba hoặc screen reader.
Không thay contract, mở production, kiểm GitHub CI hay triển khai luồng Reader/CMP/recovery.
