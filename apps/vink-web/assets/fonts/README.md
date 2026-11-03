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
phục vụ chúng như các file tĩnh khác (`content-type: font/woff2`). Danh sách face nằm trong
mục `fonts` của [tokens.json](../../../../packages/ui-system/shared/tokens.json); `cargo xtask
ui-tokens` sinh `@font-face` vào `tokens.css` (phục vụ đầu `/pkg/vink.css`) với `url("fonts/…")` và `font-display: swap`: chữ hiện ngay bằng fallback, rồi
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
3. Thêm file vào `provenance.json` và vào mục `fonts` của `tokens.json`, rồi chạy
   `cargo xtask ui-tokens`; kèm OFL của family mới.

`cargo xtask verify quick` kiểm mọi face/license khớp SHA-256 trong `provenance.json`, mọi
`.woff2` đều được ghi nhận, và mọi URL font trong `tokens.css`/`vink.css`/shell trỏ tới face đã ghi.
Preview CMP Desktop dùng TTF giải nén từ chính ba WOFF2 này, xem phần dưới;
Android/iOS vẫn dùng font hệ thống, chưa có pipeline branded-font native.
`cargo xtask verify web` kiểm trang đọc tải được hai face chính và file được phục vụ đúng
`font/woff2`. Các lane này không chứng minh vị trí dấu, cắt chữ hay hiển thị trên thiết bị thật;
những điều đó cần ảnh chụp đã mở ở viewport/theme/cỡ chữ cụ thể.

## Adapter CMP Desktop

Skiko 0.9.22.2 trên Linux không giải mã được ba WOFF2 đã kiểm hash, trong khi TTF giải nén
từ cùng dữ liệu đọc được. Desktop đóng gói ba derivative trong
[`preview/fonts`](../../../../packages/ui-system/cmp/preview/fonts), tổng **844 748 bytes**;
Gradle chép chúng, OFL và provenance vào classpath `vink-fonts/`. Web tiếp tục dùng các WOFF2
trong bảng trên. Không cần Python hoặc mạng khi build/chạy preview.

`files.*.desktop` trong [provenance.json](provenance.json) là nguồn duy nhất của tên, byte count
và hash derivative; không có manifest font thứ hai. [derive_desktop.py](derive_desktop.py) giải
nén bằng API [fontTools woff2](https://fonttools.readthedocs.io/en/latest/ttLib/woff2.html), không
subset/instance lại. Script kiểm mọi bảng OpenType giữ nguyên, ngoại trừ checksum `head` được
tính lại; các field còn lại của `head` cũng được so sánh. Font/license/nguồn Google Fonts và các
trục Literata `opsz` 7–72, `wght` 200–900 được giữ nguyên. OFL cho phép derivative này.

Để tái tạo, cài `fonttools==4.68.0 brotli==1.2.1` trong môi trường Python riêng rồi chạy
`python3 apps/vink-web/assets/fonts/derive_desktop.py` từ repo; thêm `--check` để chỉ đối chiếu
byte, không ghi. Các phiên bản và lệnh cũng được pin trong provenance. `verify quick` kiểm
format/length/hash TTF và file không có chủ; Desktop tests kiểm resource, metadata, 154 điểm mã
tiếng Việt NFC/NFD, font được Compose dùng ở 400/600 và fallback missing/corrupt. Glyph có sẵn
không tự chứng minh shaping/IME đúng; xem [báo cáo PR 08](../../../../docs/design/ui-repair-08.md).
