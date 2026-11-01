# Font tự host của Vink Web

Ba face phục vụ giao diện tiếng Việt đã chọn trong [baseline thiết kế](../../../../docs/design/README.md):

| File | Family / weight | Vai trò | Bytes |
| --- | --- | --- | --- |
| `be-vietnam-pro-400.woff2` | Be Vietnam Pro 400 | Chữ giao diện, đoạn hướng dẫn, metadata | 36 464 |
| `be-vietnam-pro-600.woff2` | Be Vietnam Pro 600 | Nhãn, nút, tiêu đề nhỏ | 38 844 |
| `literata-variable.woff2` | Literata, trục `wght` 200–900 và `opsz` 7–72 | Văn đọc, editor, tiêu đề tác phẩm | 256 620 |

Tổng 331 928 bytes, so với 1 365 116 bytes của bốn TTF trong prototype. Nguồn là các file
TTF chưa sửa trong [google/fonts](https://github.com/google/fonts) tại commit pin; SHA-256
nguồn trùng với `assets/fonts/manifest.json` của bộ thiết kế. [provenance.json](provenance.json)
ghi đường dẫn nguồn, Git blob SHA, SHA-256 trước/sau, lệnh chuyển đổi, phiên bản công cụ và dải
Unicode. Cả hai font dùng SIL Open Font License 1.1 ([Be Vietnam Pro](OFL-BeVietnamPro.txt),
[Literata](OFL-Literata.txt)); không khai báo Reserved Font Name nên được phép subset và giữ tên.
Bảng `name` trong mỗi WOFF2 vẫn giữ bản quyền và trường license.

## Cách tải

`cargo xtask build` chép các file `.woff2` và `OFL-*.txt` vào `target/site/pkg/fonts/`; server
phục vụ chúng như các file tĩnh khác (`content-type: font/woff2`). `vink.css` khai báo
`@font-face` với `url("fonts/…")` và `font-display: swap`: chữ hiện ngay bằng fallback, rồi
đổi sang font khi tải xong; nếu tải lỗi, trang vẫn đọc được bằng stack fallback (Noto, Segoe UI,
Georgia…). Shell SSR preload Be Vietnam Pro 400 và Literata vì mọi trang hiện có đều dùng hai face
này ở phần đầu trang; face 600 để trình duyệt tự phát hiện qua CSS. Trình duyệt chỉ tải một face
khi có chữ dùng đến nó.

Chưa có Be Vietnam Pro 700 hay italic vì chưa có consumer; CSS yêu cầu 700 sẽ dùng face 600.
Literata bỏ Greek/Cyrillic: các ký tự đó dùng font serif fallback. Server chưa
đặt `Cache-Control` hay tên file có hash; chính sách cache/CDN production là việc riêng.

## Thêm hoặc tạo lại một face

1. Lấy TTF từ google/fonts tại một commit cụ thể; ghi lại Git blob SHA và SHA-256.
2. Chuyển đổi bằng lệnh trong `provenance.json` (fontTools + brotli), cùng dải Unicode. Chạy lại
   với cùng phiên bản công cụ cho ra đúng các byte đã ghi.
3. Thêm file vào `provenance.json` và khai báo `@font-face` trong `vink.css`; kèm OFL của family mới.

`cargo xtask verify quick` kiểm mọi face/license khớp SHA-256 trong `provenance.json`, mọi
`.woff2` đều được ghi nhận, và mọi URL font trong `vink.css`/shell trỏ tới face đã ghi.
`cargo xtask verify web` kiểm trang đọc tải được hai face chính và file được phục vụ đúng
`font/woff2`. Các lane này không chứng minh vị trí dấu, cắt chữ hay hiển thị trên thiết bị thật;
những điều đó cần ảnh chụp đã mở ở viewport/theme/cỡ chữ cụ thể.
