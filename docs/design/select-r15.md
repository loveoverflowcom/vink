# Select Web R1.5 — #135

Người viết chọn thứ tự danh sách tại Bàn viết. Giữ native select, nhãn “Sắp xếp”,
hai option và authority sắp xếp hiện có. R1.5 là reference cho hình học control;
Editor tiếp tục dùng TagSelector của #99.

```text
Field — nhãn native liên kết bằng for/id
  Control — relative, tonal, chiều cao tối thiểu theo density
    select — focus/invalid/disabled và option do trình duyệt sở hữu
    chevron — decorative, pointer-events none, slot 18 × 18 CSS px
```

Chevron cách inline-end 12 CSS px, cân tâm dọc; chừa 8 CSS px từ mép vùng nhãn.
Padding inline-start 12 CSS px. Các khoảng hình học cố định khi tăng chữ; chữ
vẫn tăng 100% → 200% theo token label hiện có (0.875rem/600), đúng recipe của
HTML R1.5. Chiều cao tăng theo nội dung/density, không cắt dấu tiếng Việt.

Đo trước tại `develop@84e635f`: app và gallery cùng dùng icon 24px, inset/gap
12/8 ở chữ100 nhưng thành 24/16 ở chữ200. Ở app 320/chữ200, nhãn cần
203.65px trong vùng 145px. Chọn CHOICE-01 A: sửa đúng shared CSS, không thêm
caller override hoặc custom dropdown. Tâm dọc vốn đúng, tiếp tục giữ nguyên.

Owner: `packages/ui-system/leptos/style/vink-ui.css`; markup Select tại
`packages/ui-system/leptos/src/field.rs`; caller tại `apps/vink-web/src/pages/desk.rs`.
Không đổi Rust, option, sort semantics, tokens chung hay CMP.

Kiểm 320/390/1440 × sáng/tối × chữ100/200, native type-ahead, focus, disabled,
invalid và forced-colors qua các runner hiện có. Trong forced-colors, chevron
dùng ButtonText hoặc GrayText của hệ thống theo trạng thái native disabled.
[Bằng chứng và giới hạn](../development/select-r15-evidence.md).

Nguồn: [issue #135](https://github.com/loveoverflowcom/vink/issues/135),
[LayoutSpec R1.5](https://drive.google.com/file/d/1LoY06vmY4RvVcNse0f2-siQiDT8pT1mm/view),
[HTML R1.5](https://drive.google.com/file/d/1pkBsJZIwbv6NDVcndE2m5qbY9INO441M/view).
