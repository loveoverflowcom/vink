# WHY · Những lựa chọn của R4.3

Cơ chế hiện có nằm trong [HOW Bàn viết](how-ban-viet.md) và [HOW Khám phá](how-kham-pha.md).
Nguồn rationale chính là [Epic #401](https://github.com/loveoverflowcom/vink/issues/401),
[lời chốt DQ trong audit R4](../redesign-r4-ban-viet-kham-pha.md#8-sửa-theo-thiết-kế-hiện-hành-vs-cần-quyết-định)
và [PR #415](https://github.com/loveoverflowcom/vink/pull/415). Source được đối chiếu tại
`develop@2712145b08a6a4c7a12c1d57033ecd2a5bd33b26`.

| Lựa chọn | Lý do và nguồn | Đánh đổi / khi cần xem lại |
| --- | --- | --- |
| Nền liên tục, thẻ rời và control bán vuông | **RECORDED.** Chủ sản phẩm muốn rail/thanh dưới cùng nền, thẻ sáng hơn nền nhưng không trắng tinh, ít chữ và góc đều. R4 §4/§6 ghi hướng Reply và vai trò token từ #402. | Khe và bo tạo phân cấp khi màu nền chỉ chênh nhẹ. Kiểm outline, trạng thái chọn và click target ở hai theme, cửa sổ hẹp và forced colors. |
| H1 Khám phá hiện, tiêu đề trang công cụ nhỏ và đậm (DQ-01) | **RECORDED.** R4 §4/§8 thay lựa chọn ẩn H1 của #308. Vai trò `type.page-title` giữ khác biệt giữa tên trang và tên tác phẩm. | Tốn một hàng hoặc bề rộng công cụ. Chỉ đổi khi ma trận 320/390 chứng minh không bố trí được logo và công cụ. |
| SearchDock mọi cỡ màn, ít chữ có nghĩa (DQ-02/12) | **RECORDED.** #401 và R4 §4 lấy tìm theo yêu cầu để giảm bề rộng header. [#426](https://github.com/loveoverflowcom/vink/pull/426) giữ lối bàn phím, Back và ô tìm riêng. | **INFERRED.** Ẩn ô làm kém dễ phiển thị trên touch, nên tên truy cập, focus return và hát hiện q đang áp dụng cần có kiểm riêng. Giữ nghĩa nhãn riêng tư/duyệt/công khai khi rút chữ. |
| Nhóm/menu chế độ dùng link (DQ-03) | **RECORDED.** R4 §8 chọn nhóm nối liền có dấu chọn, gập khi thiếu chỗ. #415 giữ link và flag. | Hai bộ link dễ bị lộ cả hai ở breakpoint. #427 cần kiểm Tab/AX tree thay vì chỉ so ảnh. |
| Lọc thẻ gọn và ngày ở cả hai luồng (DQ-07/08/09/10) | **RECORDED.** R4 §8 chọn chip bề rộng theo chữ, “Bỏ chọn” local, ngày công khai đầu có dữ kiện, avatar tròn, tertiary cho Lọc thẻ và dòng thể loại. | Dùng ngày đầu theo múi giờ chung giúp kết quả SSR/hydrate nhất quán. Giữ chỗ cho tên dài, thiếu ngày và 0/4 thẻ. Tertiary ở Khám phá chưa quyết tone “Chờ duyệt” ở Desk. |
| Bỏ Tải lại thường trực nhưng giữ recovery | **RECORDED.** R4 §4 ghi yêu cầu bỏ nút và dùng visibility + `wake.due`. #426 giữ thử lại, đối soát tạo, về phần đầu. | **INFERRED.** Trải nghiệm gọn phụ thuộc vào hành động phục hồi ngay tại lỗi và hiển thị rõ dữ liệu cũ. Kiểm mất mạng, cursor hết hiệu lực, request muộn và change account. |
| Select định dạng và nút tạo (DQ-06) | **RECORDED.** “Giữ Select hiện tại” trong [R4 §8](../redesign-r4-ban-viet-kham-pha.md#8-sửa-theo-thiết-kế-hiện-hành-vs-cần-quyết-định) và [#426](https://github.com/loveoverflowcom/vink/pull/426). Hai control dùng cùng radius theo chiều cao. | Tốn thêm chiều ngang, nhưng hành động tạo và chỉnh định dạng dễ phân biệt. So lại Tab và hai định dạng ở first-use khi đổi bố trí. |
| Trạng thái Bàn viết và thẻ tác phẩm | **RECORDED.** [#429](https://github.com/loveoverflowcom/vink/pull/429) giữ từng dữ kiện và metric, tách lối Đọc. [#430](https://github.com/loveoverflowcom/vink/pull/430) dùng EmptyState và skeleton. | **UNKNOWN.** Chưa có lời chốt về ẩn công cụ lúc kho rỗng (DQ-04), rút timezone (DQ-05) hoặc tone “Chờ duyệt” (phần còn lại DQ-10). Code hiện hành và đề xuất trong comment #411 là hai nguồn khác nhau. |
| Rail, Giao diện và cỡ chữ (DQ-11) | **RECORDED.** [#413](https://github.com/loveoverflowcom/vink/issues/413) thay phần “giữ Giao diện”: chỉnh trong Tôi, Mặc định 100% / Lớn 150%, legacy 200 → Lớn. | Đổi preference tác động ngưỡng #415 và lối đổi trong lúc viết. #427 giữ phép kiểm riêng, #428 giữ phần thiết kế editor. **UNKNOWN.** Không có lời chốt lối tắt mới trong editor tại đây. |
| CHOICE-01 B · LayoutSpec riêng | **RECORDED.** [Comment kết quả #414](https://github.com/loveoverflowcom/vink/issues/414#issuecomment-7102229465) chọn một spec hiện hành nhỏ, tách khỏi audit dài. | **RECORDED.** Thêm một file và duy trì banner ở audit. **INFERRED.** Dẫn cùng spec từ audit, design index và handoff giúp người đọc tránh dùng hai anatomy song song. Bản gốc `2c4f5fb` vẫn unavailable. |

## Quyết định và sổ nguồn

- DQ nêu trên có phạm vi UI issue và lời chốt dẫn từ audit. LayoutSpec R4.3 vẫn `DESIGN_PROPOSED`,
  `fidelity: NO_APPROVED_REFERENCE`. DQ-04, DQ-05 và phần Desk của DQ-10 vẫn `OPEN`.
- [Registry author feedback](../../development/desk-author-feedback.md) ghi `O-07@r1 OPEN` về
  chính sách metric qua revision. Giữ chỉ số đang có theo bản công khai và không suy chốt chính
  sách dài hạn từ #429.
- [VDR-WEB-TAILWIND-FOUNDATION@r1](../../../decisions/proposals/VDR-WEB-TAILWIND-FOUNDATION.r1.toml)
  là kiến trúc stylesheet và vẫn `PENDING`. [VDR-DISCUSSION-R4@r1](../../../decisions/README.md#nhận-xét-r4)
  thuộc Nhận xét, ngoài lát này. Giữ các revision liên quan. Không cần proposal TOML mới cho
  bản hòa giải bố cục này vì nó dẫn lại lựa chọn đã ghi và không thay policy/giá trị token.
