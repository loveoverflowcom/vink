# Pen Flight R2 — CHOICE-07 A · Mềm

10/11/2026 · lát UI riêng của [#73](https://github.com/loveoverflowcom/vink/issues/73),
nền `develop@1329642` sau #139. Thiết kế chốt 09/11/2026, không mở lại policy LVE.

Nguồn: [SVG R2](https://drive.google.com/file/d/1KeBVl1G0HwP8T91z2L38ELpgngLaiGKI/view),
[LayoutSpec R2](https://drive.google.com/file/d/1shsfRkf3dN_qcNYQTO5eef-PTbR_d36H/view),
[desktop R1.3](https://drive.google.com/file/d/1G18GyQHrmYdxplNd9kdsn05hKRjl5AW8/view),
[ảnh mẫu sáng/tối](https://drive.google.com/file/d/1gcxDRPZcWMHVzqDjHsVU6Zz_HWCpa7Iz/view).
Ảnh mẫu đã mở để đối chiếu; dữ liệu hư cấu không là bằng chứng ghi thật.

```text
#reader-body → #work-endorsement
  Pen Flight · Đánh giá cao · số của revision · slot tiến trình/✓
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

## Specimen hẹp

Sau kiểm ảnh 320px/chữ200, riêng gallery Leptos bỏ padding lồng của Surface và
indent thừa của replies dưới 36rem. Đây là chrome của specimen: giữ ngữ nghĩa ul/li,
nhãn phản hồi và đủ chiều rộng đọc từng từ, không đổi pattern/CSS của app.
Kiểm lại các từ bằng DOM Range và ảnh focus cùng viewport/theme/text scale.
