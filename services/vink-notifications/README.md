# Vink Notification Service

Tiến trình riêng của Notifications v1 ([#347](https://github.com/loveoverflowcom/vink/issues/347),
Epic [#305](https://github.com/loveoverflowcom/vink/issues/305) R3). Service sở hữu receipt nhận sự
kiện, gom nhóm inbox khi ghi, watermark `Seen`, dấu `Read` và retention trong database PostgreSQL
riêng `vink_notifications` trên cùng instance với App và Identity (`K1`, `K2`). Đặc tả miền ở
[notifications.md](../../docs/architecture/notifications.md); hợp đồng envelope/receipt ở
[contract HTTP v1 của App](../vink-app/contracts/http/v1/notifications.md); hợp đồng socket của
service ở [contracts/rpc/v1](contracts/rpc/v1/README.md); hiện trạng và bằng chứng ở
[docs/development/notifications.md §7](../../docs/development/notifications.md#7-hiện-trạng-347-notification-service).

| Thuộc Notification Service | Không thuộc service |
| --- | --- |
| `ingestion_receipts`, nhóm inbox và membership, dấu Thấm `P-02B`, fence bình luận, `Seen`/`Read`, purge 180 ngày | Chọn người nhận và quyền lúc phát (App, `#346`); kiểm quyền hiển thị, hydrate danh tính/nội dung (Gateway App, `#348`); danh sách người Thấm công khai (Endorsement của App) |

Service không đọc database `vink` hay `vink_identity`, không có cổng TCP, không lưu nội dung, tên
hiển thị, email hay danh tính người kiểm duyệt. Hai crate không import nhau (`verify quick` kiểm
manifest và SQL).

## Topology và caller (`T-07 / CHOICE-21 = A`)

Hai Unix socket, mỗi socket một phạm vi và đúng một OS user được gọi (kiểm bằng `SO_PEERCRED`):

| Socket | Biến | Caller | Route |
| --- | --- | --- | --- |
| ingest | `VINK_NOTIFICATIONS_INGEST_SOCKET` + `VINK_NOTIFICATIONS_RELAY_UID` | Relay trong `vink-worker` | `POST /v1/ingest` |
| inbox | `VINK_NOTIFICATIONS_INBOX_SOCKET` + `VINK_NOTIFICATIONS_GATEWAY_UID` | Gateway trong `vink-app` (`#348`) | `/v1/inboxes/{recipient}/…` |

Route của phạm vi này không tồn tại trên socket kia. Khi production chạy relay và Gateway cùng một
UID, phạm vi được tách bằng việc chỉ mount socket ingest vào container worker và socket inbox vào
container app. Header, body hay địa chỉ không bao giờ là bằng chứng caller. Phía App kiểm ngược
UID của tiến trình phục vụ socket (`VINK_NOTIFICATIONS_SERVER_UID`) trước khi gửi.

## Lệnh

`vink-notifications [serve|migrate|maintenance purge|maintenance status|maintenance recovery begin|end <ghi chú>]`; mã thoát 0 thành công,
1 lỗi runtime, 2 cấu hình hoặc cú pháp sai.

| Lệnh | Credential | Việc |
| --- | --- | --- |
| `serve` (mặc định) | `VINK_NOTIFICATIONS_DATABASE_URL` (runtime) | Bind hai socket, purge mỗi giờ; không bao giờ migrate |
| `migrate` | `VINK_NOTIFICATIONS_MIGRATE_DATABASE_URL` (migrator) | Áp migrations; không dùng runtime làm dự phòng |
| `maintenance purge` | runtime | Xóa hàng hết hạn theo lô |
| `maintenance status` | runtime | Đếm inbox, nhóm, receipt, dấu Thấm, tuổi nhóm cũ nhất và trạng thái cổng recovery (không in ID) |
| `maintenance recovery begin\|end <ghi chú>` | runtime | Đóng/mở cổng recovery: inbox trả `503 recovering` trong lúc đối soát sau restore; có audit ([runbook](../../docs/development/notifications.md#73-retention-sự-kiện-muộn-và-phục-hồi)) |

Readiness (`/health/ready` trên mỗi socket) chỉ `200` khi database trả lời, schema hiện hành, role
không có quyền đặc biệt và socket đó có caller.

| File | Vai trò |
| --- | --- |
| [src/envelope.rs](src/envelope.rs) | Parse envelope nghiêm ngặt, digest chuẩn hóa, luật theo từng loại; từ chối `admin.notice.reserved` (`P-05`, `TYPE_ONLY_NOW`) |
| [src/rollup.rs](src/rollup.rs) | Luật thuần: `group_key`, preview 3 người, bước Thấm `P-02B`/`P-03A`, Read theo phiên bản, snapshot, các hạn công bố |
| [src/store.rs](src/store.rs) | Transaction receipt + hiệu ứng, lệnh `Seen`/`Read`/`MarkAllRead` idempotent, đọc trang/badge, purge và status |
| [src/http.rs](src/http.rs) | Hai router, xác thực caller theo peer UID, deadline 5 s, readiness |
| [src/server.rs](src/server.rs) | Socket `0660` trong thư mục đóng, thay socket cũ an toàn, shutdown xóa đúng socket của mình |
| [migrations/](migrations/) | Schema `vink_notifications` |
| [migrations-rollback/](migrations-rollback/) | Backout, từ chối khi còn nhóm trong hạn |
| [tests/postgres.rs](tests/postgres.rs) | PostgreSQL thật: idempotency, rollup, đua ghi, Thấm, invalidation, Seen/Read, retention, restore, cổng recovery; phép đo ngân sách `#[ignore]` ở `tests/postgres/budget.rs` |
