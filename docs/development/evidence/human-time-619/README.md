# Reader đã lưu · ngày giờ #619

Chrome desktop, Light, 1440×900 CSS px, text 100%, device scale 1. Hai lần dùng cùng DB scratch, cùng tác phẩm mẫu `33333333-3333-4333-8333-333333333333` và cùng principal fixture qua loopback. Nút Lưu đã nhấn. Máy giả lập `Asia/Ho_Chi_Minh` (Chrome trả alias `Asia/Saigon`). Đồng hồ được ghim tại `2026-11-29T00:00:00Z`. API cung cấp `2026-11-28T07:02:15.977193Z` cho cả hai lần.

| Ảnh đã mở | Source và bundle hydrate dev | Nhãn |
| --- | --- | --- |
| [Trước](before.png) | Product `fa91d50e`, package `9bf0c2e2c22b` | 14:02 GMT+7 28/11/2026 |
| [Sau](after.png) | Product `5e134cfa`, package `34e0b9b158e9` | Hôm qua |

Pha `reader-saved-time` đi tới Reader thật, kiểm membership, nhấn Lưu nếu cần, tải lại rồi chụp. Hai ảnh dùng cùng hàm capture. Vùng chạm giờ sau sửa cao ít nhất 44 CSS px. [Chính sách và bảng consumer](../../../design/human-time.md). [Ảnh lịch sử c259](https://drive.google.com/file/d/1xxfle2hFdJ_-nHaeEvt8b4-6liZKu6W7/view).

C259 ghi nhận hiện tượng ở fixture tổng hợp trên SHA `91191186c1ac5ec6f4032dc0292af025eb2c0458`. Manifest lịch sử không ghi override múi giờ cho các hàng này. Hai lần chụp mới ở trên ghim riêng cả đồng hồ và múi giờ. [Manifest nguồn](https://drive.google.com/file/d/1aSRfwKgtDpwg8leGGswwpEBOT0_O6H7J/view).

## Danh sách và kiểm duyệt

Chrome Light, 1440×900, chữ 100%, cùng đồng hồ `2026-11-28T08:00:00Z` và `America/Los_Angeles`. Pha `time-surfaces` mở các route thật. Build trước `fa91d50e` dùng gói `9bf0c2e2c22b`, build sau `5e134cfa` dùng gói `34e0b9b158e9`. CDP cung cấp cùng hai hàng Bàn viết và ba thông báo qua các điểm chặn của suite hiện có. Chi tiết dùng fixture quyết định gỡ nhận xét của API. Mỗi cặp dùng một driver và cùng dữ liệu.

| Vị trí | Trước | Sau | Dấu hiệu đã đối chiếu |
| --- | --- | --- | --- |
| Bàn viết | [Trước](desk-before.png) | [Sau](desk-after.png) | Hai hàng dùng “Hôm qua” và “3 phút trước”. Nút xem giờ đầy đủ nằm sau danh sách. |
| Inbox | [Trước](inbox-before.png) | [Sau](inbox-after.png) | Qua nửa đêm LA, “Hôm nay” đổi thành “7 ngày qua”. Giờ bản gửi vẫn riêng với giờ sự kiện, hàng cũ hiện “1/10”. |
| Chi tiết | [Trước](decision-before.png) | [Sau](decision-after.png) | Ngày quyết định hiện “18:12, 19/11/2026”. Nút thời gian mở đủ giây và múi giờ. |

## Kiểm chứng local

Chạy trên Mac mini, checkout riêng của `loveoverflowcom/vink`, PostgreSQL scratch và Chrome headless. Build dev. Mỗi lần chạy một workload. Gói hydrate cuối `34e0b9b158e9`, gói CSR `8e3a3430fa7a`, cùng source Product tại `5e134cfa`. Lần chạy Discussion sau sửa oracle ở `5473711c` có cùng code Product. Các lane tích hợp trước đó được giữ ở bundle đã chạy.

| AC | Bằng chứng |
| --- | --- |
| 01–03, 09–10 | [Bảng nguồn → trường → chính sách](../../../design/human-time.md), rà lại Rust và JS. Discovery, Password (Following, hồ sơ), Standalone (Đã lưu) và Desk chạy qua route thật. Với cùng membership lúc `2026-11-27T18:30:00Z`, giờ ghim `2026-11-28T08:00:00Z`, Saved cho “13 giờ trước” ở HCM và “Hôm qua” ở LA. Unit báo cáo Lượt xem cho 28/11 theo HCM. |
| 04, 07, 11 | Unit Patterns kiểm ngưỡng giây, phút, giờ, ngày 6/7, năm nhuận, giao năm và DST. Pha `timestamp` kiểm HCM, Los Angeles, Tokyo, New York, Kiritimati, giờ lịch sử Paris, đổi zone khi hiện lại tab, timestamp tương lai, ngày sai, thiếu offset và Intl lỗi. |
| 05–06 | Reader và Desk kiểm Enter, Space, Escape, cảm ứng 320 px/chữ 200%, đích focus và đường xem instant đầy đủ. Inbox kiểm cây accessibility, giữ link được focus khi sang ngày và đếm request Seen/Read. Editor thật giữ nguyên chữ, version, focus và vị trí cuộn qua nhịp đồng hồ. |
| 08 | HTML SSR và Chrome tắt JS hiện UTC. Observer bắt node UTC trước WASM và xác nhận hydrate nhận lại chính node đó. Reader đã lưu sau hydrate và trên server static CSR trả cùng nhãn, instant, giờ ghim và múi giờ. Cấu hình Web featureless biên dịch đạt. |
| 12–13 | Đã mở các PNG trong hai bảng. LayoutSpec liên kết chính sách và ghi riêng DQ-05. Đối chiếu theo PR #642. |

Lệnh kiểm gồm `cargo test --locked -p vink-patterns --lib` (24 đạt), `cargo test --locked -p vink-web --features ssr --lib` (333 đạt, một benchmark ignored), Clippy native và WASM hydrate với `-D warnings`, cùng `cargo xtask verify static docs`. Các scenario của `cargo xtask verify web` chạy với `--capture checks` gồm desk (gồm guest), review, notifications, standalone, password, discovery và discussion. Lượt Discussion đầu báo lỗi vì assertion tìm `aria-pressed` trên cả HTML khi nút giờ đã có thuộc tính này. Oracle sau sửa kiểm chính nút Thấm, gồm cả trạng thái unknown và public count bằng 0. Lượt Discussion kế tiếp đạt.

Phạm vi trợ năng là cây AX của Chrome và thao tác qua CDP. VoiceOver/TalkBack trên thiết bị thật và môi trường production chưa được kiểm.
