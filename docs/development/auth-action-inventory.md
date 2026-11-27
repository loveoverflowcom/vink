# Kiểm kê hành động Auth trên Product Web · #625

[LayoutSpec R5](../design/auth-password-web.md) sở hữu quy ước trình bày. Lát cắt này áp dụng
quy ước ấy ở các caller Product Web. Thẩm quyền phiên và thứ tự đăng ký theo
[D-05](../architecture/auth-authority.md), [Auth Web](auth-web.md) và ACC-03.
Hình thức biểu tượng thuộc LayoutSpec, nên công việc này không thêm bản ghi quyết định TOML.

| Host và trạng thái | Đích trong DOM và owner | Thay đổi hoặc trạng thái đã đối chiếu |
| --- | --- | --- |
| `/toi`, Reader/Nhận xét và Reader/Lưu bài, khách có gate | `GuestAuth` → `GuestPassword`: `#password-login-submit`, `#use-registration`, `#password-login-retry` | Bổ sung Lock cho Filled submit và Text thử lại, PersonAdd cho Text chuyển đăng ký. Filled submit đổi glyph sang MicroArc khi pending. |
| Cùng host, gửi mã và xác minh tài khoản mới | `RegistrationForm`: `#registration-send`, `#registration-resend`, `#use-password`; `GuestPassword`: `#password-register-submit` | Mail cho yêu cầu mã Filled/Text, Lock cho Text chuyển về đăng nhập, PersonAdd cho Filled tạo tài khoản sau chứng minh email. Chia sẻ `AuthAccountSwitch` ở cả hai bước đăng ký và Login. |
| Cùng host, đăng nhập email | `EmailSignIn`: `#sign-in-send`, `#sign-in-complete`, `#sign-in-resend`, `#use-password`; `GuestPassword`: `#use-email-code` | Thêm Mail cho yêu cầu mã, Lock cho submit mã. Dùng mã email và Dùng mật khẩu đã có Mail/Lock. Đổi email chỉ đổi ô nhập. |
| `/toi`, active, cần bút danh hoặc suspended | `SignedInSettings` và `AccountHub` → `SignOut`, `#sign-out` | Dùng Logout thay Close. Text ở Cài đặt, Tonal cho hai nhánh khác. Một MicroArc thay glyph khi chờ. Kiểm tra phiên, thêm mật khẩu và quản lý/xóa tài khoản là các thao tác riêng. |
| Shell mọi route, rộng/hẹp/modal/bar | `AccountLink`, `a[data-destination=me]` | Đăng nhập + Lock khi gate mở, Tài khoản + Person/PersonOutline khi đóng, avatar sau xác nhận. Tên AX có ở rail thu gọn và `aria-current` tiếp tục theo route. |
| Bàn viết, khách hoặc liên kết sâu; lỗi tải bản thảo khi hết phiên | `DeskAccess` hoặc adapter lỗi, `#desk-sign-in` | Thêm Lock vào anchor Filled hoặc Standalone với đích `next` hiện có. Khi gate đóng chỉ có Khám phá. Đặt bút danh, Mở Tài khoản khi bị tạm khóa và Kiểm tra lại phiên giữ mục đích riêng. |
| Khám phá Đang theo dõi, URL trực tiếp khi khách hoặc expired | `FeedAccountLink`, `#feed-sign-in` | Lock cho Đăng nhập. Hoàn tất bút danh dùng chung anchor và đích nhưng không gắn Lock. Tab Đang theo dõi vẫn ẩn với khách; restricted chỉ dẫn Mở Tài khoản. |
| Hồ sơ tác giả và Theo dõi | `ProfileActionRegion`, `#follow-sign-in`; hồ sơ riêng `OwnProfileHost` | Theo dõi là anchor intent có đích quay lại hồ sơ, rồi dùng form Auth chung. Cần bút danh hiện hành động tương ứng. Hồ sơ riêng dùng Mở Tài khoản khi thiếu scope; không công khai overlay. |
| Reader, Nhận xét và Đã lưu | `#save-work`, `#root-composer-send`, `#reply-composer-…-send`, `#reader-auth`, saved profile | CTA mở overlay vẫn là Lưu hoặc Gửi. Form Login trong overlay dùng cùng owner, còn Close vẫn đóng overlay. Không tự chạy ý định sau khi xác thực. Tắt đăng nhập giữ các lý do chặn; phần Đã lưu riêng phụ thuộc scope. |
| Các guard còn lại | Inbox `.vk-notif-empty`, Desk review status, lỗi xuất bản và nhận xét, thông báo phiên | Mở Tài khoản là liên kết hỗ trợ, còn chỉ dẫn và tiêu đề là nội dung đọc. Không thêm CTA vào các trạng thái này. |

Người dùng guest, expired, suspended và cần bút danh có nhánh riêng ở các owner nói trên.
`AuthEntry` không dựng form cho guest khi deployment đóng sign-in. Không khôi phục Auth
recovery chưa triển khai và không đưa lựa chọn email vào form đăng ký. Shared Button và Link
theo [UIS-02](../../packages/ui-system/shared/components.md), pending theo UIS-03 từ #624.
Icon Logout đến từ đúng pin của [catalog](../../packages/ui-system/shared/icons.json) và được
sinh cho cả Rust lẫn Kotlin.
