# Editor R4 · delta triển khai Web ngày 24/11/2026

Phạm vi [#475](https://github.com/loveoverflowcom/vink/issues/475), PR 1/2 của
[#428](https://github.com/loveoverflowcom/vink/issues/428). Nguồn thiết kế giữ nguyên theo
[LayoutSpec r4 ngày 23/11](https://docs.google.com/document/d/15iMZyQFaPRJ_0ExTnpdHe5zDAlav-OC-/edit)
và [bàn giao 13 ảnh, ZIP](https://github.com/loveoverflowcom/vink/issues/428#issuecomment-7103859904)
(`DESIGN_PROPOSED`, `NO_APPROVED_REFERENCE`). Báo cáo preview mô tả fixture riêng của thiết kế.
[VDR-EDITOR-R4@r1](../../decisions/proposals/VDR-EDITOR-R4.r1.toml) còn `PENDING` trong kho quyết định.

## Bố cục và nguồn dữ liệu

- Route `/ban-viet/{id}` dùng một EditorView và DraftSession. Thanh đầu trang giữ nút Bàn viết,
  tên màn, trạng thái lưu, một ô thao tác lưu và menu Chi tiết lưu. “Gửi” ở bản mới vẫn là lưu nháp
  theo projection hiện có. Gửi kiểm duyệt và chuyển riêng tư là thao tác xác nhận trong phần Công khai.
- Mặt giấy dùng `bg-content`, cột bên rộng 17rem và giấy tối đa 48rem. Khi nội dung còn dưới 50rem
  sau rail và lề, cùng bảng Tùy chọn bài viết xuống dưới giấy và gập. Disclosure native theo
  container query, hỗ trợ Enter/Space; co màn khi focus trong bảng chuyển focus về summary.
  Mục lục Markdown ở cùng cột phụ khi đủ chỗ và dùng sheet native khi hẹp.
- Dạng bài dùng select, giữ tối đa ba chủ đề và giữ ô tìm/chip đã chọn khi gập bảng. Định dạng giữ
  xem trước đích, Hủy và Áp dụng. Công tắc Công khai chỉ đổi ý định; trạng thái áp dụng và tên bản
  đang chờ duyệt lấy từ máy chủ, chỉ thay đổi qua hành động riêng hiện có. Bảng gập vẫn báo Chờ duyệt.
- Tác giả thử lưu lại tại thanh lưu. Xử lý xung đột chữ mở dialog có “Ở lại viết” và “Lưu đè bản
  của tôi”. Guard rời hai bước và các lựa chọn xử lý xung đột thẻ tiếp tục dùng các owner hiện có.
  Màu và cỡ chữ (100%/150%) chỉnh tại `/toi`; 200% dùng cho kiểm stress.

## Delta đối với nút con mắt trong r4

Bỏ nút con mắt và hộp thoại Xem trước bản nháp. Markdown Live Preview vẫn nằm ngay trong
EditorView. Dialog đổi định dạng vẫn xem trước kết quả chuyển. Reader và màn kiểm duyệt vẫn
render snapshot đúng quyền. Nháp riêng và snapshot kiểm duyệt giữ quy tắc không tính lượt xem
([PUB-01, VIEW-05](../../prd/requirements.md)).

## Kiểm chứng và phần tiếp theo

Ảnh bàn giao thuộc preview. Kiểm chứng app thật, build, trình duyệt và đối chiếu hình học
ED-INV-01..06 ở 1440/1024/390/320 px, hai theme và 100/150/200% ở lượt code draft này là `NOT_RUN`.
Kịch bản Web hiện có được cập nhật cho trạng thái gập/mở, giữ editor và query khi resize, DOM/AX
sau khi bỏ preview độc lập, guard và giữ bản gửi đã ACK. [#476](https://github.com/loveoverflowcom/vink/issues/476)
theo sau trên head đã tích hợp để audit phần CSS và contract cũ còn caller hoặc cần dọn.
