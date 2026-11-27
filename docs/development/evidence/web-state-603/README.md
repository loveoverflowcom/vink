# Kiểm chứng UI State System #603

Base sau #627 là `1cc19e70851b23ab74ee74d942a7f5c6560ea773`. Mã Product được kiểm tại `6ef16a77c811c46fec89d768da0df63daba4599e`. Các ảnh dưới đây dùng Chrome thực, fixture tổng hợp của runner local và database PostgreSQL tạm. [Manifest sau sửa](captures.json) ghi route, SHA source, SHA-256 PNG, viewport, text scale và theme của từng ảnh.

| Case | Trạng thái trước | Product sau sửa |
| --- | --- | --- |
| c049 · guest Light | [Trang Thơ](https://drive.google.com/file/d/1fTEEpegd3-C0NA-K8Q5USDItSIAcUPc2/view) | [ArrowBack, H1, search và artwork](603-c049-anonymous-390-100-light.png) |
| c306 · authenticated Dark | [Route kiểm duyệt đã ngừng](https://drive.google.com/file/d/18d649XT-DpfPwj5GlKeeyddWcE26lzdW/view) | [NotFound và Explore leading](603-c306-390-dark-100.png) |
| c320 · guest Dark | [Đường dẫn không tồn tại](https://drive.google.com/file/d/1akDZhhk5FzLCT6yiiN4n0TGQUXbr3oky/view) | [NotFound](603-c320-390-dark-100.png) · [Focus ở 320 × 540, chữ 200%](603-c320-320-dark-200.png) |

Ba ảnh trước thuộc develop `91191186c1ac5ec6f4032dc0292af025eb2c0458`, 390 × 844 CSS px, chữ 100%, fixture tổng hợp. [Manifest lịch sử](https://drive.google.com/file/d/1aSRfwKgtDpwg8leGGswwpEBOT0_O6H7J/view) · [Mapping file ID](before.json).

[Lỗi đọc lần đầu ở Inbox](notifications-first-error-390-100-light.png) · [Sheet chọn một thẻ](chip603-390-844-light-100-1.png).

## Lệnh local

Từ checkout chính Vink trên Mac mini, từng lane chạy tuần tự với một Cargo job, một test thread, debug info và incremental tắt. Browser sử dụng bản build dev SSR + WASM hydrate. c049 dùng local App guest và App tác giả đọc cùng catalog. Notifications dùng fixture transport có kiểm soát để giữ hoặc trả lỗi request. Luồng bàn phím/IME được kiểm bằng Chrome DevTools Protocol.

| Lệnh | Kết quả |
| --- | --- |
| `cargo xtask verify web --scenario notifications` | PASS · page, popover, session fence, lỗi initial/partial, retry, quyết định và 404 |
| `cargo xtask verify web --scenario reader` | PASS · SSR, hydrate, native GET/no-JS, bàn phím, CDP IME, hai phiên thực và c049 |
| `cargo xtask verify web --scenario discovery` | PASS · catalog thật, A→B, legacy AND, pagination, reload, focus, forced colors và phản hồi trễ |
| `cargo xtask verify web --scenario appearance` | PASS · Account H1/Title, cỡ chữ, tên dài, rail và bottom nav |
| `cargo xtask verify web --scenario return-position` | PASS · 13 ca, 50 vòng, fixture 200 tác phẩm và 1.000 nhận xét |
| `cargo test --locked -p vink-web --features ssr --lib` | PASS · 326 test, 1 benchmark retention đã đánh dấu ignored |
| `cargo test --locked -p vink-ui --lib` | PASS · 74 test |
| `cargo clippy --locked -p vink-web --features ssr --lib -- -D warnings` | PASS |
| `cargo xtask verify preview` | PASS · 938 ảnh ở `9a645cc11f1c2899399dc9d6cee54c781640bdce`. Hai commit Product sau đó chỉ sửa trang thẻ và regression Product. |
| `cargo xtask verify static docs`, `cargo xtask decision validate`, `cargo fmt --all -- --check`, `git diff --check` | PASS |

Ở c320 với chữ 200%, sau Tab cạnh dưới CTA nằm tại 387,94 px và thanh dưới bắt đầu ở 396 px. Cả CTA và viền focus đều còn nhìn thấy. Trong sheet, tỷ lệ tương phản chữ nhỏ nhất đo được là 11,87:1 Light và 8,16:1 Dark. Các số đo này thuộc fixture Chrome của lượt kiểm.

## Acceptance

| AC | Kiểm chứng |
| --- | --- |
| 01 | Đối chiếu ảnh gốc, route owner, API và consumer tại base #627. LayoutSpec và inventory trong cùng diff. |
| 02 | Notifications chạy lỗi lần đọc đầu tại page và popover ở Light/Dark. Heading H2/H3, glyph Error, retry bằng Enter. Giữ phản hồi đầu để kiểm aria-busy, focus và số request. |
| 03 | Chrome giữ các hàng, snapshot và unread qua lỗi phân trang/mark-all. Kiểm ACK, scope A→B→A và retry đếm. |
| 04 | Guest Inbox dùng NotFound chung. Hai đường 404 được kiểm bàn phím, hit-test và toàn bộ viền focus ở hai viewport điện thoại, 100/150/200% và desktop. |
| 05 | URL quyết định trực tiếp có native Up về Inbox, tên đọc được, ArrowBack và target 48 px. Kiểm browser Back, Escape và trả focus về chuông. |
| 06 | Chrome mở Tất cả, Đang theo dõi, Tác giả với từng loại phiên. Chỉ Tất cả có nút lọc. Hình học và contrast của icon được đo trong Reader. |
| 07 | Link ở cả hai nhóm catalog thay A bằng B, giữ q, bỏ cursor. Kiểm sheet, Escape/Back, áp dụng, × và native GET trước hydration. |
| 08 | Reader và Discovery dùng API/corpus local, kiểm alias, retired, unknown, hai nhãn trên URL AND cũ, phân trang, reload và phản hồi A đến sau B. |
| 09 | LayoutSpec, bản ghi triển khai và inventory đã cập nhật. Mở lại ảnh Product và đối chiếu dữ liệu viewport, theme, text scale, SHA và checksum. |
| 10 | Một PR vào develop gồm code, test, tài liệu, ảnh và manifest. |
| 11 | SSR và Chrome kiểm một link tonal, href do caller cấp, SVG Explore trang trí đứng trước nhãn. Cả c306 và c320 giữ HTTP 404. |
| 12 | c049 giữ ArrowBack về parent, H1 ở hàng riêng, nhãn “Tìm trong thẻ”, ô tìm và landmark. Luồng Reader kiểm native GET và IME. |
| 13 | c049 guest cùng hình artwork, c320 guest và c306 authenticated đúng viewport reference. Khi q có giá trị, xóa q giữ thẻ, xóa thẻ giữ q. |
