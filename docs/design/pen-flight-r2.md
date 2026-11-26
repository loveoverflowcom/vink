# Pen Flight R2 — CHOICE-07 A · Mềm

10/11/2026 · lát UI riêng của [#73](https://github.com/loveoverflowcom/vink/issues/73),
nền `develop@1329642` sau #139. Thiết kế chốt 09/11/2026, không mở lại policy LVE.

<!-- vdr: active LVE-WP-04-CHOICE-07@r1 -->

Nguồn: [SVG R2](https://drive.google.com/file/d/1KeBVl1G0HwP8T91z2L38ELpgngLaiGKI/view),
[LayoutSpec R2](https://drive.google.com/file/d/1shsfRkf3dN_qcNYQTO5eef-PTbR_d36H/view),
[desktop R1.3](https://drive.google.com/file/d/1G18GyQHrmYdxplNd9kdsn05hKRjl5AW8/view),
[ảnh mẫu sáng/tối](https://drive.google.com/file/d/1gcxDRPZcWMHVzqDjHsVU6Zz_HWCpa7Iz/view).
Ảnh mẫu đã mở để đối chiếu; dữ liệu hư cấu không là bằng chứng ghi thật.

Delta copy 15/11/2026 ([#213](https://github.com/loveoverflowcom/vink/issues/213)): tác phẩm dùng
“Thấm”, tên truy cập “Thấm”/“Bỏ Thấm”, count-unit “lượt Thấm”; nhãn luôn ổn định.
Icon, phase, count/order/session và “Hữu ích” giữ nguyên. Bằng chứng R2 bên dưới là lịch sử,
không tự trở thành bằng chứng cho delta mới.

Delta 20/11/2026 ([#359](https://github.com/loveoverflowcom/vink/issues/359)): bình luận dùng “Ưng”
với `heart-outline`/`heart` theo giá trị đã xác nhận; Pen Flight chỉ còn ở “Thấm”. Cây dưới đây giữ
dạng R2 làm lịch sử ([delta thuật ngữ](../architecture/literary-endorsement.md#delta-thuật-ngữ-20112026--359)).

```text
#reader-body → #work-endorsement
  Pen Flight · Thấm · số của revision · slot tiến trình/✓
#discussion root / flat reply → hàng thao tác
  Trả lời · [Pen Flight · Hữu ích · số của comment · slot tiến trình/✓]
#composer → Gửi (mũi tên ngang hiện hành)
```

Giữ viewBox 24×24 và path SVG nguyên văn, không raster hay icon font. Một path đơn sắc,
currentColor, hiển thị 20px trang trí (`aria-hidden`, không thêm tên truy cập). Ngòi bút
hướng lên, lỗ thở tròn; không tim. Vector không đổi theo phase. Vùng chạm thoải mái 48px;
nhãn xuống dòng khi hẹp/chữ200 theo ToggleAction, mật độ gọn vẫn theo contract hiện có.

Owner: `shared/icons.json` lưu source URL, revision, Vink ownership và SHA-256 SVG gốc;
`xtask/src/ui_tokens.rs` kiểm provenance/hash rồi sinh Rust/CMP. Không gán license của
Material cho glyph Vink. `EndorsementAction` Leptos/CMP nhận glyph qua slot có sẵn.
App adapter giữ count, desired-state, quyền, command key/order/session, vị trí và thứ tự;
slot cuối vẫn dành cho tiến trình hoặc ✓ sau ACK. Selected dùng action.toggle.* hiện có.
Khách/chặn vẫn có lý do bằng chữ; số không khả dụng là “–”; uncertain chỉ đối soát theo owner.
Không thêm animation, backend, ranking, auth hoặc native product consumer.

Đường kiểm: generator/hash negative, quick, Web SSR/hydration và real LocalFixture writes,
preview Leptos sáng/tối, 320/390/1440 và chữ200; CMP common/desktop báo riêng.
Kết quả và giới hạn: [evidence](../development/pen-flight-r2-evidence.md).

## Cặp Thấm · Lưu trên Reader

Delta 25/11/2026, D1/D4/D5 của [Epic #384](https://github.com/loveoverflowcom/vink/issues/384). Sau thân bài có một nhóm nối, thứ tự **Thấm + số → Lưu**, tiếp đến phản hồi và liên kết danh sách người Thấm. Số Thấm cạnh chữ trên một dòng. Hai nút cùng kích thước, cao 56 px ở 100%, khe 2 px, nội dung chưa chọn căn giữa. Pen Flight giữ path và viewBox 24×24, hiển thị 24 px trong nhóm (20 px khi hẹp); khi chọn dùng hình viên thuốc và ✓. Token chữ `on-surface` chỉ áp dụng trong nhóm.

Delta thay bố cục nút Thấm ở cuối Reader; glyph, count theo revision, phase/ACK, order/session và Ưng ở bình luận theo owner hiện hành. [LayoutSpec Lưu bài](saved-works.md) sở hữu responsive, motion, focus, sheet và trạng thái. Bằng chứng R2 ở trên thuộc phiên bản trước khi ghép cặp. Nhóm đích đã triển khai ở #388, kết quả và giới hạn ghi trong [registry Lưu bài](../development/saved-works.md#reader-web-388).

## Specimen hẹp

Sau kiểm ảnh 320px/chữ200, riêng gallery Leptos bỏ padding lồng của Surface và
indent thừa của replies dưới 36rem. Đây là chrome của specimen: giữ ngữ nghĩa ul/li,
nhãn phản hồi và đủ chiều rộng đọc từng từ, không đổi pattern/CSS của app.
Kiểm lại các từ bằng DOM Range và ảnh focus cùng viewport/theme/text scale.
