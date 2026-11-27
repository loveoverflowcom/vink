# Chữ giao diện Auth, tài khoản và hồ sơ — #585

[Issue #585](https://github.com/loveoverflowcom/vink/issues/585) rà chữ thừa từ
[report nguồn](https://drive.google.com/drive/u/2/folders/1b4kXO7k1wub1JmKSBuC2jdpG8mnpdXQ1).
Ảnh report ghi lại giao diện trước thay đổi. Delta của PR #586 được đối chiếu lại sau
[#591](https://github.com/loveoverflowcom/vink/pull/591),
[#592](https://github.com/loveoverflowcom/vink/pull/592) và
[#594](https://github.com/loveoverflowcom/vink/pull/594).

## LayoutSpec

| Vùng | Bố cục và nội dung |
| --- | --- |
| Auth | Form dọc R3, Be Vietnam Pro, token Lavender và trường nhập native. Helper nêu giới hạn ngay cạnh trường. Heading gọi tên phương thức, còn CTA nói rõ bước tiếp theo. Focus và thông báo theo đúng trạng thái yêu cầu mã. |
| Tài khoản và Chỉnh hồ sơ | [R5](account-profile-r5.md) sở hữu header có chỗ cho bút danh, BackLink, form và CTA. Tinh giản thông báo khi chưa thiết lập địa chỉ. Dialog giữ format, số lượt, lần đầu miễn lượt và alias. |
| Hồ sơ công khai | #592 sở hữu ProfileIdentity và CTA khách. Heading của vùng tác phẩm dùng “Tác phẩm”; thông báo “Chưa có tác phẩm công khai.” mô tả đúng phạm vi đang xem. |
| Danh sách theo dõi | #594 sở hữu BackLink, nhận diện, tabs và trạng thái danh sách. Hiển thị total theo quyền đếm độc lập với quyền xem list. Zero đã xác nhận, trang tiếp rỗng và total ẩn có dữ liệu và cách hiển thị riêng. |

## Nội dung rà soát

### Auth và email

Owner: [forms.rs](../../apps/vink-web/src/account/forms.rs),
[attachment.rs](../../apps/vink-web/src/account/forms/attachment.rs),
[session.rs](../../apps/vink-web/src/session.rs).

| Vị trí / trước | Sau | Quyết định và lý do |
| --- | --- | --- |
| Register/Enroll: tên đăng nhập có hai câu bổ sung về hoa thường và bút danh | “3–30 ký tự: chữ không dấu, số, _ hoặc dấu chấm.” | Rút helper dùng chung, giữ giới hạn và ký tự hợp lệ. Bước chọn bút danh vẫn giải thích tên công khai. |
| Register/Enroll: mật khẩu mô tả Unicode, 4.096 byte và quy tắc chữ hoa/số/ký hiệu | “10–128 ký tự.” | Giữ thông tin nhập thường dùng; validator không đổi. Lỗi InvalidPassword vẫn có điều kiện chuẩn hóa/byte vì backend gộp các nguyên nhân vào cùng lỗi. |
| Register: “Quay lại đăng nhập” | “Đăng nhập” | Đích đã rõ; giữ cùng handler. |
| Email: “Dùng mã email” ở heading | “Đăng nhập bằng email” | Gọi đúng phương thức đang dùng. CTA chuyển từ password vẫn là “Dùng mã email” theo Auth R3. |
| Email: “Nhập email để nhận mã đăng nhập gồm 6 chữ số.” | Bỏ đoạn mở đầu | Field Email và “Gửi mã đăng nhập” đã đủ. Field “Mã 6 chữ số” vẫn xuất hiện ở bước nhập mã. |
| Email: “Quay lại đăng nhập” | “Dùng mật khẩu” | Nêu rõ đổi phương thức, tránh hai CTA “Đăng nhập” có nghĩa khác nhau ở bước nhập mã. |
| Email trong Cài đặt | Địa chỉ thật và trạng thái “Đã xác minh” | #607 thay dòng trống bằng thông tin riêng tư của phiên hiện tại. |

Rà thêm và giữ: helper Chọn bút danh; acknowledgement yêu cầu gửi mã mới được ghi nhận;
expiry/retry; cảnh báo enrollment; email đang liên kết khác email nhập; pending, kết quả chưa
xác nhận và hướng đối soát phiên. Không thay IDs, labels của field, autocomplete, ARIA,
validation, focus hoặc session owner. Không chuyển helper bị bỏ vào hidden metadata.

### Hồ sơ và đối soát với R5

[profile.rs](../../apps/vink-web/src/pages/profile.rs) rút “Thiết lập địa chỉ tác giả trong
Chỉnh hồ sơ.” thành “Thiết lập địa chỉ trong Chỉnh hồ sơ.”; trạng thái và đường vào form
giữ nguyên. R5 đã có trợ giúp Bút danh ngắn, Giới thiệu, Thiết lập, Lưu, BackLink về Tài
khoản, header reflow và các nhãn quyền riêng tư trong component tương ứng.

[saved_profile.rs](../../apps/vink-web/src/pages/saved_profile.rs) sở hữu heading chung
cho vùng tác phẩm công khai. Giữ liên kết heading–section và điều kiện chỉ chủ hồ sơ mới
thấy các chế độ riêng. [handle.rs](../../apps/vink-web/src/pages/handle.rs) giữ lời giải thích
khi chưa có tác phẩm công khai.

Guest Appearance thuộc #576/#583. Auth R3 thuộc #559/#571. Hành vi quay về hồ sơ, focus
sau phân trang và trạng thái riêng tư, alias, retry thuộc #594. #560 tiếp tục sở hữu
[chuẩn Semantic UI](../development/web-semantics.md).

## Đường kiểm chứng

| Scenario | Kiểm ở giao diện và boundary |
| --- | --- |
| `auth` | Identity, bước gửi/nhập mã, trạng thái phiên và chuyển phương thức. |
| `password` | Auth R3, email xác minh, quyền theo phiên, đổi principal, hồ sơ và relationship, count/list, continuation và focus. |
| `standalone` | Artifact CSR với API-only, xác nhận heading vùng tác phẩm công khai và quyền truy cập danh sách riêng của chủ hồ sơ. |

[Scoped workflows](../development/scoped-workflows.md#scenario-web-và-setup) mô tả setup
và fixture. Kiểm ảnh từ browser đang chạy cùng các phép đo geometry, theme và cỡ chữ;
200% root-font và Chrome native zoom là hai phép đo riêng.
