# Logo Vink — selected pack #34

Ngày 03/11/2026, người dùng giao tích hợp bộ `vink_logo_final_selected` và tạo PR.
Nguồn đóng gói là [Vink_Logo_Final_Selected_Pack.zip](Vink_Logo_Final_Selected_Pack.zip)
(SHA-256 `46acba3f22e5b684a1a72f4518d3de6cce0906f0511ef11472a92a056008eac4`).
ZIP giữ nguyên 24 tệp và tài liệu bàn giao gốc; README/issue update bên trong là tư liệu lịch sử
của lần thiết kế, không mô tả trạng thái tích hợp hiện tại. Tracking: [#34](https://github.com/loveoverflowcom/vink/issues/34).

## Nguồn triển khai và độ trung thực

Source production là **SVG/PNG trong selected pack** theo chỉ dẫn người dùng; không tái tạo
logo từ ảnh board. Sáu SVG và 12 PNG đã được chép nguyên byte vào
[assets/brand](../../../../apps/vink-web/assets/brand/README.md), kèm
[manifest SHA-256](../../../../apps/vink-web/assets/brand/provenance.json) của toàn bộ ZIP.
Các file này có URL GitHub theo commit của PR; ZIP vẫn là nguồn phục hồi đủ 24 tệp.

[Bảng review](vink-final-review-board.png) có highlight, mặt cắt/khối màu và tỷ lệ vùng
ngòi khác asset vector. Đây là **ảnh trình bày tham khảo**, không phải bản raster pixel-match
với source production. [Preview kỹ thuật](vink-logo-v4-preview.png) thể hiện SVG được chọn
sát hơn. PR giữ hình học và gradient của SVG/PNG, không nhận board và asset là tương đương.
Việc tích hợp này không xác nhận người dùng đã nghiệm thu app hay launcher thật.

- [SVG Light](../../../../apps/vink-web/assets/brand/vink-mark-light.svg) /
  [Dark](../../../../apps/vink-web/assets/brand/vink-mark-dark.svg).
- [Compact Light](../../../../apps/vink-web/assets/brand/vink-mark-light-compact.svg) /
  [Dark](../../../../apps/vink-web/assets/brand/vink-mark-dark-compact.svg).
- PNG Light/Dark đủ 24/32/48/96/192/512px ở cùng thư mục runtime; 24/32 dùng compact.
- Mono và mono compact giữ nguyên trong runtime và ZIP, chưa đưa vào consumer.

Sáu SVG trùng hash bản V4 cũ đã lưu tại commit `ef6c16443695eb19d5e85225d6b71ef370a6d0a9`;
điểm mới là quyết định selected pack, lưu đủ bộ và tích hợp thay logo Enso #10/PR #17.
Không dùng yêu cầu lông vũ/crop raster của #10 để sửa lại bộ mới.

[Bằng chứng tích hợp Web và giới hạn](evidence/README.md) lưu summary, HTTP QA và ảnh đã mở.
