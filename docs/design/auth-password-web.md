# Tôi · Đăng nhập và thêm mật khẩu (AUTH-PASSWORD-WEB-01)

**Intent.** Tạo tài khoản mới để viết, trở lại đúng Bàn viết, hoặc thêm cách đăng nhập vào tài
khoản mã email đang dùng. Tên đăng nhập chỉ dùng để xác thực; bút danh vẫn là tên công khai.
Mỗi bước cho thấy một việc chính có thể làm ngay.

**References.** [Auth Web R2](https://drive.google.com/drive/folders/1FqKCK6Z3Znej_RYehYFE66v5qd5OQjL9) là mốc Web **đã duyệt** ngày 19/11/2026 cho khách và bút danh ([#327](https://github.com/loveoverflowcom/vink/issues/327)). Ba ảnh: đăng nhập mobile tối, bút danh mobile sáng, đăng nhập desktop sáng. Đây là thiết kế, không phải ảnh trình duyệt hay bằng chứng phiên. Phần tài khoản đang hoạt động và Giao diện kế thừa [Tôi R5](me-preferences-r5.md); dùng [FloatingTonal](../../packages/ui-system/shared/components.md) và [quy tắc phản hồi](state-and-feedback.md).

**Hierarchy · R2.** Trang `/toi` giữ rail/thanh dưới. Thứ tự đọc cũng là thứ tự form:

```text
#page-account / #account-session
  .vk-auth-entry             vùng phẳng, không có thẻ trạng thái khách
    V-pen → tiêu đề          Đăng nhập / Tạo tài khoản / Dùng mã email / Chọn bút danh
    #password-login          Tên đăng nhập → Mật khẩu (hiện/ẩn) → Đăng nhập (Filled)
    Thay thế                 Chưa có tài khoản? Tạo tài khoản → Dùng mã email (Outlined)
    #password-register       chỉ hai trường và hướng dẫn → Tạo tài khoản
    #sign-in-email / -code   một form ở mỗi bước; quay lại dùng mật khẩu
    #pen-name-form           chỉ khi máy chủ yêu cầu: Bút danh + trợ giúp → Lưu bút danh
  #auth-appearance           Giao diện → lựa chọn thật trên cùng trang
  Đã hoạt động               trạng thái, Bàn viết, thêm mật khẩu, đăng xuất như trước
```

**Responsive.** Cột form căn giữa phần nội dung, tối đa 24rem, lề mobile 20px tại 390px.
Logo và tiêu đề Literata căn giữa; trường, nút chính và nút email cùng trục/cùng bề rộng.
Giao diện ở dưới nhóm đăng nhập trên mobile, góc phải trên desktop. Bút danh giữ thao tác
thoát phiên ngoài form. Cuộn theo trang; ở 200% ưu tiên reflow, lỗi và mọi hành động tới
được bằng bàn phím thay vì ép toàn bộ vào một màn hình. Không thêm stepper hay link recovery.

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

[Ảnh Chrome và kết quả theo AC của R2](evidence/auth-r2-327/README.md).
