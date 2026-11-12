# Không gian kiểm duyệt tác phẩm — LayoutSpec r1

Phạm vi triển khai Web theo MOD-10..14 và `O-17@r2`, không thay quyết định sản phẩm/ADR.
Nguồn luật và API: [moderation-runtime](../development/moderation-runtime.md),
[publication-moderation](../architecture/publication-moderation.md). Bản layout này ghi phương án
đã mô tả trong phiên triển khai sau #170; không phải signoff sản phẩm mới.

## Người dùng và tác vụ

- Người duyệt mở hàng ca đúng phạm vi, đọc snapshot đã gửi, nhận ca rồi quyết định/ghi chú/chuyển admin.
- Admin đọc phản đối và giữ nguyên/đảo quyết định; dừng hoặc bật tự động có version và lý do.
- Tác giả xem trạng thái từng bản gửi, lý do bác, gửi phản đối và đối chiếu lịch sử.
- Khách ở URL cũ đọc thông báo và thảo luận. Trang ẩn không đưa tên tác giả hay metadata.

## Cấu trúc được chọn

Dùng danh sách và trang chi tiết riêng thay cho danh sách–chi tiết hai cột. Việc đọc văn bản dài
và nhập lý do có đủ chiều ngang ở cả desktop lẫn màn hình hẹp; Back/đường dẫn thật giữ hành vi
trình duyệt. Phương án hai cột tiết kiệm chuyển trang trên desktop nhưng ép snapshot và hành động
trên màn hình hẹp, nên chưa dùng trong lát cắt này.

| Trang | Thứ tự vùng ổn định |
| --- | --- |
| `/kiem-duyet` | Tiêu đề → ca tác phẩm → phản đối (admin) → chế độ tự động (admin). |
| `/kiem-duyet/ca/{id}` | Quay về → tiêu đề/trạng thái/version → snapshot → nhận/trả/chuyển → lý do/kết luận → ghi chú/lịch sử. |
| `/kiem-duyet/phan-doi/{id}` | Quay về → tiêu đề/snapshot → lý do → giữ nguyên/đảo quyết định. |
| `/ban-viet/{id}/kiem-duyet` | Về bản viết → các revision/trạng thái/lý do → gửi phản đối khi được phép → lịch sử. |
| Reader thay thế | Về Khám phá → thông báo (tên tác giả chỉ ở review-withdrawn) → thảo luận. |

## Kích thước, token và tương tác

Tái sử dụng shell, `Heading`, `Link`, `Button`, `TextArea`, `DocumentView` và notice hiện có.
Nội dung quản trị tối đa `54rem`; khoảng cách `--vk-space-3/4/6`, actions flex wrap, không có màu,
bóng hay breakpoint riêng. Snapshot và lý do theo một cột; nút xuống hàng ở viewport hẹp.
Dùng nhãn tiếng Việt, liên kết thật, trạng thái pending của primitive, focus theo trình duyệt.

Mỗi lệnh mang version từ server. Trong lúc gửi khóa nút; chỉ tải trạng thái mới sau response,
không tự đánh dấu duyệt. Xung đột/mất quyền/lỗi mạng có chữ và nút tải lại; lý do/ghi chú ở trang
chi tiết không mất khi refetch. Dữ liệu riêng chỉ tải ở client, gắn session scope + route; HTML
SSR không chứa snapshot, API và trang riêng `no-store`/`noindex`. Không báo “đã gửi thông báo” khi
chưa có kênh thông báo.

## Evidence và giới hạn

Phase `review-withdrawn` trong `cargo xtask verify web` thao tác ca duyệt desktop và phản đối
390px, kiểm overflow, chụp screenshot, đối chiếu API và SSR. Lượt chạy, SHA và giới hạn ở
[registry](../development/moderation-runtime.md#9-bằng-chứng-và-giới-hạn).
Chưa có UI quản lý grant/nhóm, điều tra, ca bình luận hay chọn audience/gửi duyệt. Không dùng mẫu
browser của suite làm bằng chứng quyền production hoặc khả năng truy cập bằng screen reader.
