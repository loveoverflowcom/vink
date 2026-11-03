> **REJECTED / KHÔNG TRIỂN KHAI — 03/11/2026.** Người dùng từ chối V3 vì thân vòng không tròn và các nét lệch/chồng nhau. Giữ thư mục này chỉ làm lịch sử; bản sửa mới ở [logo-v4](../logo-v4/README.md), vẫn chờ review trong [issue #34](https://github.com/loveoverflowcom/vink/issues/34). Nội dung dưới đây là hồ sơ cũ, không phải phê duyệt hiện hành.

# Vink logo v3 — vector proposal / review only

Status: **PROPOSAL**, chưa thay logo production. Source image là biểu tượng vòng mực/ngòi bút được chọn trong cuộc trò chuyện ngày 03/11/2026.

## Files
- `vink-mark-light.svg` / `vink-mark-dark.svg`: cùng hình học và khác palette.
- `vink-mark-*-compact.svg`: bỏ nét lông mảnh cho cỡ <=32px.
- `vink-mark-mono.svg`: đơn sắc với `currentColor`.
- `preview.svg`: bảng so sánh light/dark và độ rõ cỡ nhỏ, render ngay trong GitHub.
- `preview.html`: preview tương tác offline (download cả các SVG cùng thư mục).

SVG 256×256 viewBox; nền trong suốt, các path Bézier tự vẽ, không có ảnh raster, font nhúng hay CSS filter. Vòng mực đậm hơn, phần ngòi bút nối liên tục (không gãy nét). 2 hình khối Bézier chính, 2 gradient nhẹ cho bản nhiều màu.

## Quy định trước khi thay logo thật
- Có phê duyệt thiết kế v3; tham chiếu issue [#10](https://github.com/loveoverflowcom/vink/issues/10) là quyết định logo v2 trước đó.
- Dùng token từ `packages/ui-system/shared/tokens.json`, không tạo màu hardcoded trong component.
- Kiểm visual 16/24/32/36/48/96/192/512px, light/dark, theme override & first paint.
- Kiểm header/rail/mobile/favicon/webmanifest/launcher, không layout shift và không regression navigation/auth.
- Cần kiểm thực tế trong app, không đánh dấu PASS khi mới chỉ render asset.
