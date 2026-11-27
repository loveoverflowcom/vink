# Hành động Auth · #625

| Cảnh Chrome | Trước | Sau |
| --- | --- | --- |
| Đăng nhập và hàng Tạo tài khoản, Dark 390 × 844 / 100% | [Ảnh trước](before/login-390-dark-100.png) | [Ảnh sau](after/login-390-dark-100.png) |
| Đăng nhập, giữ phản hồi, Light 390 × 844 / 100% | [Ảnh trước](before/login-390-light-100-pending.png) | [Ảnh sau](after/login-390-light-100-pending.png) |
| Đăng xuất ở Chọn bút danh, Light 390 × 844 / 100% | [Ảnh trước](before/logout-390-light-100.png) | [Ảnh sau](after/logout-390-light-100.png) |
| Đăng xuất đang chờ, Light 390 × 844 / 100% | [Ảnh trước](before/logout-390-light-100-pending.png) | [Ảnh sau](after/logout-390-light-100-pending.png) |
| Guard Đang theo dõi, Light 834 × 900 / 150% | [Ảnh trước](before/following-834-light-150.png) | [Ảnh sau](after/following-834-light-150.png) |

Bản trước là `8a3d6a5ab3ca67030eb7f9b47e85750e60f7413c`. Bản Product sau là
`9def42688c02a295e1856d2fc2a52fd84d7da797`. Bản trước dùng cùng feature flags với bản sau.
Chrome chạy SPA Product với API fixture trên loopback cho ảnh so sánh. Dữ liệu form và phản hồi lỗi
trong ảnh là fixture thử nghiệm. App, Identity và PostgreSQL scratch được dùng trong kịch bản
kiểm chứng hành vi bên dưới. Các ảnh liên kết trong bảng đã được mở và đối chiếu.

[Số đo trước](before/measurements.json) và [số đo sau](after/measurements.json) có SHA, route,
state, theme, viewport, cỡ chữ và tọa độ. Mỗi bản ghi 25 cảnh ở 320, 390, 834 và 1440 CSS px
với Light/Dark, cỡ chữ 100%, 150% Product và 200% stress. Các cảnh Đăng nhập 390 px ghi thêm
motion thường, reduced motion và forced colors. [SHA-256 các ảnh](sha256.json).
Số đo Đăng xuất có cùng năm cấu hình mỗi bản. [Trước](before/logout-measurements.json) · [Sau](after/logout-measurements.json).

Phản hồi giữ khi Đăng nhập có một MicroArc leading và mô tả của nút. Tọa độ và focus giữ nguyên
trong cả hai theme, reduced motion có 0 vòng lặp. Test backend phát hiện thêm chuyển vị 7,5 CSS px
khi ghi chú chờ gửi lại mã biến mất cùng thanh cuộn dọc. Sau khi giữ gutter của trang Tài khoản,
tọa độ nút và nhãn Tạo tài khoản trùng nhau khi chờ, focus còn ở ô tên đăng nhập.
Chrome đọc nút pending là “Đăng nhập”, busy cùng mô tả “Đang đăng nhập…”.
Rail modal vẫn có link Đăng nhập và Lock. Bàn viết và Đang theo dõi mở đúng đích với
modifier-click và chạm mô phỏng CDP. Thử đăng nhập lại sau lỗi 503 mở lại form mà không gửi
một yêu cầu thứ hai. [Oracle AX, liên kết và retry](after/ax-native-links.json).
Lỗi 401 ở bản thảo và Following thử qua fixture Chrome, có tên AX và Lock trước chữ.
Nhánh Hoàn tất bút danh giữ đúng đích. [Số đo guard](after/guard-edges.json).

## Chỉ dẫn chạy

Chạy từ root Vink, một Cargo job và một test thread. Kiểm thử Browser dùng runner
`cargo xtask verify web --scenario <tên> --capture full`, VINK_TEST_DATABASE_URL trên PostgreSQL 17
scratch và Chrome local. Những lượt `--capture checks` vẫn chạy oracle hành vi và DOM. Build, unit
và Clippy dùng `tools/local-budget run --low-disk`; phép đo RSS browser chạy theo cây tiến trình.

[Kết quả kiểm kê và LayoutSpec](../../auth-action-inventory.md).

## Đối chiếu runtime

| Phần kiểm | Bằng chứng |
| --- | --- |
| Desk | Guard khi sign-in đóng, riêng tư khi khách, các tình huống recovery và responsive. Guard khi gate mở cũng chạy trong Password. |
| Password | App/Identity/PostgreSQL thật. Đăng ký đã chứng minh email, Lock/PersonAdd, đăng xuất Tonal/Text, một DELETE, giữ focus và kích thước lúc pending, lỗi và ACK muộn. Các hành trình hồ sơ, Follow và feed giữ quyền và ngữ cảnh. |
| Standalone | Product CSR với App API và phiên thật. Reader, giữ bản nháp, đổi phiên, Hồ sơ/Đã lưu và phân vùng dữ liệu tiếp tục hoạt động qua logout và phiên khác. |
| Appearance | Rail, modal, thanh dưới, avatar, cỡ chữ, màu và focus chạy lại trên bản có gutter bằng `--capture checks`. |
| Auth | OTP giữ một POST và trả trạng thái lỗi; Reader mở form với đúng tên truy cập. Gửi/Lưu vẫn đòi thao tác rõ ràng sau Auth. Nháp, selection và focus được giữ. |
| Unit và lint | Web 328 đạt, 1 ignored có sẵn. Shared UI 74 đạt. Clippy SSR `-D warnings`, fmt và kiểm tài liệu đạt. |
| Leptos Preview | `ui-preview leptos verify` đạt, 938 captures. Các oracle Button, Link, pending/motion và reflow của shared UI chạy trên cùng SHA Product. `ui-tokens --check` xác nhận catalog sinh Rust và Kotlin cùng khớp nguồn. |

`Password`, `Standalone`, `Appearance`, `Auth`, Preview và unit Web chạy ở SHA Product ghi trên. Lượt Desk đầy đủ chạy
ở `f7d3c7004260608112fece23db84509d0a054611`; sau đó Product chỉ thêm gutter ở trang Tài khoản.
Render và kiểm layout/logic của riêng Desk vẫn áp dụng. Guard mở đã chạy lại trong Password.
