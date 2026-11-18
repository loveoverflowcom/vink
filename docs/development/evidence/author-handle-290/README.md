# Nghiệm thu địa chỉ tác giả #290

Mã đã chạy ở `8b2d98be8159efaee40077e7847c963fd19287e6`, từ `develop@3ec0a05137180a65ed113e114e627361efaaa539`. Contract r1 của #288 ở `12790d821d565260903727284233b3282943a259`, LayoutSpec r2 của #289 ở `9fcd75ec03229a27464123eac1d68104e8447b19`. [Issue #290 và preview Handle r2 đã duyệt](https://github.com/loveoverflowcom/vink/issues/290) ghi revision của HTML và ảnh.

## Luật và giao dịch

| AC | Bằng chứng |
| --- | --- |
| HANDLE-01 | Cùng subject provision đồng thời chỉ có một hồ sơ. Bộ sinh nhận bút danh và số ngẫu nhiên, kiểm ASCII, tên dành riêng và dấu nối. PostgreSQL xác minh giới hạn 16 lần thử, phục hồi sau va chạm, hai tài khoản cùng chọn một tên và backfill chạy lại. |
| HANDLE-02 | Thiết lập bằng tên gợi ý hoặc tên tự nhập đều còn 2 lượt. Kiểm hai giao dịch cùng phiên bản, collision, no-op, phát lại receipt, lỗi trước khi ghi receipt, hết lượt, quyền theo PrincipalId và đổi bút danh. Chrome làm mất phản hồi đã commit rồi tải lại để đối soát. |
| HANDLE-03 | HTML SSR trả bút danh và trạng thái tác giả chưa có tác phẩm. GET và HEAD qua hai lượt đổi đều trả 301 trực tiếp với Location nội bộ và no-store. Chrome giữ fragment, bỏ query. Sau khi thu hồi hồ sơ, Back và các URL đã thăm đều thấy trạng thái đã xóa. HTTP trả 410, các tên vẫn được giữ. |
| HANDLE-04 | Phiên chủ thật trong Chrome đi từ Tôi đến thiết lập, đổi, xác nhận lượt cuối và hết lượt. Khách thấy hồ sơ công khai, không thấy điều khiển của chủ. Test PostgreSQL xác minh principal khác không thể dùng lệnh hoặc đọc receipt của chủ. |

CLI chạy trên database riêng với một binding hợp lệ, một binding có bút danh chỉ chứa khoảng trắng và hai principal seed thiếu binding. Lượt đầu báo `created=1, attempts=1, existing=0, invalid=1, unbound=2`. Lượt hai báo `created=0, attempts=0, existing=1, invalid=1, unbound=2`. Kết quả cuối có một hồ sơ và một tên trong registry.

## Web và accessibility

| Chiều | Kết quả | Phạm vi |
| --- | --- | --- |
| Functional | PASS | Tài khoản native, hai tab chủ, một phiên khách độc lập, App và Identity nối PostgreSQL scratch. |
| Geometry | PASS | 320, 390, 834, 1024, 1440 px × sáng/tối × chữ gốc 100/200%. Toàn bộ 20 trường hợp giữ control trong chiều rộng tài liệu. Nút chính cao ít nhất 48 px. |
| Visual | PASS, tự review | Đối chiếu bốn ảnh Chrome bên dưới với preview r2. Desktop giữ form và bản xem trước cạnh nhau. Mobile tách địa chỉ cũ/mới và cảnh báo lượt cuối. Chữ 200% đọc theo chiều dọc. |
| Accessibility/Responsive | PASS trong Chrome | Cây accessibility nhận ô “Tên trong liên kết”. Tương phản đo trên ô nhập, trợ giúp, nút chính và dòng thông tin hồ sơ đạt tối thiểu 6,33:1 ở giao diện sáng và 8,16:1 ở giao diện tối. |
| Interaction | PASS trong Chrome | Tab đến hành động chính. Lỗi giữ chữ và đưa focus về ô. Escape trả focus về nút mở. Composition chưa kết thúc chặn gửi. Lỗi phiên cũ giữ chữ. |

[Thiết lập desktop](handle-setup-1440-light-100.png) · [Màn hẹp, tối, chữ 200%](handle-setup-320-dark-200.png) · [Xác nhận lượt đổi cuối](handle-final-confirm-390.png) · [Trang tác giả khi xem bằng phiên khách](handle-public-guest.png) · [Số đo, font, viewport và SHA-256 từng ảnh](measurements.json)

Chrome headless, ma trận dùng `font-size` trên phần tử gốc và sự kiện composition trong trình duyệt. Chưa chạy IME hệ điều hành, VoiceOver/TalkBack, thiết bị chạm vật lý hay bộ Web toàn phần. Test vòng đời dùng seam thu hồi App trên scratch. Điều phối xóa tài khoản xuyên Identity/App thuộc #297.

## Lệnh đối chiếu

```sh
./tools/local-budget run -- cargo xtask verify quick
./tools/local-budget run -- cargo xtask verify postgres
cargo xtask verify web --scenario password
vink-app principal backfill-handles
```

Ba bộ test PostgreSQL ghi nhận 6 test store, 233 test App và 48 test Identity đạt. Bộ App có 7 probe opt-in được bỏ qua. Cả bốn test PostgreSQL riêng của #290 đã chạy.

Chạy tuần tự trên macOS. Lane Web dùng Chrome cài trên máy. CLI chạy hai lần sau migration và seed của database scratch riêng. Bộ test theo owner nằm trong [PostgreSQL/HTTP](../../../../services/vink-app/tests/postgres/handle.rs), [provision](../../../../services/vink-app/tests/postgres/principal.rs) và [kịch bản Chrome](../../../../apps/vink-web/e2e/author-handle-smoke.mjs).
