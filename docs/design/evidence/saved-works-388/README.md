# Lưu bài trên Reader · #388

- Base `f9c14d0b` (`develop`), source là working tree của PR này trước khi commit. Code, CSS và
  kịch bản của commit giống hệt bản đã chạy; sau lượt chạy chỉ thêm tài liệu và ảnh trong thư mục này.
- Lệnh: `cargo xtask verify web --scenario standalone` (artifact CSR, App API-only, Identity
  native, PostgreSQL 17.11 scratch) và `cargo xtask verify web --scenario auth` (App SSR +
  hydration, Identity, mã email qua outbox cục bộ). Chrome 158.0.8127.89 headless/CDP, macOS.
- Kịch bản: [saved-works-smoke.mjs](../../../../apps/vink-web/e2e/saved-works-smoke.mjs).
  Mất ACK là giữ rồi cắt câu trả lời thật sau khi server commit; không có câu trả lời thành công
  nào được dựng giả. Riêng phép đo số Thấm 4 và 7 chữ số thay chữ số trong DOM
  (`dom-override-geometry-only`).
- Chữ 150/200/300% là phóng font gốc (300% = chữ “Lớn” 150% của app × 200% trình duyệt),
  không phải zoom. [LayoutSpec](../../saved-works.md) · [registry](../../../development/saved-works.md#reader-web-388).

## Ảnh đã mở

| Ảnh | Đối chiếu |
| --- | --- |
| [390 sáng, đã lưu](reader-save-390-light-100.png) | Nhóm nối cao 56, hai nửa bằng nhau, Thấm góc ngoài 28/trong 8, Lưu đã lưu thành viên thuốc + bookmark đặc + ✓; dòng “Đã lưu vào hồ sơ của bạn.” + “Mở Đã lưu”. |
| [390 tối, đã lưu](reader-save-390-dark-saved.png) | Cùng cấu trúc ở theme tối; chữ on-surface trên nền neutral container, nút chọn secondary-container. |
| [320 tối, chữ 200%](reader-save-320-dark-200.png) | Host hẹp hơn 20rem nên xếp dọc, cụm “Thấm 0” một dòng, ghi chú xuống dòng, không tràn ngang. |
| [320 sáng, chữ 300%](reader-save-320-light-300.png) | Ca cực đại vẫn xếp dọc, không cắt chữ, không tràn ngang. |
| [1024 sáng](reader-save-1024-light-100.png) | Nhóm rộng 400 px ở 100%, khe 2 px, nằm ngay sau thân bài, trước Nhận xét. |
| [Sheet khách 1280 sáng](reader-save-guest-sheet-1280-light.png) | “Đăng nhập để lưu bài”, lời dẫn, nút chính và hai nút phụ viền 2 px bằng nhau; focus đầu ở Đóng. |

Ảnh R2 cũ của sheet nhận xét (trước delta viền 2 px) ở [auth-r2-328](../auth-r2-328/README.md).
Ảnh sau delta do lane `auth` tạo trong `target/verify-web/<run>/auth/screenshots/`, không commit.
