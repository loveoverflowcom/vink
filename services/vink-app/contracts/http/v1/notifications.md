# Notifications v1 & Danh sách Thấm công khai — Hợp đồng đề xuất (ngoài Web)

**Trạng thái:** Hợp đồng đề xuất r1 · `NOT_IMPLEMENTED` (chưa có route Axum, service hay fixture
kiểm tự động trong `contract.rs`).
Tài liệu này chốt ngữ nghĩa chung cho **tầng liên dịch vụ** (`#346` App/Relay ↔ `#347` Notification
Service) và **tầng Gateway ↔ Client** (`#347` ↔ `#348`, dùng lại được cho Web và CMP/mobile theo
`P-14A`), **không** kê đơn SQL DDL và **không** chọn bố cục/CSS.

- **Yêu cầu sản phẩm:** [PRD §16](../../../../../prd/requirements.md#16-thông-báo-cộng-đồng-và-danh-sách-thấm-công-khai-notifications-v1) (`NOTIF-01..10`)
- **Đặc tả miền, Event Inventory & UX bằng chữ:** [docs/architecture/notifications.md](../../../../../docs/architecture/notifications.md)
- **Bảng truy vết & điều kiện bàn giao:** [docs/development/notifications.md](../../../../../docs/development/notifications.md)

> Mọi JSON minh họa dưới đây là **ví dụ hợp đồng độc lập Leptos/DOM/HTML** dùng chung giữa
> `#346`–`#348` và làm đầu vào dựng fixture/client; không phải fixture đang chạy trên `develop`.

---

## 1. Ngữ nghĩa liên dịch vụ (`#346` App/Relay ↔ `#347` Notification Service)

### 1.1. Event Envelope & Chính sách tối thiểu dữ liệu

App xác định `event_type`, `recipient(s)`, quyền lúc phát và thứ tự nguồn **bên trong giao dịch
nghiệp vụ PostgreSQL đã khóa**. Relay không đọc thêm trạng thái nghiệp vụ để tự đoán người nhận;
Notification Service không đọc trực tiếp App DB hay Identity DB (`K4`).

| Trường envelope | Kiểu & Ngữ nghĩa bắt buộc | Quy tắc bất biến |
| --- | --- | --- |
| `event_id` | UUID duy nhất của lần phát nguồn | Khóa chống trùng chính ở `ingestion_receipt`. Cùng `event_id` + cùng nội dung chỉ có một hiệu ứng. |
| `event_type` | Enum đóng (`comment.root.approved`, `comment.reply.approved`, `work.moderation.decided`, `comment.moderation.adverse`, `work.endorsement.tham.changed`, `admin.notice.reserved`, `notification.invalidation`) | Trong v1: 5 nhóm nghiệp vụ + invalidation plumbing chạy thật; `admin.notice.reserved` chỉ dành trước (`P-05`, `TYPE_ONLY_NOW`); **Hữu ích** và **Follow** không có trong v1. |
| `schema_version` | Số nguyên dương (`1`) | Cho phép nâng cấp schema an toàn mà không làm hỏng delivery cũ đang đợi retry. |
| `source_owner` | `"discussion"` \| `"publication_moderation"` \| `"endorsement"` \| `"principal"` | Phục vụ truy vết và tách luồng nếu `#346` chọn `T-01 B`. |
| `activation_epoch` | Mốc/epoch kích hoạt tính năng gắn tại commit nguồn | Ghi theo **thời điểm commit giao dịch nguồn**, không lấy giờ Relay. Sự kiện có mốc trước activation bị bỏ qua; không backfill lịch sử. |
| `source_order` / `source_version` | Phiên bản tăng đơn điệu tại owner nguồn (`moderation_version`, `decision_version`, `accepted_order`, `public_epoch`) | Dùng để xử lý sự kiện đến đảo thứ tự (ví dụ `Add`/`Remove` Thấm hoặc vô hiệu hóa sau khi duyệt) mà không suy từ `now()` hay UUID. |
| `occurred_at` | RFC 3339 UTC do PostgreSQL nguồn ghi khi commit | Chỉ để hiển thị/tính hạn trễ; không dùng làm bằng chứng thứ tự commit thay `source_order`. |
| `actor_id` | `PrincipalId` (UUID) hoặc `null` với quyết định hệ thống/kiểm duyệt | Không gửi tên hiển thị, email hay tên moderator; Gateway hydrate tên/avatar công khai tại lúc đọc. |
| `recipients` | Mảng không rỗng `PrincipalId` đã loại self và dedup tại nguồn (trừ invalidation toàn cục theo target) | Tối đa đã rút gọn theo `P-01B` (`reply` gửi tối đa `{direct_target_author, root_author} \ {actor}`). |
| `target` | Object định danh có kiểu (xem §2.4) | Chỉ mang `work_id`, `revision_id`, `root_id`, `reply_target_id`, `comment_id`, `decision_version`, `moderation_version`. |
| `semantic_kind` | Chỉ dấu ngữ nghĩa cho gom nhóm & quyết định (`approved` \| `rejected` \| `removed` \| `add` \| `remove` \| `invalidate`) | Với Thấm: chỉ phát khi `EndorsementDelta` là `Add` hoặc `Remove`; không phát `EndorsementDelta::None`. Với tác phẩm: phân biệt `approved` với `rejected` (`P-07A`, `P-11b`). |
| `action_required` | `bool` | Chỉ `true` khi chủ nội dung thực sự cần hành động (`P-11a`); `NeedsAdmin` / retry exhausted không phát event cho tác giả. |
| `payload_digest` | SHA-256 chuẩn hóa của các trường nghiệp vụ bất biến trong event | Dùng để phát hiện `payload_mismatch` khi nhận lại cùng `event_id` với nội dung khác. |

**Cấm tuyệt đối trong envelope, receipt và log vận hành:** thân bài viết, thân bình luận, địa chỉ
email, mật khẩu/token phiên, danh tính cá nhân người kiểm duyệt và ghi chú kiểm duyệt nội bộ (`MOD-12`).

### 1.2. Activation Boundary, Cutoff & Late Delivery

- **Ghim activation tại commit nguồn (`T-01`):** Giao dịch App chỉ enqueue event khi giao dịch đó
  chạy dưới mốc activation đã bật (cấu hình/epoch commit). Event sinh trước activation hoặc trước
  cutoff phục hồi bị loại có chủ đích; **không backfill** từ dữ liệu lịch sử cũ trước khi bật
  Notifications.
- **Sự kiện đến muộn (`P-13B`, `T-08`):** Notification Service đặt ngưỡng tuổi tối đa cho sự kiện đến
  muộn so với chính sách 180 ngày (`N8`). Sự kiện đã trễ quá tuổi nhóm hoặc quá retention cutoff không
  được phép tạo hoặc hồi sinh một nhóm inbox hết hạn; tuy nhiên, đối với `work.endorsement.tham.changed`,
  dấu chống báo lại `P-02B` vẫn phải được đối soát đúng mà không làm hiện thông báo cũ.

### 1.3. Delivery Receipt, Payload Mismatch & Fenced ACK (`#346` ↔ `#347`)

Giao tiếp qua hai database không dùng transaction phân tán; chu kỳ giao tuân chặt:

1. **Claim ngắn (`#346`):** Relay khóa bản ghi delivery ở App DB, tăng `claim_generation`, gán lease
   hữu hạn và `COMMIT` trước khi mở I/O mạng.
2. **Giao ngoài transaction App:** Relay gửi batch/event qua kênh xác thực nội bộ (`T-07`) tới
   Notification Service kèm `event_id` và `payload_digest`.
3. **Receipt + Effect nguyên tử tại Notification DB (`#347`):** Notification Service chạy một
   transaction cục bộ kiểm tra `ingestion_receipt(event_id)`:
   - Chưa có `event_id`: ghi `ingestion_receipt(event_id, payload_digest, …)` + áp dụng hiệu ứng
     rollup/invalidation (`group_key`, `actor_count`, `actors_preview`, `P-02B` suppression,
     `signal_version`) trong **cùng transaction**, trả `status = "applied"`.
   - Đã có `event_id` và **khớp** `payload_digest`: không áp dụng hiệu ứng lần hai, trả
     `status = "already_applied"`.
   - Đã có `event_id` nhưng **lệch** `payload_digest`: từ chối ghi, không coi là replay hợp lệ, trả
     `status = "payload_mismatch"` để Relay đưa vào trạng thái lỗi cần đối soát/cảnh báo (dead-letter /
     quarantine).
   - Payload sai schema/không hợp lệ: trả `status = "rejected_invalid"` (không retry vô hạn).
   - Quá tải/DB tạm lỗi: trả `status = "transient_unavailable"` để Relay backoff/retry có giới hạn.
4. **Fenced ACK (`#346`):** Nhận được `applied` hoặc `already_applied`, Relay khóa bản ghi delivery tại
   App DB và chỉ đánh dấu hoàn tất khi `claim_generation` vẫn khớp lease đang giữ. Nếu mất ACK hoặc
   lease hết hạn trước đó, lần retry của worker mới gửi lại cùng `(event_id, payload_digest)` sẽ
   nhận `already_applied` an toàn.
5. **Restore lệch mốc (`T-08`, Epic AC-15):** Nếu Notification DB phải restore về thời điểm cũ hơn mốc
   App đã ACK trong giới hạn recovery horizon công bố, kênh vận hành/replay của `#346` + `#347` cho
   phép phát lại các sự kiện trong cửa sổ phục hồi (kể cả đã ACK ở App) nhưng **không** hồi sinh các
   nhóm đã vượt quá trần 180 ngày (`P-13B`).

---

## 2. Ngữ nghĩa Gateway ↔ Client (`#347` ↔ `#348`, tái sử dụng CMP/mobile `P-14A`)

### 2.1. Hai ranh giới xác thực & Bộ nhớ đệm tách biệt

Client (Web, CMP/mobile hoặc HTTP client thử nghiệm) **chỉ giao tiếp với App Gateway**; không có
endpoint nào nối trực tiếp tới Notification Service (`K5`).

| Bề mặt đọc/ghi | Chủ sở hữu dữ liệu | Yêu cầu phiên / Quyền | Chính sách `Cache-Control` |
| --- | --- | --- | --- |
| Badge count, trang Inbox, ACK `Seen`, `Read`, `MarkAllRead`, Chi tiết quyết định riêng | Notification Service (dữ liệu nhóm/con trỏ) + App Gateway (authz/hydration) | Bắt buộc phiên thật (`__Host-vink_session` hoặc `Authorization: Bearer`). Server tự giải `PrincipalId`. Khách nhận `401`. Ghi đòi `Origin` hợp lệ (với cookie/browser) và `principal::require_active`. | `Cache-Control: no-store` (mọi phản hồi 2xx và lỗi). Không `ETag`, không dùng chung cache giữa các phiên. |
| Danh sách người Thấm công khai (`WorkId + RevisionId`) | App / Endorsement (`work_endorsements` + phép chiếu hồ sơ công khai) | Actor-free (không bắt đăng nhập); khách vẫn xem được khi tác phẩm có `publications` đúng `RevisionId` đó. Không có bản công khai hiện hành trả `404`. | `200`/`304`: `Cache-Control: private, no-cache` kèm strong `ETag` trên đúng byte phản hồi (kiểm visibility trước `If-None-Match`, giống `READ-01`). Lỗi `404`/`422`/`503`: `no-store`. |

### 2.2. `inbox_snapshot`, `group_version`, `signal_version` và ACK đơn điệu

Để bảo đảm các bất biến `N5`, `N6`, `P-09A`, `P-10A` qua nhiều tab, retry và mất ACK:

- **`inbox_snapshot`:** Chuỗi/token đại diện mốc nhất quán theo **thứ tự commit của Notification DB**
  (không dùng `now()`/`created_at`/UUID trước commit — `T-05`). Phản hồi `GET` trang đầu inbox trả về
  `inbox_snapshot` mới nhất đã quan sát.
- **`group_version` vs `signal_version`:**
  - `group_version` tăng mỗi khi nhóm có thay đổi quan sát được (thêm actor, bỏ Thấm giảm count,
    cập nhật preview, đánh dấu Read).
  - `signal_version` chỉ tăng khi có **tín hiệu nghiệp vụ mới thật sự** cần báo trên badge (không tăng
    khi đối soát âm thầm relike `P-02B` hoặc khi bỏ actor làm giảm count `P-03A`).
- **Ba lệnh ACK đều idempotent (`P-09A`, `P-10A`):**
  1. `Seen(up_to_snapshot, command_key)`: Tiến watermark `seen_snapshot = max(seen_snapshot, up_to_snapshot)`
     cho đúng `PrincipalId` của phiên. Không làm thay đổi trạng thái `is_read` của bất kỳ nhóm nào.
     ACK cũ mang snapshot nhỏ hơn `seen_snapshot` hiện tại là no-op thành công và không xóa tín hiệu mới.
  2. `Read(group_id, observed_group_version, command_key)`: Đánh dấu `is_read = true` cho phiên bản
     nhóm mà người dùng đã thấy, **kể cả khi việc mở trang đích tiếp theo bị lỗi hoặc mất quyền**. Nếu
     sau `observed_group_version` đã có tín hiệu mới tách sang nhóm `unread` mới (`N5`), nhóm cũ đã
     được Read ở phiên bản thấy được mà **không** tự đánh dấu Read nhóm `unread` mới vừa sinh.
  3. `MarkAllRead(up_to_snapshot, command_key)`: Chỉ đánh dấu đã đọc các nhóm unread của chủ phiên có
     cập nhật cuối nằm trong `<= up_to_snapshot`. Nhóm mới hoặc nhóm nhận tín hiệu sau `up_to_snapshot`
     vẫn giữ `is_read = false`.

### 2.3. Trạng thái Badge & Hạn hội tụ (`P-04B`)

Endpoint đếm nhẹ cho polling 30–60 giây trả về `freshness` rõ ràng để client không bao giờ nhầm lỗi
hoặc dữ liệu cũ với `0`:

- `status = "fresh"`: kèm `unseen_group_count` (số nguyên `>= 0`), `as_of_snapshot`, `max_staleness_seconds`.
- `status = "stale"`: khi nguồn count chưa thể xác minh kịp trong hạn `max_staleness_seconds` nhưng
  còn mốc hợp lệ gần nhất trong giới hạn chính sách; kèm `last_known_count`, `as_of_snapshot`, `reason`.
- Lỗi dịch vụ / dependency mất kết nối: HTTP `503 dependency_unavailable` (không trả `200` với `0`).

### 2.4. Typed Target & Chi tiết quyết định riêng (`P-12A`, `P-14A`)

Đích nghiệp vụ trong payload JSON là **dữ liệu có cấu trúc theo kiểu**, không ép Web route/HTML. Web
hoặc CMP tự ánh xạ sang route hoặc màn hình gốc của mình:

| `target.kind` | Trường định danh bắt buộc | Đích điều hướng nghiệp vụ (Web / CMP) |
| --- | --- | --- |
| `work_comment_thread` | `work_id`, `comment_id`, `moderation_version` | Mở tác phẩm tại bình luận gốc (nếu còn hợp lệ); nếu mất quyền, mở trạng thái ẩn/tombstone/không khả dụng. |
| `work_comment_reply` | `work_id`, `root_id`, `reply_target_id`, `comment_id`, `moderation_version` | Mở chuỗi phản hồi tại `comment_id` dưới `root_id` (giữ `reply_target_id` làm ngữ cảnh). |
| `work_endorsement` | `work_id`, `revision_id` | Mở tác phẩm tại phần Thấm (hoặc chỉ dẫn tới revision nếu bài đã cập nhật revision mới). |
| `work_moderation_decision` | `work_id`, `revision_id`, `decision_version` | Mở **màn chi tiết quyết định riêng** của chủ tác phẩm (`P-12A`) cho đúng `(work_id, revision_id, decision_version)`. |
| `comment_moderation_decision` | `work_id`, `comment_id`, `moderation_version` | Mở **màn chi tiết quyết định riêng** của người viết bình luận (`P-12A`) cho đúng `(comment_id, moderation_version)`. |
| `admin_notice_reserved` | `notice_id` | Dành trước (`P-05`, `TYPE_ONLY_NOW`); v1 không sinh item runtime. |

**Quy tắc màn chi tiết quyết định riêng (`P-12A`):**
- Chỉ chủ tác phẩm đọc được `work_moderation_decision`; chỉ tác giả bình luận đọc được
  `comment_moderation_decision`. Mọi actor khác (kể cả chủ tác phẩm đối với bình luận của người khác)
  nhận `404` không phân biệt.
- Luôn trả `outcome`, `action_required`, mã/lý do được phép hiển thị (`reason`), `decided_at`, và
  phiên bản (`revision_id`/`decision_version` hoặc `moderation_version`) của chính quyết định đó — kể
  cả khi tác phẩm đang riêng tư/bị bác hoặc bình luận đã bị gỡ.
- Khi bình luận đã bị gỡ bởi kiểm duyệt (`Removal::ByModeration`), trường `comment_excerpt` / `body`
  luôn là `null` (không phục hồi thân chữ đã xóa/gỡ). Không bao giờ trả tên/ID người duyệt hay ghi
  chú nội bộ.

### 2.5. Phân trang (`Cursor`) & Continuation hữu hạn (`T-06`)

- **Inbox (`T-06`):**
  - Danh sách sắp theo hoạt động mới nhất của nhóm còn hiệu lực; nhóm có thể đổi vị trí khi nhận
    rollup mới. Cursor phải ổn định qua rollup đổi vị trí (ràng buộc theo phiên + snapshot/tiêu chí
    sắp) và **không** được coi một trang candidate bị lọc quyền hoàn toàn là đã hết danh sách.
  - Phản hồi phân trang dùng trường `next_cursor` (khi còn candidate tiếp theo) và cờ
    `continuation_reason` (ví dụ `"more_items"` hoặc `"filtered_page_scan_bound"` nếu đã chạm ngân
    sách quét mà chưa gom đủ `limit` mục hợp lệ, cho phép client đọc tiếp bằng `next_cursor` thay vì
    nhầm là danh sách trống).
  - Bỏ qua toàn bộ item hết hiệu lực / mất quyền (`N4`); **tuyệt đối không** trả item mà Gateway
    không hydrate được quyền.
- **Danh sách người Thấm công khai (`P-08`):**
  - Tham số: `limit` hữu hạn (mặc định & trần do `#348` chốt, không trả không giới hạn) và `cursor`
    opaque gắn với `(work_id, revision_id, ordering_basis)` — ví dụ sắp theo `(endorsed_at DESC,
    actor_id ASC)` hoặc `(accepted_order DESC, actor_id ASC)` dựa trên bản ghi đang `selected = true`
    trong `work_endorsements`.
  - Mỗi lần tải trang kiểm tra lại `publications.revision_id == requested_revision_id` và áp phép
    chiếu danh tính công khai hiện hành (`linked` | `deleted` | `unavailable`). Nếu revision hiện hành
    đã đổi sang revision khác hoặc bài bị rút/ẩn giữa hai trang: từ chối tiếp tục (`409`
    `revision_changed` hoặc `404`), không trộn người Thấm của hai revision.

### 2.6. Bảng mã lỗi chuẩn hóa

| HTTP Status | Mã lỗi (`error.code`) | Khi nào trả & Bắt buộc với client |
| --- | --- | --- |
| `401` | `unauthenticated` | Thiếu hoặc hết hạn/thu hồi phiên cho badge, inbox, ACK hoặc chi tiết quyết định riêng. Không trả cho danh sách Thấm công khai khi bài đang công khai. |
| `403` | `forbidden` / `principal_suspended` / `invalid_origin` | Sai `Origin` ở lệnh ghi hoặc principal đang bị đình chỉ. |
| `404` | `not_found` | Tác phẩm/revision không công khai (với danh sách Thấm), hoặc quyết định/đối tượng không tồn tại hoặc không thuộc quyền đọc của người gọi. |
| `409` | `revision_changed` / `group_version_stale` / `work_unavailable` | Revision công khai vừa thay đổi khi đang phân trang Thấm, hoặc trạng thái nhóm xung đột ngoài quy tắc idempotent. |
| `422` | `validation_failed` / `idempotency_key_reused` | Sai cursor/limit/query, unknown field, hoặc tái sử dụng `command_key` với payload khác. |
| `503` | `dependency_unavailable` | Identity, App DB hoặc Notification Service không sẵn sàng / quá hạn. **Không bao giờ** thay bằng count `0` hay mảng rỗng. |

### 2.7. Hợp đồng hành vi client: Polling, Ẩn/Offline & Đổi tài khoản (bàn giao `#348` & Web)

Client Web (và CMP sau này) phải tuân các quy tắc điều phối sau trên nền API trên:
1. **Nhịp polling:** Khi người dùng đã đăng nhập và tab đang hiển thị + có mạng, poll endpoint count nhẹ
   mỗi **30–60 giây** (`K6`). **Không** tải trang inbox đầy đủ hay danh sách người Thấm công khai theo
   nhịp poll nền.
2. **Ẩn tab & Offline:** Khi tab ẩn (`visibilityState == "hidden"` / app nền) hoặc offline, tạm dừng
   polling nền; khi tab sáng lại hoặc kết nối trở lại, thực hiện kiểm tra lại có giới hạn + jitter
   (và exponential backoff khi nhận `503`/lỗi mạng).
3. **Đổi tài khoản / Đăng xuất (`A → B → A`):** Tăng generation phiên ở client; hủy bỏ hoặc bỏ qua
   mọi request đang bay, dữ liệu inbox, badge count và state `Read` của phiên trước. Danh sách Thấm
   công khai đang mở giữ phần dữ liệu công khai hợp lệ nhưng đặt lại trạng thái riêng (`own_state`)
   theo phiên mới hoặc góc nhìn khách.

---

## 3. Bộ ví dụ dữ liệu hợp đồng (dùng được ngoài Web)

### Ví dụ 1 — Event Envelope liên dịch vụ (`#346` → `#347`) & Receipt

```json
{
  "event_id": "019a9b10-0001-7000-8000-000000000101",
  "event_type": "comment.reply.approved",
  "schema_version": 1,
  "source_owner": "discussion",
  "activation_epoch": 1,
  "source_order": 2,
  "occurred_at": "2026-11-20T02:10:00Z",
  "actor_id": "11111111-1111-4111-8111-11111111111a",
  "recipients": [
    "22222222-2222-4222-8222-22222222222b",
    "33333333-3333-4333-8333-33333333333c"
  ],
  "target": {
    "kind": "work_comment_reply",
    "work_id": "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
    "root_id": "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
    "reply_target_id": "cccccccc-cccc-4ccc-8ccc-cccccccccccc",
    "comment_id": "dddddddd-dddd-4ddd-8ddd-dddddddddddd",
    "moderation_version": 2
  },
  "semantic_kind": "approved",
  "action_required": false,
  "payload_digest": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
}
```

Ví dụ envelope **Thấm** (`Add` và `Remove` mang `source_order = accepted_order` của `work_endorsements`):

```json
{
  "event_id": "019a9b10-0002-7000-8000-000000000102",
  "event_type": "work.endorsement.tham.changed",
  "schema_version": 1,
  "source_owner": "endorsement",
  "activation_epoch": 1,
  "source_order": 4,
  "occurred_at": "2026-11-20T02:11:12Z",
  "actor_id": "44444444-4444-4444-8444-44444444444d",
  "recipients": [
    "55555555-5555-4555-8555-55555555555e"
  ],
  "target": {
    "kind": "work_endorsement",
    "work_id": "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
    "revision_id": "eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee"
  },
  "semantic_kind": "remove",
  "action_required": false,
  "payload_digest": "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08"
}
```

Phản hồi ingestion của Notification Service về cho Relay (`#347` → `#346`):

```json
{
  "results": [
    {
      "event_id": "019a9b10-0001-7000-8000-000000000101",
      "status": "applied"
    },
    {
      "event_id": "019a9b10-0002-7000-8000-000000000102",
      "status": "already_applied"
    },
    {
      "event_id": "019a9b10-0003-7000-8000-000000000103",
      "status": "payload_mismatch",
      "reason": "existing_receipt_digest_differs"
    }
  ]
}
```

### Ví dụ 2 — Badge Count (`fresh`, `stale`, `503`)

```json
{
  "freshness": {
    "status": "fresh",
    "unseen_group_count": 3,
    "as_of_snapshot": "s1.00000042",
    "max_staleness_seconds": 120
  }
}
```

```json
{
  "freshness": {
    "status": "stale",
    "last_known_count": 3,
    "as_of_snapshot": "s1.00000042",
    "max_staleness_seconds": 120,
    "reason": "notification_backlog"
  }
}
```

```json
{
  "error": {
    "code": "dependency_unavailable",
    "message": "Không thể kiểm tra số thông báo chưa xem lúc này."
  }
}
```

### Ví dụ 3 — Trang Inbox, Typed Target & Lệnh `Seen`/`Read`/`MarkAllRead`

```json
{
  "inbox_snapshot": "s1.00000045",
  "items": [
    {
      "group_id": "019a9b20-1111-7000-8000-000000000001",
      "group_version": 5,
      "type": "work.endorsement.tham.changed",
      "is_read": false,
      "actor_count": 4,
      "actors_preview": [
        {
          "kind": "linked",
          "pen_name": "Lan Phương",
          "handle": "lan-phuong-12",
          "href": "/@lan-phuong-12"
        },
        {
          "kind": "linked",
          "pen_name": "Minh Khôi",
          "handle": "minh-khoi-08",
          "href": "/@minh-khoi-08"
        },
        {
          "kind": "deleted",
          "label": "Tài khoản đã xóa"
        }
      ],
      "target": {
        "kind": "work_endorsement",
        "work_id": "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
        "revision_id": "eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee",
        "work_title": "Chiều qua bến cũ"
      },
      "summary": {
        "kind": "work_tham_rollup",
        "total_actors": 4,
        "action_required": false
      },
      "created_at": "2026-11-18T10:00:00Z",
      "updated_at": "2026-11-20T02:15:00Z",
      "expires_at": "2027-05-17T10:00:00Z"
    },
    {
      "group_id": "019a9b20-1111-7000-8000-000000000002",
      "group_version": 1,
      "type": "work.moderation.decided",
      "is_read": false,
      "actor_count": 0,
      "actors_preview": [],
      "target": {
        "kind": "work_moderation_decision",
        "work_id": "ffffffff-ffff-4fff-8fff-ffffffffffff",
        "revision_id": "99999999-9999-4999-8999-999999999999",
        "decision_version": 3,
        "work_title": "Mùa gió chướng"
      },
      "summary": {
        "kind": "work_moderation_rejected",
        "action_required": true
      },
      "created_at": "2026-11-20T02:05:00Z",
      "updated_at": "2026-11-20T02:05:00Z",
      "expires_at": "2027-05-19T02:05:00Z"
    }
  ],
  "next_cursor": "n1.019a9b20111170008000000000000002",
  "continuation_reason": "more_items"
}
```

Lệnh ACK idempotent (Gateway nhận principal từ phiên):

```json
// Đánh dấu Seen khi mở chuông:
{
  "command_key": "019a9b30-0001-7000-8000-000000000001",
  "up_to_snapshot": "s1.00000045"
}

// Đánh dấu Read đúng phiên bản dòng vừa bấm (kể cả mở đích lỗi):
{
  "command_key": "019a9b30-0002-7000-8000-000000000002",
  "group_id": "019a9b20-1111-7000-8000-000000000002",
  "observed_group_version": 1
}

// Đánh dấu Đọc tất cả tới snapshot đang xác nhận:
{
  "command_key": "019a9b30-0003-7000-8000-000000000003",
  "up_to_snapshot": "s1.00000045"
}
```

### Ví dụ 4 — Chi tiết quyết định riêng của người viết (`P-12A`, `P-11a/b`)

Tác phẩm bị từ chối cần xử lý:

```json
{
  "kind": "work_moderation_decision",
  "work_id": "ffffffff-ffff-4fff-8fff-ffffffffffff",
  "work_title": "Mùa gió chướng",
  "revision_id": "99999999-9999-4999-8999-999999999999",
  "decision_version": 3,
  "outcome": "rejected",
  "action_required": true,
  "reason": {
    "code": "safety_policy",
    "message": "Nội dung vi phạm chính sách an toàn cộng đồng ở khổ thứ ba."
  },
  "can_appeal": true,
  "decided_at": "2026-11-20T02:05:00Z"
}
```

Tác phẩm được duyệt lại sau khi bị từ chối (`P-11b`):

```json
{
  "kind": "work_moderation_decision",
  "work_id": "ffffffff-ffff-4fff-8fff-ffffffffffff",
  "work_title": "Mùa gió chướng",
  "revision_id": "99999999-9999-4999-8999-999999999999",
  "decision_version": 4,
  "outcome": "approved",
  "action_required": false,
  "reason": {
    "code": "appeal_reversed",
    "message": "Phản đối được chấp nhận; phiên bản đã được duyệt lại."
  },
  "can_appeal": false,
  "decided_at": "2026-11-20T02:20:00Z"
}
```

Bình luận bị kiểm duyệt gỡ (không phục hồi thân chữ đã gỡ, không lộ tên moderator/ghi chú nội bộ):

```json
{
  "kind": "comment_moderation_decision",
  "work_id": "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
  "work_title": null,
  "comment_id": "77777777-7777-4777-8777-777777777777",
  "moderation_version": 4,
  "outcome": "removed",
  "action_required": false,
  "comment_body": null,
  "reason": {
    "code": "harassment",
    "message": "Bình luận chứa lời lẽ công kích cá nhân không phù hợp tiêu chuẩn cộng đồng."
  },
  "decided_at": "2026-11-20T02:12:00Z"
}
```

### Ví dụ 5 — Danh sách người Thấm công khai có phân trang (`P-08 R3`)

Khách chưa đăng nhập đọc danh sách người đang Thấm của một bản công khai hiện hành (`Cache-Control:
private, no-cache`, kèm strong `ETag`; lấy từ `work_endorsements` + `public_author_attribution`, độc
lập hoàn toàn với `actors_preview` và tuổi 180 ngày của Notifications):

```json
{
  "work_id": "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
  "revision_id": "eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee",
  "total_count": 27,
  "items": [
    {
      "author": {
        "kind": "linked",
        "pen_name": "Lan Phương",
        "handle": "lan-phuong-12",
        "href": "/@lan-phuong-12"
      },
      "endorsed_at": "2026-11-20T02:15:00Z"
    },
    {
      "author": {
        "kind": "deleted",
        "label": "Tài khoản đã xóa"
      },
      "endorsed_at": "2026-04-01T08:30:00Z"
    },
    {
      "author": {
        "kind": "unavailable",
        "label": "Người đọc Vink"
      },
      "endorsed_at": "2026-03-10T12:00:00Z"
    }
  ],
  "next_cursor": "e1.eeeeeeeeeeee.0000000000000003"
}
```

*(Lưu ý ví dụ trên minh họa lượt Thấm từ `2026-03-10` — hơn 180 ngày trước `2026-11-20` — vẫn còn
hiệu lực trên danh sách công khai sau khi inbox Notifications đã purge theo `P-08` & `P-13B`.)*

---

## 4. Bàn giao riêng sang giai đoạn Design & Web (`Epic AC-16` & `AC-17`)

Phần việc sau nằm ngoài phạm vi tài liệu `#345` và ngoài các lượt BE `#346`–`#348`, được bàn giao cho
giai đoạn Design/Web sau khi hợp đồng và API BE sẵn sàng:

1. **Lập LayoutSpec (`vink-layout-spec`, `vink-ui-design` — `Epic AC-16`):**
   - Bề mặt chuông & badge trên thanh điều hướng thích ứng (desktop rail / tablet / mobile bottom bar).
   - Popover/sheet/trang Inbox: dòng gom nhóm (`actors_preview <= 3`), phân biệt `unread`/`read`, nút
     “Đọc tất cả”, trạng thái trống/loading/lỗi/`stale`/`503` (không thành `0`/trống).
   - Màn chi tiết quyết định riêng (`P-12A`) cho tác phẩm & bình luận (tôn trọng `body: null`, nhãn
     “Cần xử lý” đúng `P-11a`, lối đi sửa bài hoặc khiếu nại theo `MOD-10`).
   - Danh sách người Thấm công khai có phân trang (`P-08 R3`) trên Reader: preview tổng số + kích
     hoạt danh sách, tải thêm “Xem thêm”, xử lý `unlike` giữa trang / `409 revision_changed`, tách
     biệt khách và người có phiên.
2. **Triển khai Web & Kiểm thử trình duyệt (`vink-leptos-web` — `Epic AC-17`):**
   - Bộ điều phối polling 30–60 giây cho count nhẹ, dừng khi tab ẩn/offline, backoff/resume khi hồi
     phục, tăng generation hủy response muộn khi logout/đổi tài khoản (`A → B → A`).
   - Kiểm chứng Chrome headless: 320–1440px, Light/Dark, chữ 100%–200%, phím `Tab`/`Enter`/`Escape`,
     hit target `≥ 44×44` px, trả focus khi đóng sheet/dialog hoặc bấm “Xem thêm”, SSR không lộ dữ
     liệu riêng tư, khách đọc được danh sách Thấm công khai không cần JS/đăng nhập (hoặc qua trang
     tải thêm có lối thử lại).
3. **Ngoài phạm vi Epic v1 (`DEFERRED`):** Giao diện CMP/native (`P-14A` chỉ giữ contract dữ liệu dùng
   lại được ở trên), kênh gửi quản trị thực tế (`P-05`), và Follow Notifications.
