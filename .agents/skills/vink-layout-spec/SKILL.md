---
name: vink-layout-spec
description: >-
  Mô tả bố cục Vink bằng cây vùng, phân cấp, responsive, theme và trạng thái trước
  khi sinh UI. Dùng cho phác thảo layout, phương án đã chọn hoặc mô tả vùng sửa nhỏ;
  cung cấp quy ước dùng chung cho Design, Fix, Inspector và frontend, không tự mở
  rộng sang triển khai hay audit toàn màn.
---

# Vink Layout Spec

Sở hữu **cách mô tả và đối chiếu bố cục**, không sở hữu quyết định nghiệp vụ,
renderer hay quyền thực hiện. Đọc nguồn gần tác vụ, branch/SHA, component và token
thực trước khi khẳng định chúng tồn tại. Dùng
[quy ước và template](references/layout-spec.md); chỉ đọc
[ví dụ](references/examples.md) khi cần minh họa hoặc kiểm lại workflow.

## Chọn mức mô tả

| Phạm vi | Đầu ra trước khi sinh UI |
| --- | --- |
| Màn mới / redesign đáng kể, chưa chọn hướng | 2–3 hướng khác nhau về cấu trúc/phân cấp; khuyến nghị một hướng rồi đặc tả hướng đó khi đã được giao thực hiện |
| Hướng đã chọn / reference đã chốt | Đặc tả đúng hướng đó; kế thừa phần còn hiệu lực, không mở lại cuộc chọn phương án |
| Sửa nhỏ: padding, copy, title, focus | Bản rút gọn cho vùng bị tác động: cây vùng ngắn, trước/sau, màn hẹp và cách kiểm |
| Chỉ phác thảo / chờ người dùng chọn | Xuất đủ phương án theo số lượng được yêu cầu rồi dừng ở phần đã giao |
| Audit-only | Đối chiếu spec nếu có; nếu thiếu, ghi cây bố cục quan sát được và giới hạn, không biến nó thành thiết kế đã duyệt |

Design sở hữu lựa chọn thiết kế; Fix sở hữu sửa lỗi; Inspector sở hữu quan sát;
frontend sở hữu code. Các workflow đó dùng quy ước này rồi tiếp tục phần của mình.
Skill này không gọi ngược lại các workflow để thực hiện cùng bước.

## Trình bày trước khi tạo giao diện

1. Nêu người dùng, tác vụ, nội dung/hành động chính và trạng thái. Với nhiều hướng,
   giữ cùng vai trò, nội dung mẫu, viewport và state; chọn **cùng một trạng thái
   khó** để so tất cả các hướng, không thay bằng empty ở A, error ở B, conflict ở C.
   Mỗi hướng có cây ngắn, màn hẹp, cách xử lý state chung và đánh đổi. Các state
   bổ sung thuộc spec hướng được chọn. Chỉ đổi màu/font/radius không thành hướng mới.
2. **Xuất mô tả cho người dùng đọc được trước lần đầu tạo/sửa mã UI, script sinh UI
   hoặc gọi công cụ tạo preview của phương án.** Có thể dùng progress message hoặc
   tài liệu được liên kết trước thao tác đó. Bao gồm CSS, HTML, JS/TS, `.mjs`,
   component Rust/Leptos và công cụ sinh ảnh UI; không viết trước rồi bổ sung ở cuối.
   Đọc source, mở ảnh và quan sát app hiện có vẫn được làm trước để hiểu tác vụ.
3. Mô tả không phải bước xin duyệt. Nếu đã được giao prototype/triển khai, nêu hướng
   chọn và tiếp tục. Chỉ chờ lựa chọn khi người dùng yêu cầu; với quyết định chưa rõ
   làm đổi nghiệp vụ/quyền, hoàn tất phần độc lập rồi hỏi đúng phần còn thiếu.
4. Giữ một cây chính có ID vùng ổn định; Grid Areas/wireframe chỉ là phần suy ra.
   Nếu cần đổi cấu trúc, CTA hoặc thứ tự đọc khi triển khai, trình bày phần spec đổi
   và lý do trước khi sửa tiếp, không âm thầm làm lệch đặc tả.
5. Đối chiếu đầu ra với spec theo cùng vùng/state/viewport/theme. Tách concept,
   source đã viết, render đã mở và tương tác đã kiểm; ghi gap đúng lớp bằng chứng.

## Điều kiện hoàn tất

Mức chi tiết vừa đủ với phạm vi; token/component đã kiểm hoặc ghi rõ đề xuất;
responsive có đường truy cập phần thu gọn; trạng thái và hành động không đổi nghĩa.
Giữ Unicode, đoạn/khổ thơ và text chưa lưu; lưu nháp không là gửi AI/duyệt/đăng,
được duyệt không là công khai hoặc tuyển chọn. Dữ liệu mẫu phải được nhận diện.

Đặc tả đã xuất đúng thời điểm và có đường kiểm đối chiếu. Với spec-only, giao đặc
tả là hoàn tất; không bịa code/render để tăng mức bằng chứng. Với sửa skill/tài
liệu, kiểm metadata/link và các tình huống workflow; không dựng màn sản phẩm chỉ
để chứng minh hướng dẫn đã thay đổi.
