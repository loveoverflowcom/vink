# Loading và phục hồi trong Product Web · #624

## So sánh Chrome tại 390 × 844, chữ 100%

| Trạng thái giữ trên màn hình | Trước | Sau |
| --- | --- | --- |
| Thành viên Dark, Đang theo dõi, chờ GET riêng | [Chữ đang tải](before/following-390-dark-100.png) | [MicroArc trong vùng feed](after/following-390-dark-100.png) |
| Khách Light, Tất cả, chờ GET public | [Chữ đang tải](before/all-390-light-100.png) | [MicroArc trong vùng kết quả](after/all-390-light-100.png) |
| Khách Light, `/?cursor=khong-hop-le` | [Hai liên kết](before/invalid-390-light-100.png) | [Một hành động về trang đầu](after/invalid-390-light-100.png) |

Bản trước là `58791fa13c16289798f94989ac2f5cf0b3934a78`. Bản sau là `013698f7a973ec6e564128f9172be002c60f9270`. Chrome chạy SPA Product với API fixture trên loopback. Fixture Following mô phỏng phiên active đủ quyền, giữ riêng GET private và trả một tác phẩm. Tất cả giữ riêng GET public. Recovery dùng phản hồi 422 có trường cursor. Trong manifest lịch sử ở SHA `91191186c1ac5ec6f4032dc0292af025eb2c0458`, c058 ghi Following mô phỏng, c053 ghi `simulation: null`. [Manifest lịch sử](https://drive.google.com/file/d/1aSRfwKgtDpwg8leGGswwpEBOT0_O6H7J/view).

[Manifest local](captures.json) có SHA-256 từng PNG, URL, loại fixture, viewport, theme, cỡ chữ tính được, số request, số status/alert, computed animation và tọa độ shell. Mỗi bản có 12 ảnh ở 390 × 844/100%, 320 × 700/200%, 834 × 900/150% và 1440 × 900/100%. Mức 150% dùng thuộc tính cỡ chữ của Product, 200% dùng override font-size trên root. Ảnh trước chụp ở motion thường, ảnh sau bật reduced motion. Hai chế độ đều có phép đo DOM riêng.

## Kết quả

- Cả tám lần giữ kết quả trên bản sau đều có một GET của đúng owner, một status và aria-busy. Arc chạy chu kỳ 1,6 giây ở motion thường. Khi bật reduced motion, computed iteration count là 0. Tọa độ header, tab và navigation trước và sau khi nhả GET trùng nhau.
- Ở 390 px, giữ từng GET quá 8 giây hiện “Vẫn đang tải…”. Đưa tab thật xuống nền làm activity thành false và animation-name thành none. Quay lại tab khôi phục vk-ui-spin. Hai lượt trả lời nhanh 5,9 ms và 2,5 ms đều kết thúc trước khi arc xuất hiện. [Số đo lượt nhanh](fast.json).
- c053 local có đúng một alert, một CTA native và href `/`. Tab đưa focus tới CTA với outline 2 px, tỉ lệ tương phản chữ 11,14:1 trên cả bốn cấu hình. Ở 320 px/200%, cạnh dưới CTA sau khi nhận focus ở 539,78 px, thanh điều hướng bắt đầu ở 556 px. [Ảnh khi nút nhận focus](after/invalid-320-light-200-focus.png).
- `cargo xtask build spa` trên SHA Product sau đạt. Một Cargo job, RSS cây tiến trình lấy mẫu tối đa 3.346.496 KiB. `cargo xtask verify unit web` đạt 328 test, 1 ignored trên cùng SHA Product. `cargo fmt --all -- --check`, kiểm cú pháp bốn runner JS và kiểm tài liệu đạt. Bộ kiểm visual-evidence đạt 11/11.

### Tái hiện kiểm chứng local

Từ root Vink, dùng một compiler job và một test thread. Build SPA và unit qua `tools/local-budget run --low-disk`. Runner Chrome dùng `target/debug/xtask verify web --scenario <tên> --capture full` với PostgreSQL 17.11 scratch trên loopback. Khi chạy đủ Reader, để Reader và Discovery ở hai scratch riêng. Fixture c049 của Reader yêu cầu thẻ thơ trống, còn Discovery cố ý seed một corpus có tác phẩm thơ.

Bốn runner JS thuộc phạm vi là `browser-smoke.mjs`, `discovery-smoke.mjs`, `following-feed-smoke.mjs` và `saved-profile-smoke.mjs`. Mốc `fc324d57` đã vượt Discovery và password/Following trên backend thật sau khi tách wake mạng khỏi sự kiện kiểm lại phiên. Mốc `013698f7` bổ sung trạng thái tải của các vùng liên quan được ghi trong [inventory](../../discovery-ui.md).

## Kiểm với backend local trên SHA Product cuối

| Kịch bản | Kết quả |
| --- | --- |
| Reader | PASS trên scratch riêng, gồm native GET, no-JS, IME mô phỏng, c049 và hành trình Reader/work views. |
| Desk | PASS cho danh sách và trạng thái Bàn viết, giữ focus, kích thước hẹp, tìm kiếm, phục hồi và chữ lớn. |
| Discovery | PASS cho loading public giữ/nhả GET thật, computed motion, HTTP 422 của c053, href lọc/query, phân trang và response A đến sau B. |
| Password/Following | PASS với App, Identity và PostgreSQL riêng. Refresh giữ hàng đang focus, append nhận click/Enter/Space đúng một lần, lỗi mạng và retry, Back và scope A→B→A được kiểm trên phiên thật. RSS cây tiến trình lấy mẫu tối đa 12.541.552 KiB, thấp hơn 14 GB. |

Kịch bản `auth` cũng đạt trên scratch riêng. Hành trình Đã lưu giữ focus và cùng hàng cho tới ACK thực, hiển thị MicroArc ở nút bỏ lưu, gửi đúng một DELETE và chuyển focus khi hàng cuối biến mất. Clippy SSR của package `vink-web` đạt với `-D warnings`.

Bổ sung hai lỗi đầu vào của Product SPA bằng fixture loopback. 404 hiện một alert và một link `/`. 503 hiện một alert và một nút retry. Hai lần kích hoạt trong khi GET đang treo tạo một GET, sau đó trang có kết quả ở `/?q=kiemthu`. [Chi tiết 404 và 503](recovery.json).
