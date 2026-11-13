# Tôi · Đăng nhập và thêm mật khẩu (AUTH-PASSWORD-WEB-01)

**Intent.** Tạo tài khoản mới để viết, trở lại đúng Bàn viết, hoặc thêm cách đăng nhập vào tài
khoản mã email đang dùng. Tên đăng nhập chỉ dùng để xác thực; bút danh vẫn là tên công khai.
Mỗi bước cho thấy một việc chính có thể làm ngay.

**References.** Kế thừa [Tôi R5](me-preferences-r5.md), [trạng thái và phản hồi](state-and-feedback.md)
và [FloatingTonal](../../packages/ui-system/shared/components.md). R5 là nguồn bố cục hiện hữu,
không phải ảnh chuẩn cho form mật khẩu. Theo [sổ thị giác](visual-anchors.md), kết luận fidelity
là `NO_APPROVED_REFERENCE`. Đây là LayoutSpec của lát Web, không phải signoff sản phẩm.

**Hierarchy.** Giữ trang `/toi`, phần Giao diện và điều hướng hiện tại.

```text
#page-account
  .vk-account-status         Tài khoản · phiên/bút danh server xác nhận
  #account-session
    Khách: #password-login   Tên đăng nhập → mật khẩu → Đăng nhập
       → Tạo tài khoản (#password-register, hai trường + hướng dẫn) hoặc Dùng mã email
       → #sign-in-email / #sign-in-code, một bước mỗi lần
    #pen-name-form           Bút danh khi cần định danh để viết
    Đã hoạt động:            Mở Bàn viết
       #password-credential → #password-credential-status
         chỉ mở khi người dùng chọn; đã có / chưa có / đang đọc / chưa đọc được
         chưa có: xin mã → #password-enroll (mã, tên đăng nhập, mật khẩu)
    #sign-out                thao tác riêng của phiên hiện tại
```

**Responsive.** Giữ trục trái của Tôi; form rộng tối đa 28rem và co theo vùng chứa. Nhóm nút tự
xuống hàng, không thu cỡ chữ. Mật khẩu có chỗ cố định cho nút hiện/ẩn. Mọi trường đều bắt buộc; giữ `aria-required` và nhãn ngắn. Ở 200%, nhãn và hướng dẫn
tăng chiều cao, với khoảng cuộn để các đích focus không nằm dưới thanh điều hướng. Lavender dùng
token hiện có cho cả sáng/tối; không lồng thêm thẻ hay thêm viền ô nhập lúc nghỉ.

**States.** Spinner giữ chỗ trong nút đang gửi, có trạng thái truy cập. Lỗi tên đăng nhập, mật khẩu
hoặc mã nằm tại trường; sai thông tin đăng nhập dùng một câu chung trong form. Thao tác chưa rõ
kết quả có đường kiểm tra phiên/trạng thái. `Retry-After` có một dòng đếm ngược dưới nút, hết giờ không tự gửi.
Enrollment không hỏi email; xác nhận yêu cầu gửi mã đã được ghi nhận và nêu hạn dùng. GET báo
đã có mật khẩu thì chỉ hiện trạng thái, không suy tên đăng nhập. Thiếu phương thức ở deployment
không gỡ các lựa chọn khác.

**Interaction và accessibility.** Mode đổi do người dùng chọn đưa focus tới ô đầu. Refresh
nền giữ focus và chữ khi phiên không đổi. Nhãn HTML và mô tả nối với trường; nút hiện/ẩn có tên
và trạng thái nhấn, không submit; click khi đang nhập giữ focus/selection, bàn phím giữ focus ở
nút. Enter gửi đúng form ngoài composition; paste và giá trị do autofill điền được đọc từ input.
Không autosubmit mã. Disclosure ở ngay trong trang, không tạo modal hoặc chiếm Escape/Back.
Rời bước xóa input; request đã đi vẫn cần đối soát với máy chủ.

**Invariants và Compare.** Dùng Chrome ở 320/390/1280 CSS px × sáng/tối × chữ gốc 100/200%:
không tràn ngang quá 1 CSS px, không mất nhãn/hành động/lỗi, font Be Vietnam Pro đã tải.
Đối chiếu vị trí và phân cấp khi mở ảnh viewport; ảnh không thay kiểm focus, motion hoặc lưu dữ
liệu. [Auth Web](../development/auth-web.md#kiểm-chứng) giữ bảng kiểm, artifact và giới hạn.
