# Tôi · Đăng nhập và thêm mật khẩu (AUTH-PASSWORD-WEB-01)

**Intent.** Tạo tài khoản mới để viết, trở lại đúng Bàn viết, hoặc thêm cách đăng nhập vào tài
khoản mã email đang dùng. Tên đăng nhập chỉ dùng để xác thực; bút danh vẫn là tên công khai.
Mỗi bước cho thấy một việc chính có thể làm ngay.

**References.** [Auth Web R2](https://drive.google.com/drive/folders/1FqKCK6Z3Znej_RYehYFE66v5qd5OQjL9) là mốc Web **đã duyệt** ngày 19/11/2026 cho khách và bút danh ([#327](https://github.com/loveoverflowcom/vink/issues/327)). Ba ảnh: đăng nhập mobile tối, bút danh mobile sáng, đăng nhập desktop sáng. Đây là thiết kế, không phải ảnh trình duyệt hay bằng chứng phiên. Phần tài khoản đang hoạt động và Giao diện kế thừa [Tôi R5](me-preferences-r5.md); dùng [FloatingTonal](../../packages/ui-system/shared/components.md) và [quy tắc phản hồi](state-and-feedback.md).

**Hierarchy · R3 Vertical Tonal.** Chủ sản phẩm duyệt hướng hình thức ngày 26/11/2026.
[Issue #559](https://github.com/loveoverflowcom/vink/issues/559) lưu LayoutSpec, [ảnh duyệt](https://drive.google.com/file/d/1Rd6so1t6J7RU9CADcw5NcfTAiJPcS5c3/view)
và [HTML preview](https://drive.google.com/file/d/1drUHZAuUnJcIdhe_YZRsTAnopx1-AXU2/view).
Ảnh và HTML minh họa hướng thiết kế. Browser Product chạy theo nguồn Web thật.
Trang `/toi` giữ rail/thanh dưới. Thứ tự đọc cũng là thứ tự form:

```text
#page-account / #account-session
  .vk-auth-entry             vùng phẳng, không có thẻ trạng thái khách
    V-pen → tiêu đề          Đăng nhập / Tạo tài khoản / Dùng mã email / Chọn bút danh
    #password-login          Tên đăng nhập → Mật khẩu (hiện/ẩn) → Đăng nhập (Filled)
    Hoặc                    Dùng mã email (Tonal, thư) → Tạo tài khoản (Tonal, thêm người)
    #password-register       chỉ hai trường và hướng dẫn → Tạo tài khoản → Quay lại đăng nhập
    #sign-in-email / -code   một form ở mỗi bước → Quay lại đăng nhập
    #pen-name-form           chỉ khi máy chủ yêu cầu: Bút danh + trợ giúp → Lưu bút danh
  Đã hoạt động               trạng thái, Bàn viết, thêm mật khẩu, đăng xuất như trước
```

**Responsive.** Cột form căn giữa phần nội dung, tối đa 24rem, lề mobile 20px tại 390px.
Logo và tiêu đề Literata căn giữa. Trường, nút chính và hai lựa chọn Tonal cùng trục, mỗi
điều khiển một hàng toàn chiều rộng. Trường dùng FloatingTonal, cao ít nhất 56 px và góc
`radius-xl`. Nút dùng recipe Button, cao ít nhất 48 px, với khe chuẩn `space-3` giữa các điều khiển
cùng nhóm. Hai trường mật khẩu khách dùng biểu tượng người/khóa ở đầu và nút hiện/ẩn riêng ở cuối
ô mật khẩu. Đăng ký giữ cả đường quay lại đăng nhập và lối dùng mã email hiện có.
Từ [#576](https://github.com/loveoverflowcom/vink/issues/576), vùng Auth chỉ trình bày các tác vụ
đăng nhập, tạo tài khoản và bút danh hiện có. Bút danh giữ thao tác thoát phiên ngoài form.
Cuộn theo trang; ở 200% ưu tiên reflow, lỗi và hành động tới được bằng bàn phím.

**Màu và trạng thái.** Form dùng `surface-container-highest` cho trường trên nền trang `bg-page`
hoặc nền dialog `surface-container`. Alias `field-container` được đặt trong form Auth. Độ chênh
hai nền tonal cần thêm một đường nhận biết 1 px màu `field-outline`. Trường dùng chính đường đó
cho focus 2 px, lỗi 1 px và lỗi có focus 2 px. Nút hiện/ẩn có focus riêng. Forced colors dùng màu
hệ thống trên cùng đường nét. Filled và Tonal dùng màu và lớp phản hồi của Button.
Spinner giữ chỗ trong nút đang gửi, có trạng thái truy cập. Lỗi tên đăng nhập, mật khẩu
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

**Invariants và Compare.** Dùng Chrome ở 320/390/834/1440 CSS px, sáng/tối theo hệ thống
và chữ 100%, thêm lượt stress 200% bằng ghi đè cỡ chữ gốc. Kiểm tràn ngang, thứ tự focus,
nhãn, lỗi, hành động và tải font Be Vietnam Pro. Thành viên chỉnh Giao diện trong
[Cài đặt của Tôi](me-preferences-r1-209.md).
Đối chiếu vị trí và phân cấp khi mở ảnh viewport; ảnh không thay kiểm focus, motion hoặc lưu dữ
liệu. [Auth Web](../development/auth-web.md#kiểm-chứng) giữ bảng kiểm, artifact và giới hạn.

[Ảnh Chrome và kết quả theo AC của R2](evidence/auth-r2-327/README.md).

[Lịch sử ảnh và kết quả kiểm Chrome #483](evidence/auth-appearance-483/README.md).
