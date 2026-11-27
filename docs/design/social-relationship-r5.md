# Người theo dõi và Đang theo dõi R5

Hướng Quiet Lavender đã duyệt tại [#590](https://github.com/loveoverflowcom/vink/issues/590).
Ảnh ngày 26/11 trong issue là mốc so sánh bố cục. Back dùng API của
[Tài khoản và Chỉnh hồ sơ R5](account-profile-r5.md).

| Vùng | Bố cục và tương tác |
| --- | --- |
| R1 Đầu trang | Cùng trục với hồ sơ công khai. Back dẫn về hồ sơ canonical và dùng tên đích làm nhãn. Desktop đặt pill phía trên bút danh. Mobile đặt nút tròn 48 CSS px cạnh danh tính. H1 hiển thị bút danh và có tên tiếp cận chứa hướng hiện tại. Handle phụ chỉ đến từ địa chỉ canonical đã cấp. |
| R2 Hướng quan hệ | Nhãn Người theo dõi và Đang theo dõi theo thứ tự cố định, căn trái và xuống hàng khi thiếu chỗ. Hướng hiện tại có chỉ báo và `aria-current` khi có hai hướng. Hướng còn lại là link khi quyền đọc đã xác nhận. Nếu chỉ có một hướng, trình bày nhãn của hướng đó. |
| R3 Danh sách | Mỗi hàng phẳng có một đích `AuthorIdentity` gồm avatar, bút danh và handle. Nhãn Theo dõi lẫn nhau ở ngoài link, chỉ hiện trong projection của chủ hồ sơ. |
| R4 Trạng thái | Count của hướng hiện tại đến từ phép đọc danh sách đã được cấp quyền. Trang đầu rỗng dùng `EmptyState` cỡ đầy đủ và ảnh NoResults trong vùng nội dung còn trống. Dùng câu xác nhận chưa có quan hệ khi count bằng 0. Khi thiếu count, câu chỉ mô tả danh sách đang hiển thị. Trang sau rỗng giữ link Về trang đầu. Lỗi, riêng tư và không tìm thấy giữ cách phục hồi của route. |

[RelationshipList](../../apps/vink-web/src/pages/follow.rs) giữ ID điều hướng, href, con trỏ và phạm
vi phiên. Điều hướng dùng link thật trên cả SSR và sau hydrate. Trang mới nhận focus ở H1.
Back của trình duyệt có thể đưa focus về hàng hoặc link nguồn nếu còn hợp lệ.

Đối chiếu Product ở 320/390/834/1440 CSS px. Cỡ 100% và 150% lấy từ Giao diện, còn 200%
là kiểm thử root font riêng. Dùng tên dài tiếng Việt, kiểm focus và thử với cả hai theme.
