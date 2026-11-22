# Notification Service — hợp đồng socket riêng tư v1 (draft, `#347`)

Hợp đồng giữa App (relay trong `vink-worker`, Gateway trong `vink-app`) và `vink-notifications`.
Ngữ nghĩa envelope, digest, receipt và ACK có fence do
[contract HTTP v1 của App §1](../../../../vink-app/contracts/http/v1/notifications.md#1-ngữ-nghĩa-liên-dịch-vụ-346-apprelay--347-notification-service)
sở hữu; tài liệu này chỉ định khung truyền, route, lỗi và các hạn service công bố. Mức kiến trúc:
`DESIGN_PROPOSED` (lựa chọn kỹ thuật của `#347`, không làm `VDR-NOTIFICATIONS-CONTRACT@r1` thành
`ACCEPTED`).

## 1. Truyền tải (`T-07 / CHOICE-21 = A`)

- HTTP/1.1 trên Unix socket; không có TCP. Hai socket: **ingest** (relay) và **inbox** (Gateway).
- Service chỉ trả lời kết nối có OS peer user bằng caller cấu hình cho socket đó; sai hoặc thiếu
  caller → `401 caller_unauthenticated` trước khi đọc body. Route của socket kia → `404 unknown_route`.
- Client kiểm UID tiến trình phục vụ socket trước khi gửi; sai UID thì không gửi gì.
- Deadline mỗi lời gọi ở service: 5 s (relay bao lời gọi trong `send_timeout` 10 s, ngắn hơn lease
  30 s). Body tối đa 1 MiB. Mọi phản hồi có `Cache-Control: no-store`.
- Lỗi luôn có dạng `{"error":{"code":"…","message":"…"}}`, message cố định, không lặp lại input.

## 2. Ingest (socket ingest)

`POST /v1/ingest`

```json
{"events": [{"event_id": "019a9b10-0001-7000-8000-000000000101", "envelope": "<envelope wire, nguyên byte>"}]}
```

- Tối đa 100 sự kiện (`413 batch_too_large`). `envelope` là chuỗi JSON đã chuẩn hóa kèm
  `payload_digest`, đúng như App lưu và gửi; service parse từ chính chuỗi đó (từ chối khóa lặp,
  field lạ, UUID không chuẩn, recipient chưa sắp hoặc trùng).
- Mỗi sự kiện chạy trong transaction riêng, theo thứ tự. Phản hồi `200` có đúng một kết quả cho
  mỗi sự kiện, cùng thứ tự:

```json
{"results": [{"event_id": "019a9b10-0001-7000-8000-000000000101", "status": "applied"}]}
```

| `status` | Khi nào | Relay làm gì (`#346`) |
| --- | --- | --- |
| `applied` | Receipt mới và hiệu ứng (tín hiệu, đối soát, hoặc không gì với sự kiện cũ/muộn/invalidation không lưu) đã commit cùng nhau | ACK có fence |
| `already_applied` | Đã có receipt cùng `event_id` và cùng digest | ACK có fence |
| `payload_mismatch` | Đã có receipt cùng `event_id`, digest khác; không ghi gì | Dead letter chờ vận hành |
| `rejected_invalid` | Digest sai, schema/type/shape sai, `event_id` khác với envelope, `admin.notice.reserved` | Dead letter |
| `transient_unavailable` | Lỗi database; transaction đã rollback | Retry có backoff |

Body batch sai khung → `400 invalid_request` (không có kết quả nào; relay retry cả lô).

## 3. Inbox (socket inbox)

`{recipient}` là UUID chữ thường mà Gateway đã giải từ phiên. Service không thể tự kiểm phiên; chỉ
Gateway đã xác thực được chọn `recipient` (`K5`).

| Route | Body | Phản hồi `200` |
| --- | --- | --- |
| `GET /v1/inboxes/{recipient}/count` | — | `{"status":"fresh","unseen_group_count":n,"as_of_snapshot":"s42","max_staleness_seconds":60}` |
| `GET /v1/inboxes/{recipient}/groups?limit=1..50&cursor=…` | — | `{"inbox_snapshot":"s42","seen_snapshot":"s40","groups":[…],"next_cursor":"c41.<uuid>"\|null}` |
| `POST /v1/inboxes/{recipient}/seen` | `{"command_key","up_to_snapshot"}` | `{"seen_snapshot":"s42"}` |
| `POST /v1/inboxes/{recipient}/read` | `{"command_key","group_id","observed_group_version"}` | `{"group_id","group_version","unread"}` |
| `POST /v1/inboxes/{recipient}/mark-all-read` | `{"command_key","up_to_snapshot"}` | `{"up_to_snapshot","groups_marked"}` |

Mỗi nhóm trong `groups`: `group_id`, `event_type`, `target` (đúng object `target` của envelope mới
nhất, §2.4 của contract App), `group_version`, `signal_version`, `unread`, `unseen`, `actor_count`,
`item_count`, `actors_preview` (≤ 3 `PrincipalId`, mới nhất trước), `action_required`,
`publication_epoch` (mốc COM-11 để Gateway so với epoch hiện hành), `created_at`, `last_signal_at`,
`expires_at`. Nhóm hết hạn hoặc về 0 không bao giờ được trả.

Từ `#348`, nhóm `comment.root.approved`/`comment.reply.approved` có thêm `contributions`:
`{"newest": [{"comment_id","actor_id"}…], "total": n}`, tối đa 16 đóng góp còn hiệu lực, mới
nhất trước, đọc trong cùng snapshot của trang. Gateway dùng nó để chọn bình luận hợp lệ mới nhất
khi bình luận mới nhất mất quyền trước lúc invalidation tới, và tính lại preview/số người. Loại
khác trả `contributions: null`. Service vẫn không lưu thân bình luận (Thin Pointer).

- **Snapshot/version (`T-05`)**: token `s<n>` là giá trị dãy số của hộp thư. Mọi thay đổi của một hộp
  thư lấy số kế tiếp khi giữ khóa dòng hộp thư, nên thứ tự số là thứ tự commit. Token lớn hơn số đã
  commit → `422 invalid_snapshot`.
- **Seen**: `seen = max(seen, up_to)`; ACK cũ là no-op thành công; không đổi `unread`.
- **Read**: đánh dấu đã đọc mọi tín hiệu tới `observed_group_version`; tín hiệu mới hơn vẫn chưa đọc,
  thay đổi âm thầm (preview, membership) không giữ nhóm chưa đọc. Phiên bản chưa từng có →
  `422 invalid_group_version`; nhóm không thuộc hộp thư hoặc đã hết hạn → `404 not_found`.
- **MarkAllRead**: chỉ các nhóm đã có tại snapshot, chỉ tín hiệu tới snapshot.
- **Idempotency**: cùng `recipient` + `command_key` (1–128 ký tự ASCII in được) + cùng lệnh/tham số
  trả lại đúng câu trả lời đã lưu trong 7 ngày; khác lệnh/tham số → `422 idempotency_key_reused`.
- Lỗi database → `503 dependency_unavailable`; không bao giờ trả `0` hay danh sách rỗng thay lỗi
  (`P-04B`). Hạn trễ của badge mà Gateway được phép phục vụ: `max_staleness_seconds = 60`.
- **Cổng recovery:** khi vận hành đang đối soát sau restore (`maintenance recovery begin`), mọi route
  inbox trả `503 recovering`; Gateway coi như nguồn không khả dụng (`stale` nếu còn mốc trong hạn,
  không thì `503`), không bao giờ thành `0`. Ingest không bị chặn. `/health/ready` báo
  `checks.recovery = "serving" | "recovering" | "unknown"` nhưng không vì thế mà not ready.

## 4. Hạn công bố

| Hạn | Giá trị | Ý nghĩa |
| --- | --- | --- |
| Tuổi nhóm (`P-06A`) | 180 ngày từ lúc tạo | Không gia hạn; nhóm do replay tạo được tính tuổi từ `occurred_at` của sự kiện gốc |
| `late_event_cutoff` (`P-13B`) | 30 ngày | Sự kiện xảy ra lâu hơn: có receipt, đối soát dấu/membership, không tạo hay hồi sinh nhóm |
| Receipt và fence bình luận | 45 ngày | Dài hơn replay horizon 30 ngày của App và `late_event_cutoff` |
| Dấu Thấm (`P-02B`) | 400 ngày từ lần đổi cuối | Phòng thủ thêm cho thứ tự/replay; bảo đảm suốt đời đến từ fact `first_selection` của App |
| Câu trả lời lệnh ACK | 7 ngày | Cửa sổ idempotency của `command_key` |
| `max_staleness_seconds` (`P-04B`) | 60 | Hạn trễ count Gateway được phục vụ |
| Recovery horizon | 30 ngày | Bằng replay horizon của App; runbook và RPO theo loại sự cố ở [§7.3](../../../../../docs/development/notifications.md#73-retention-sự-kiện-muộn-và-phục-hồi) |
