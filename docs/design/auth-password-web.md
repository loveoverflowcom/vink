# Tôi · Đăng nhập và thêm mật khẩu (AUTH-PASSWORD-WEB-01)

**Intent.** Tạo tài khoản mới để viết, trở lại đúng Bàn viết, hoặc thêm cách đăng nhập vào tài
khoản mã email đang dùng. Tên đăng nhập chỉ dùng để xác thực; bút danh vẫn là tên công khai.
Mỗi bước cho thấy một việc chính có thể làm ngay.

**References.** [Auth Web R2](https://drive.google.com/drive/folders/1FqKCK6Z3Znej_RYehYFE66v5qd5OQjL9) là mốc Web **đã duyệt** ngày 19/11/2026 cho khách và bút danh ([#327](https://github.com/loveoverflowcom/vink/issues/327)). Ba ảnh: đăng nhập mobile tối, bút danh mobile sáng, đăng nhập desktop sáng. Đây là thiết kế, không phải ảnh trình duyệt hay bằng chứng phiên. Phần tài khoản đang hoạt động và Giao diện kế thừa [Tôi R5](me-preferences-r5.md); dùng [FloatingTonal](../../packages/ui-system/shared/components.md) và [quy tắc phản hồi](state-and-feedback.md).

**Hierarchy · R3.1.** Chủ sản phẩm duyệt bố cục hành động ngày 27/11/2026 trong
[issue #559](https://github.com/loveoverflowcom/vink/issues/559). [HTML preview R3.1](https://drive.google.com/file/d/1PC5meJCs_J_5ig3jwaZT-OEP6RzSAUY3/view)
minh họa thứ tự và khoảng cách. Trường và surface của R3 ngày 26/11 được giữ theo
[PR #571](https://github.com/loveoverflowcom/vink/pull/571). Product dùng logo, font và màu Vink.
Trang `/toi` giữ rail/thanh dưới. Thứ tự đọc cũng là thứ tự form:

```text
#page-account / #account-session
  .vk-auth-entry             vùng phẳng, không có thẻ trạng thái khách
    V-pen → tiêu đề          Đăng nhập / Tạo tài khoản / Đăng nhập bằng email / Chọn bút danh
    #password-login          Tên đăng nhập → Mật khẩu (hiện/ẩn) → Đăng nhập (Filled)
                             Dùng mã email (Tonal, thư) → Chưa có tài khoản? Tạo tài khoản (Text)
    #registration-email      Email → Gửi mã xác minh (Filled)
    #password-register       Mã xác minh → Tên đăng nhập → Mật khẩu → Tạo tài khoản (Filled)
                             Gửi lại mã / Dùng email khác
                             Đã có tài khoản? Đăng nhập (Text)
    #sign-in-email           Email → Gửi mã đăng nhập (Filled) → Dùng mật khẩu (Tonal, khóa)
    #sign-in-code            Mã → Đăng nhập → Gửi lại mã / Dùng email khác → Dùng mật khẩu
    #pen-name-form           chỉ khi máy chủ yêu cầu: Bút danh + trợ giúp → Lưu bút danh
  Đã hoạt động               Email riêng tư chỉ đọc, mật khẩu, Bàn viết, đăng xuất
```

**Responsive.** Cột form căn giữa phần nội dung, tối đa 24rem, lề mobile 20px tại 390px.
Logo và tiêu đề Literata căn giữa. Trường FloatingTonal cao ít nhất 56 px, góc `radius-xl`.
Nút Filled và Tonal cùng trục, mỗi nút một hàng toàn chiều rộng, cao ít nhất 48 px. Nhóm hành
động chính dùng khe `space-2` (8 px ở cỡ chữ mặc định). Hàng chuyển tài khoản cách nhóm
`space-4` (16 px), căn giữa, có thể xuống dòng và giữ đích bấm tối thiểu 48 px chiều cao.
Hai trường mật khẩu khách dùng biểu tượng người/khóa và nút hiện/ẩn riêng ở cuối ô mật khẩu.
Từ đăng ký, chọn Đăng nhập để về Login rồi chọn Dùng mã email khi cần.
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

**Interaction và accessibility.** `GuestAuth` dùng chung cho `/toi` và Reader. Nút Text và
Tonal đổi mode tới đích xác định, đưa focus tới ô đầu. Các mode cùng một URL.
Refresh nền giữ focus và chữ khi phiên không đổi. Nhãn HTML và mô tả nối với trường; nút hiện/ẩn có tên
và trạng thái nhấn, không submit; click khi đang nhập giữ focus/selection, bàn phím giữ focus ở
nút. Enter gửi đúng form ngoài composition; paste và giá trị do autofill điền được đọc từ input.
Không autosubmit mã. Disclosure ở ngay trong trang, không tạo modal hoặc chiếm Escape/Back.
Rời bước xóa input; request đã đi vẫn cần đối soát với máy chủ.

**Invariants và Compare.** Dùng Chrome ở 320/390/834/1440 CSS px, sáng/tối theo hệ thống
và cỡ chữ 100%/150% theo CSS Product, thêm lượt stress 200% bằng ghi đè cỡ chữ gốc.
Cảnh khách 150% dùng fixture áp dụng `data-text="150"` trên CSS Product. Kiểm tràn ngang, thứ tự focus,
nhãn, lỗi, hành động và tải font Be Vietnam Pro. Thành viên chỉnh Giao diện trong
[Cài đặt của Tôi](me-preferences-r1-209.md).
Đối chiếu vị trí và phân cấp khi mở ảnh viewport; ảnh không thay kiểm focus, motion hoặc lưu dữ
liệu. [Auth Web](../development/auth-web.md#kiểm-chứng) giữ bảng kiểm, artifact và giới hạn.

[Ảnh Chrome và kết quả theo AC của R2](evidence/auth-r2-327/README.md).

[Lịch sử ảnh và kết quả kiểm Chrome #483](evidence/auth-appearance-483/README.md).
