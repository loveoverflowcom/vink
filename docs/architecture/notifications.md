# Thông báo (Notifications v1) — Đặc tả miền, Event Inventory và UX hành vi

**Ngày:** 20/11/2026 · **Nguồn:** [Epic #305 (R1/R2/R3 ngày 18/11/2026)](https://github.com/loveoverflowcom/vink/issues/305),
[Issue #345](https://github.com/loveoverflowcom/vink/issues/345).
[PRD §16](../../prd/requirements.md#16-thông-báo-cộng-đồng-và-danh-sách-thấm-công-khai-notifications-v1)
sở hữu yêu cầu sản phẩm `NOTIF-01..10`. Tài liệu này sở hữu ranh giới miền, bảng sự kiện nguồn
(inventory), chuyển trạng thái `Seen`/`Read`/gom nhóm/hết hạn và **đặc tả UX bằng chữ** cho chuông,
inbox, chi tiết quyết định và danh sách người **Thấm** công khai.
[Hợp đồng sự kiện & Gateway v1](../../services/vink-app/contracts/http/v1/notifications.md) sở hữu
ngữ nghĩa envelope, receipt/ACK, snapshot/version, cursor, lỗi và ví dụ JSON độc lập Web.
[Bảng truy vết & bàn giao](../development/notifications.md) ghi ánh xạ đầy đủ `N1–N8`, `K1–K6`,
`P-01..P-14`, `Epic AC-01..19` và đường sang `#346`, `#347`, `#348` cùng đợt Design/Web.

| Chiều trạng thái | Ghi nhận hiện tại |
| --- | --- |
| Quyết định sản phẩm | `PRODUCT_CONFIRMED` trong brief `#305` R1–R3 (`N1–N8`, `P-01..P-14` / `NOTIF-01..10`). Proposal [VDR-NOTIFICATIONS@r1](../../decisions/proposals/VDR-NOTIFICATIONS.r1.toml) đang `PENDING` chờ signoff theo `VDR-DECISION-SIGNOFF@r1`. |
| Quyết định kiến trúc | `DESIGN_PROPOSED` (`ARCHITECTURE_DIRECTION_CONFIRMED` cho `K1–K6` từ lời chủ sản phẩm, chưa phải ADR/TOML `ACCEPTED`). Proposal [VDR-NOTIFICATIONS-CONTRACT@r1](../../decisions/proposals/VDR-NOTIFICATIONS-CONTRACT.r1.toml) và [LVE-PR-02-03-CHOICE-12@r2](../../decisions/proposals/LVE-PR-02-03-CHOICE-12.r2.toml) đang `PENDING`; `LVE-PR-02-03-CHOICE-12@r1` (`DESIGN_PROPOSED`) vẫn là revision hiện hành. `T-01..T-08` là `TECHNICAL_OPEN` giao `#346`–`#348`. |
| Mức triển khai | `#346`: producer phía App (năm nhóm v1 + invalidation), outbox thông báo có kích hoạt và relay ACK có fence ở mức `persistence-api` ([registry §6](../development/notifications.md#6-hiện-trạng-346-producer-outbox-và-relay-phía-app)); relay chưa chạy trong `vink-worker` vì chưa có transport `T-07` (#347). `NOT_IMPLEMENTED` cho Notification DB/service, Gateway inbox/`Seen`/`Read`/chi tiết quyết định, danh sách người Thấm công khai, LayoutSpec và UI Web. |
| Bằng chứng | Đối chiếu source trên `develop@1a0c4e31`, kiểm định kho TOML, link và ranh giới tài liệu qua `verify quick`. Kiểm thử runtime/unit/PostgreSQL/HTTP/browser/CMP cho Notifications: `NOT_RUN`. |

<!-- vdr: active D-05@r1 -->
<!-- vdr: active D-14@r1 -->
<!-- vdr: active LVE-PR-02-03-CHOICE-12@r1 -->
<!-- vdr: active O-17@r2 -->
<!-- vdr: active VDR-DECISION-SIGNOFF@r1 -->

---

## Bổ sung ngày 20/11/2026 từ #365

Chủ sản phẩm chốt “Có, báo Ưng”. P-08b được mở lại cho **thông báo** Ưng nhận xét.
P-08a tiếp tục sở hữu danh sách **Thấm tác phẩm** công khai. Các bảng R3 dưới đây giữ mốc lịch sử
18/11/2026. Đề xuất [VDR-NOTIFICATIONS@r2](../../decisions/proposals/VDR-NOTIFICATIONS.r2.toml)
và [VDR-NOTIFICATIONS-CONTRACT@r2](../../decisions/proposals/VDR-NOTIFICATIONS-CONTRACT.r2.toml)
ghi delta và giữ `PENDING`.

| Phần bổ sung | Nguồn và vòng đời |
| --- | --- |
| Trích đoạn | `comment.root.approved` và `comment.reply.approved`. App Gateway hydrate từ đóng góp còn quyền mới nhất đã kiểm (`excerpt: {text, truncated}`, NFC, tối đa 140 extended grapheme cluster). Hold, gỡ, rút bài, epoch công khai và standing được kiểm ở App. |
| `comment.endorsement.ung.changed` | Endorsement phát Add/Remove trên nhận xét (`EndorsementTarget::Comment`) trong cùng transaction với lệnh được chấp nhận. Người nhận là tác giả nhận xét, loại self. Khóa nhóm theo `(recipient, event_type, comment_id)`. Báo lần đầu theo actor+nhận xét (`P-02B`); bỏ Ưng cập nhật nhóm chưa đọc, ẩn khi về 0 (`P-03A`). |
| Đích Ưng | `work_comment_endorsement` mang `work_id` và `comment_id`. App kiểm nhận xét và tác phẩm ở lần đọc/mở; dòng Ưng không có trích đoạn (N4 của `@r2` chỉ cho dòng duyệt nhận xét). Hiện trạng runtime ở [registry §9](../development/notifications.md#9-hiện-trạng-365-thông-báo-ưng-nhận-xét). |

**Nguồn:** [Epic #305 và lời chốt](https://github.com/loveoverflowcom/vink/issues/305)
· [AC của #365](https://github.com/loveoverflowcom/vink/issues/365)
· [hợp đồng HTTP](../../services/vink-app/contracts/http/v1/notifications.md#24-typed-target--chi-tiết-quyết-định-riêng-p-12a-p-14a).

## 1. Mục tiêu, phạm vi v1 và thu hẹp R3

1. **Mục tiêu:** Cung cấp hộp thư thông báo riêng cho từng tác giả/người dùng qua một Notification
   Service độc lập, gom nhóm **ngay khi ghi** (`Write-time Rollup`), giữ `Thin Pointer` và kiểm quyền
   hiện hành tại `App Gateway` để không lộ nội dung đã ẩn/gỡ và không lặp nhiễu khi retry hay
   bỏ-rồi-làm-lại. Đồng thời cung cấp danh sách công khai có phân trang những người đang **Thấm** bản
   công khai hiện hành của tác phẩm (`P-08 R3`).
2. **Phạm vi runtime v1 (`N1`, `P-05`, `P-08`):**
   - **Năm nhóm nghiệp vụ cần triển khai trong v1:** (1) bình luận gốc được duyệt, (2) phản hồi được duyệt,
     (3) quyết định kiểm duyệt tác phẩm, (4) bình luận bị từ chối hoặc gỡ do kiểm duyệt, (5) Thấm tác phẩm.
   - **Nhóm thứ sáu — Thông báo quản trị (`P-05`):** chỉ dành trước `type` trong hợp đồng
     (`TYPE_ONLY_NOW`). Mọi công cụ soạn/gửi, sửa/rút, danh xưng người gửi, ủy quyền operator và
     broadcast đều `DEFERRED`. Theo `N7`, mặc định bật tất cả loại *đã triển khai* không bao giờ bật
     loại reserved/deferred.
   - **Loại bỏ khỏi phạm vi Epic (`P-08b REMOVED_FROM_SCOPE`):** **Hữu ích** trên bình luận không có
     producer thông báo, không có inbox item và không có danh sách người ghi nhận trong Epic `#305`.
     Code/API tổng số + trạng thái riêng hiện hữu của LVE bình luận được giữ nguyên (không tự ý xóa).
   - **Hoãn (`DEFERRED`):** Follow Notifications (chờ Social Graph `#284` và một lần giao sau), push OS,
     email thông báo, WebSocket/SSE, preference UI nâng cao và native/CMP UI. LayoutSpec và UI Web thuộc giai đoạn Design/Web của Epic (AC-16/17), sau lượt đặc tả này.
     `P-14A` chỉ yêu cầu hợp đồng dữ liệu/đích nghiệp vụ dùng lại được cho CMP/mobile ngay từ v1.

---

## 2. Thuật ngữ domain và ranh giới sở hữu

| Tên trong sản phẩm | Tên trong code / hợp đồng hiện có hoặc đề xuất | Trạng thái | Ghi chú ranh giới |
| --- | --- | --- | --- |
| Chủ hộp thư / Người nhận | `PrincipalId` (`recipient`) | `existing` | Định danh bởi `App` (`principal_bindings`), giải từ phiên `Identity`; client không được khai báo. |
| Người tác động / Tác nhân | `actor_id` (`PrincipalId`) + phép chiếu công khai (`pen_name`, `handle`, `href`) | `existing` (chiếu) | Chỉ là ID ở sự kiện/Notification; Gateway hydrate danh tính công khai hiện hành lúc đọc. |
| Bình luận gốc / Phản hồi một tầng | `CommentPlacement`, `StoredPosition::Root \| Reply { root_id, reply_to }` | `existing` | Vink chỉ có root + 1 tầng reply phẳng; `reply_to` giữ ngữ cảnh trả lời, không tạo tầng thứ ba. |
| Thấm (tác phẩm) | `EndorsementTarget::Work { work: WorkId, revision: RevisionId }` | `existing` | Neo theo `WorkId + RevisionId`; đổi revision tạo neo mới độc lập. |
| Mốc công khai hiện hành / Epoch | `publications`, `Publication::notification_stamp`, `PublicEpoch` | `existing` | `COM-11`: ghim `NotificationStamp { epoch, readable }` lúc commit tương tác. Khi sự kiện đến muộn, chỉ báo tác giả nếu [`should_notify_author`](../../services/vink-app/src/domain/publication.rs) còn chấp nhận dấu này. Quyết định riêng của người viết tuân N3. |
| Quyết định kiểm duyệt tác phẩm | `WorkModeration`, `DecisionVersion`, `Moderation`, `DecisionProvenance` | `existing` | Lưu theo `(work_id, revision_id, decision_version)` trong `work_revision_moderation` và lịch sử. |
| Khóa gom nhóm | `group_key` (`recipient + type + target_key`) | `proposed` | Duy nhất tối đa 1 nhóm chưa đọc (`unread`) còn hiệu lực cho mỗi `group_key`. |
| Nhóm thông báo | `NotificationGroup` (`group_id`, `group_version`, `signal_version`, `read_at`) | `proposed` | Tuổi tối đa 180 ngày từ lúc tạo (`created_at`); không gia hạn khi có tương tác mới. |
| Dấu chưa xem / Đã đọc | `Seen` (`seen_snapshot`, user-level) & `Read` (`group_version`, item-level) | `proposed` | `Seen` cập nhật theo snapshot khi mở chuông; `Read` xác nhận riêng từng dòng hoặc mark-all tới snapshot. |
| Nhóm tạm ẩn do về 0 | `suppressed_empty` / nhóm unread có `actor_count = 0` | `proposed` | `P-03A`: bỏ Thấm/gỡ tương tác làm nhóm unread về 0 thì ẩn/hết hiệu lực, không giả `Read`. |
| Dấu chống báo lại suốt đời | `suppression marker` (`actor + loại + WorkId + RevisionId`) | `proposed` | Bảo vệ `P-02B` sau khi relike hoặc sau 180 ngày purge inbox; không là nguồn dựng danh sách công khai. |
| Danh sách người Thấm công khai | `PublicEndorserList` từ `work_endorsements` (`selected = true`) | `proposed` | Thuộc App/Endorsement qua Gateway; khách được đọc khi bản đang công khai; độc lập inbox 180 ngày. |

### Ranh giới sở hữu theo kiến trúc `K1–K6`

| Thành phần / Fact | Owner duy nhất | Đầu vào / Giao thức | Cấm vượt ranh giới |
| --- | --- | --- | --- |
| Xác thực phiên, tài khoản, đình chỉ mức Identity | `services/vink-identity` (`vink.identity.v1`) | `App Gateway` gọi qua socket để resolve session. | Notification không gọi Identity trực tiếp và không đọc DB `vink_identity`. |
| Nghiệp vụ gốc, recipient lúc commit, outbox & Relay | `services/vink-app` (`persistence/*`, worker/relay `#346`) | Commit transaction App ghi nghiệp vụ + outbox; Relay gửi ra ngoài tx tới Notification. | Cấm transaction xuyên 2 DB; cấm network call trong `EffectHandler` hiện tại; cấm đưa body/email/secret/ghi chú nội bộ vào event/log. |
| Lưu trữ inbox, rollup, receipt, `Seen`/`Read`, retention 180 ngày | `vink-notifications` + DB PostgreSQL riêng (`#347`) | Nhận ingestion event có auth từ Relay; nhận truy vấn inbox/ACK nội bộ có auth từ Gateway. | Không tự đọc DB `vink` hay `vink_identity`; không mở port trực tiếp cho client; không cấp quyền hiển thị nội dung. |
| Kiểm quyền đọc, hydrate danh tính/nội dung theo lô, public Thấm list | `services/vink-app` Gateway + Endorsement (`#348`) | Client gọi `/api/v1/*`; Gateway đọc phiên, gọi Notification cho con trỏ/nhóm và tự kiểm quyền + hydrate từ App DB. | Không tin `recipient` từ client; không suy danh sách Thấm công khai từ inbox/preview/receipt. |
| Trình bày Web và bộ đệm phiên trình duyệt | `apps/vink-web` (đợt Design/Web) | Tiêu thụ Gateway API v1 + polling coordinator; CMP/mobile tái sử dụng cùng contract. | Không dùng `HTML`-only hoặc cấu trúc riêng Leptos trong hợp đồng đích/lỗi API. |

---

## 3. Quyền, kiểm tra bền vững và bất biến cốt lõi

| Actor / Bối cảnh | Hành động | Điều kiện được phép | Kiểm tra bền vững khi đọc / ghi |
| --- | --- | --- | --- |
| Khách chưa đăng nhập | Xem danh sách người Thấm công khai | Tác phẩm đang có `publications` (công khai hiện hành) tại đúng `RevisionId` | Đọc trong một snapshot `App DB`, không yêu cầu phiên; bài ẩn/rút/bị bác/revision cũ trả `404` (hoặc trạng thái không khả dụng đồng nhất). |
| Khách chưa đăng nhập | Xem badge, inbox, `Seen`/`Read`, chi tiết quyết định riêng | Bị từ chối | Với request hợp lệ về Origin, trả `401` trước khi đọc body hay gọi Notification Service. Middleware kiểm Origin trước ở mọi lệnh ghi. |
| Thành viên `Active` | Xem badge, inbox, `Seen`/`Read`, “Đọc tất cả” | Có phiên thật; đọc chỉ inbox của chính `PrincipalId` lấy từ phiên; thao tác ghi kiểm `Origin` và standing `Active` | Gateway trích `principal` từ session/Identity, chặn đọc/ACK hộ người khác; ghi `Seen`/`Read` qua `principal::require_active`. |
| Chủ tác phẩm / Tác giả bình luận | Mở chi tiết quyết định riêng (`P-12A`) | Phiên `Active` và là chính chủ tác phẩm hoặc chính tác giả bình luận đó | Kiểm quyền sở hữu ở App DB trong snapshot đọc; người khác (kể cả chủ tác phẩm với bình luận người khác) nhận cùng `404 not_found` như khi quyết định không tồn tại. |
| Người nhận mất tư cách / Bị đình chỉ | Dùng inbox / Nhận sự kiện mới | `principal_suspensions` chặn toàn bộ API riêng; với tài khoản đã xóa (`SOC-DEP-01`), dọn theo fence | Standing fence tại mọi ghi; Gateway từ chối mọi thao tác riêng tư ngay khi phiên mất hiệu lực. |
| App Relay / App Gateway nội bộ | Giao event / Đọc-ghi inbox nội bộ | Xác thực peer/service token & scope hợp lệ (`T-07`) | Loopback/IP nội bộ không đủ làm xác thực; từ chối caller sai credentials hoặc sai scope. |

**Bất biến phải giữ (`NOTIF-INV`):**
1. **Atomic source vs decoupled relay:** Enqueue outbox lỗi bắt buộc rollback giao dịch nghiệp vụ nguồn; sau khi App commit, lỗi Relay hay Notification Service không đảo ngược giao dịch đã commit.
2. **Idempotency & Payload safety:** Cùng `event_id` + cùng payload tạo tối đa một hiệu ứng inbox; cùng `event_id` khác payload bị từ chối/quarantine để đối soát, không âm thầm coi là replay hợp lệ.
3. **Zero privacy leak (`N4`):** Notification Service không sở hữu nội dung hay quyền đọc. Bất kỳ thu hồi quyền nào (ẩn bài `PUB-07`, bác bản đang đăng `O-17@r2`, hold bình luận `post_review`, gỡ bình luận, xóa bài/tài khoản) phải chặn rò rỉ ngay ở lần đọc/mở đích kế tiếp tại Gateway.
4. **No false zero / no silent drop:** Lỗi service hoặc quá hạn freshness trả `stale` hoặc `503`, tuyệt đối không biến thành `0` hoặc danh sách rỗng; `Read` theo dòng chỉ ghi cho `(group_id, observed_group_version)` và `MarkAllRead` chỉ áp dụng tới `inbox_snapshot` đã xác nhận.
5. **180-day ceiling & Endorsement separation:** Dữ liệu inbox/preview/membership của Notifications hết hạn tuyệt đối sau tối đa 180 ngày từ lúc tạo nhóm; purge Notifications không xóa lượt Thấm còn hiệu lực ở Endorsement (`work_endorsements`), và dữ liệu kỹ thuật (receipt/suppression) không được dùng để tái dựng inbox đã xóa.

---

## 4. Event Inventory v1, Invalidation Plumbing và Ví dụ nhập/xuất (`AC-02`)

Các định danh `event_type` dưới đây là **tên chuẩn hóa ở hợp đồng đích** (chưa tồn tại trong `services/vink-app/src/domain/discussion/outbox.rs`, nơi mới chỉ có `CommentSubmitted` và `OperatorReviewRequested`).

### 4.1. Bảng Inventory 5 nhóm phát runtime v1 + 1 type quản trị dành trước

Với bình luận, [`CommentState::decide`](../../services/vink-app/src/domain/discussion/lifecycle.rs)
trả `Transition::Changed` và tăng version cả khi xác nhận lại cùng kết luận. Producer so sánh trạng thái
trước và sau thay đổi trong cùng giao dịch App, rồi ghi nghĩa vụ giao trước khi commit. Một lần duyệt có thể có nhiều người nhận trong **một** envelope.
Notification áp dụng tối đa một đóng góp cho mỗi người nhận của envelope đó.

Khi một bình luận trong nhóm root/reply bị gỡ hoặc mất quyền, chỉ bỏ đóng góp của bình luận đó.
Ví dụ B viết hai bình luận rồi gỡ bình luận mới nhất. Nếu bình luận trước vẫn hợp lệ, B vẫn được
đếm một lần và đích mở trỏ tới bình luận hợp lệ mới nhất. Nhóm unread chỉ ẩn khi hết đóng góp hợp lệ.

| Loại sự kiện & trạng thái | Transition / Transaction nguồn & ca KHÔNG phát | Actor | Người nhận (`recipients`) | Self & Dedup | Quyền lúc phát & lúc đọc | `group_key` | Cách đếm & `actors_preview` | Đích & phiên bản | Mất hiệu lực / Invalidation & Purge | Owner |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| **`comment.root.approved`** (`RUNTIME_V1`) | **Phát:** `persistence::discussion::moderate_comment` khi `Approve` chuyển `comments` (`kind='root'`, `removal IS NULL`) từ non-approved → `approved`, **VÀ** tác phẩm đang công khai (`publications` hiện hữu / `readable = true`, epoch hiện hành).<br>**KHÔNG phát:** `submit_comment` (`comment.submitted`), `Approved → Approved` (tăng version nhưng giữ nguyên kết luận), tác phẩm đang ẩn (`PUB-07` / `COM-11`), đã bác (`O-17@r2`), bị gỡ hoặc chưa public. Không phát bù khi tác phẩm công khai lại. | Tác giả bình luận gốc (`comments.author_id`) | Tác giả tác phẩm (`works.owner_id`) | Loại nếu `actor == recipient`. Một người nhận duy nhất trên nhóm. | **Phát:** Bình luận `Present + Approved` và tác phẩm đang public.<br>**Đọc:** Gateway kiểm lại `publications` + `CommentView::Public` + standing; ẩn/gỡ/hold thì lọc bỏ trước hiển thị/mở đích. | `(recipient, "comment.root.approved", work_id)` | `actor_count`: số tác giả bình luận **khác nhau** còn hợp lệ trong nhóm.<br>`actors_preview`: ≤ 3 `actor_id` mới nhất. | `work_comment_thread` (`work_id`, `comment_id`, `moderation_version`) | Đóng góp của bình luận hết hiệu lực nếu bị gỡ (`remove_own_comment` / `ByModeration`), hold (`post_review::apply`), tác phẩm rút/ẩn/bị bác/xóa. Nhóm xóa sau ≤ 180 ngày từ tạo. | `#346` producer/relay<br>`#347` rollup<br>`#348` Gateway |
| **`comment.reply.approved`** (`RUNTIME_V1`) | **Phát:** `persistence::discussion::moderate_comment` khi `Approve` chuyển `comments` (`kind='reply'`, `removal IS NULL`) từ non-approved → `approved`, **VÀ** tác phẩm đang public (epoch hiện hành). Đọc `root_id`, `reply_to_id` và `replied_to_reply` để phân biệt trả lời trực tiếp root với ngữ cảnh reply đã purge (§4.3 B).<br>**KHÔNG phát:** reply pending/rejected, xác nhận lại cùng kết luận, bài đang ẩn/bác (`COM-11`), hoặc phát bù sau tái công khai. | Tác giả reply (`comments.author_id`) | - Nếu trả lời trực tiếp root: `{author(root)}`. Nếu đích đã purge: chủ root còn đủ điều kiện và tác giả đích trực tiếp nếu nguồn còn xác định được.<br>- Nếu `reply_to_id = Some(target)`: `{author(target), author(root)}`. | Loại nếu `recipient == actor`; khử trùng theo `PrincipalId` (nếu `author(target) == author(root)`, chỉ tạo 1 recipient/1 sự kiện gửi người đó). | **Phát:** Reply `Present + Approved` trên bài public (không phụ thuộc ngữ cảnh trả lời còn hiện diện hay đã `Unavailable`, miễn reply được duyệt).<br>**Đọc:** Gateway kiểm lại bài còn mở/thay thế hợp lệ và reply còn `CommentView::Public`; ngữ cảnh gốc tuân `ReplyContext`. | `(recipient, "comment.reply.approved", reply_target_id)` theo đích trả lời trực tiếp trong root (`P-07A`). Với ngữ cảnh đã purge, dùng khóa riêng như mô tả ở §4.3 B. | `actor_count`: số tác giả reply **khác nhau** trả lời vào cùng đích trực tiếp.<br>`actors_preview`: ≤ 3 `actor_id` mới nhất. | `work_comment_reply` (`work_id`, `root_id`, `reply_target_id`, `comment_id`, `moderation_version`) | Đóng góp của reply hết hiệu lực nếu reply bị gỡ/hold hoặc bài không còn đọc được. Nếu ngữ cảnh gốc bị gỡ nhưng reply vẫn public theo chính sách thread thì reply vẫn tồn tại theo quyền hiện hành. Xóa sau ≤ 180 ngày. | `#346` producer/relay<br>`#347` rollup<br>`#348` Gateway |
| **`work.moderation.decided`** (`RUNTIME_V1`) | **Phát:** 3 commit seams khi kết luận thật đổi sang `Approved` hoặc `Rejected`:<br>1. `persistence::evaluation::finish_job` (`Applied(Published \| ApprovedUnpublished \| Rejected { .. })`)<br>2. `persistence::moderation::conclude_case` (`Approved \| Rejected`)<br>3. `persistence::appeals::conclude` (`AppealVerdict::Reverse` đổi kết luận trên revision bị bác thành `Approved`).<br>**KHÔNG phát:** `NeedsAdmin` (`evaluator` uncertain/invalid/exhausted), `Retryable`, `Refused`, `Stale`, `Rerouted`, `set_private`, `request_public` khi giữ nguyên approval cũ (`Reaffirmed`), `Uphold` chỉ tăng version và giữ kết luận, `Approved → Approved` (reaffirmation không đổi kết quả trên cùng vòng quyết định) hay callback cũ trễ. **Phát mới khi `Approved → Rejected → Approved` (`P-11b`).** | Hệ thống AI (`ai_evaluator`) hoặc quyết định kiểm duyệt (`moderation_authority`; **không lộ danh tính moderator/admin ra ngoài** theo `MOD-12`/`P-12A`) | Tác giả tác phẩm (`works.owner_id`) | **Không** tự loại tác giả (`N2`, `N3`: chủ nội dung vẫn nhận thông báo quyết định về bài của mình). | **Phát:** Quyết định gắn đúng `revision_id` + `decision_version` + `outcome` (`approved \| rejected`) + `action_required` (`true` chỉ khi tác giả thực sự cần hành động. Quyền gửi phản đối là một trường riêng).<br>**Đọc:** Chỉ chủ tác phẩm đọc qua phiên thật (`P-12A`); không cần bài đang công khai. | `(recipient, "work.moderation.decided", work_id, revision_id, outcome)` — tách biệt `approved` với `rejected` và giữa các revision để không gộp nhầm nghĩa quyết định (`P-07A`). | Nhóm theo quyết định (`item_count = 1` cho cặp `(revision, decision_version)` hoặc số lần đảo kết luận cùng `outcome` trước khi Read); không hiển thị danh sách người duyệt. | `work_moderation_decision` (`work_id`, `revision_id`, `decision_version`) mở màn chi tiết quyết định riêng (`P-12A`). | Chỉ đọc được bởi chủ sở hữu còn tư cách hợp lệ; `approved` trên revision cũ vẫn giữ lịch sử quyết định nhưng không đánh lừa rằng revision đó đang đăng; xóa sau ≤ 180 ngày từ tạo nhóm. | `#346` producer/relay<br>`#347` rollup<br>`#348` Gateway |
| **`comment.moderation.adverse`** (`RUNTIME_V1`) | **Phát:** `persistence::discussion::moderate_comment` khi chuyển sang `Review::Rejected` (`ModerationAction::Reject`) hoặc `Removal::ByModeration` (`ModerationAction::Remove`).<br>**KHÔNG phát:** `ModerationAction::Approve`, `remove_own_comment` (tác giả bình luận tự gỡ của mình), `Rejected → Rejected`, gỡ lặp hoặc stale version. So sánh review và removal trước/sau trong cùng giao dịch. | Kiểm duyệt viên/hệ thống (ẩn danh tính cá nhân người duyệt theo `MOD-12`) | Tác giả bình luận (`comments.author_id`) | **Không** tự loại người viết; không gửi cho tác giả bài viết (`COM-08`). | **Phát:** Quyết định bất lợi đã commit; độc lập bài có đang công khai hay đang ẩn/tombstone (`N3`).<br>**Đọc:** Chỉ người viết bình luận đọc qua màn chi tiết quyết định riêng (`P-12A`). Tuyệt đối không phục hồi thân bình luận nếu đã bị gỡ; chỉ hiện lý do được phép. | `(recipient, "comment.moderation.adverse", comment_id, outcome_kind)` với `outcome_kind ∈ {rejected, removed}`. | `1` quyết định bất lợi gắn `moderation_version`; không đếm diễn viên bên ngoài. | `comment_moderation_decision` (`work_id`, `comment_id`, `moderation_version`) mở màn chi tiết quyết định riêng (`P-12A`). | Chỉ người viết bình luận có phiên hợp lệ đọc được quyết định dù bình luận đã gỡ; xóa khỏi inbox sau ≤ 180 ngày từ tạo nhóm. | `#346` producer/relay<br>`#347` rollup<br>`#348` Gateway |
| **`work.endorsement.tham.changed`** (`RUNTIME_V1`) | **Phát:** `persistence::endorsement::set_endorsement` khi target là `EndorsementTarget::Work { work, revision }` **VÀ** `EndorsementOutcome::Accepted(write)` có `write.delta()` là `EndorsementDelta::Add` hoặc `EndorsementDelta::Remove` (bằng chứng là `version` đổi; kèm `accepted_order`, `version`).<br>**KHÔNG phát:** `EndorsementTarget::Comment` (**Hữu ích ngoài scope R3**), `EndorsementDelta::None` (same-state fence), `Replay`, `Conflict` hoặc `Refused` (gồm chọn mới trên revision cũ). | Người bấm Thấm/Bỏ Thấm (`actor`) | Tác giả tác phẩm (`works.owner_id`) | Chặn tự chọn bằng `EndorsementRefusal::OwnContent` tại `decide_endorsement`; loại self một lần nữa nếu có. | **Phát:** `Add` chỉ xảy ra khi tác phẩm đang công khai đúng `revision_id` (`visible = true`); `Remove` luôn truyền về để đối soát nhóm unread theo `P-03A` kể cả khi bài vừa chuyển ẩn/rút hoặc đã đăng revision mới. Một `Remove` được chấp nhận trên lựa chọn riêng vẫn mang neo revision cũ.<br>**Đọc:** Kiểm quyền tác phẩm & danh tính public hiện hành của các actor; suppression `P-02B` không bao giờ tự lộ ở giao diện. | `(recipient, "work.endorsement.tham.changed", work_id, revision_id)` | **Inbox (`P-07A`):** `actor_count` = số người **khác nhau** còn hiệu lực trong nhóm unread (có thể nhỏ hơn tổng Thấm vì chỉ tính lượt vào nhóm đó và loại relike sau `P-02B`); `actors_preview` ≤ 3 người gần nhất.<br>**Danh sách công khai (`P-08`):** đọc trực tiếp từ `work_endorsements`, độc lập hoàn toàn. | `work_endorsement` (`work_id`, `revision_id`) | **Suppression `P-02B`:** mỗi `(recipient, "work.endorsement.tham.changed", work_id, revision_id, actor)` chỉ tạo tín hiệu mới tối đa 1 lần suốt vòng đời.<br>`Remove` bỏ contribution ở nhóm unread, còn 0 thì ẩn nhóm (`P-03A`).<br>Relike đối soát membership im lặng, không mở lại tín hiệu/nhóm mới. Inbox xóa ≤ 180 ngày; dấu `P-02B` giữ bằng chứng tối thiểu độc lập (`P-13B`). | `#346` producer/relay<br>`#347` rollup + `P-02B`/`P-03A`<br>`#348` Gateway + Endorsement list |
| **`admin.notice.reserved`** (`TYPE_ONLY_NOW`) | **Không có producer trong v1.** Chỉ định nghĩa schema type/enum dành trước cho tương lai (chủ sản phẩm/admin gửi thông báo tới danh sách user chỉ định hữu hạn, có dedup). `N7` không bật type này. | `Admin` (tương lai, `DEFERRED`) | Người dùng chỉ định (tương lai, `DEFERRED`) | Không gửi đại trà/broadcast; chi tiết `DEFERRED`. | Không có endpoint/công cụ gửi hoặc quyền operator ủy quyền trong v1. | `(recipient, "admin.notice.reserved", notice_id)` (dự kiến) | `1` thông báo quản trị (dự kiến) | Chưa phát hành đích thực tế (`DEFERRED`). | V1 từ chối hoặc bỏ qua mọi ý định phát thực tế nếu chưa kích hoạt hợp đồng tương lai. | `#345` hợp đồng<br>`#347` type-only |

### 4.2. Invalidation Plumbing (không phải loại thông báo mới)

Khi một đối tượng, quyền đọc hoặc tư cách tài khoản thay đổi sau khi thông báo đã được phát, tầng nguồn
ghi nghĩa vụ invalidation/đối soát cho số đếm và đóng góp của nhóm. Gateway luôn kiểm quyền hiện hành khi trả danh sách, preview hoặc mở đích, kể cả lúc invalidation đang chờ giao (`N4`).

| Biến cố nguồn | Owner / Commit seam hiện hữu hoặc phụ thuộc | Tác động bắt buộc lên Notifications | Tác động lên danh sách Thấm công khai (`P-08`) |
| --- | --- | --- | --- |
| Tác giả bình luận tự gỡ bình luận | `persistence::discussion::remove_own_comment` | Gỡ contribution ở nhóm unread (`comment.root.approved` / `comment.reply.approved`) theo `P-03A`; nếu là contribution cuối cùng làm count = 0 thì ẩn nhóm unread; không gửi thông báo cho người tự gỡ; Gateway chặn mở đích trực tiếp tới chữ bình luận đã xóa. | Không ảnh hưởng Thấm tác phẩm (Hữu ích bình luận ngoài scope R3). |
| Kiểm duyệt từ chối/gỡ bình luận đã duyệt trước đó | `persistence::discussion::moderate_comment` (`Reject`/`Remove`) | Vô hiệu hóa đóng góp cũ ở nhóm của từng người đã nhận (chủ bài với root, người được trả lời/chủ root với reply); đồng thời phát `comment.moderation.adverse` riêng cho chính tác giả bình luận đó. | Không ảnh hưởng Thấm tác phẩm. |
| Hậu kiểm đưa bình luận đã duyệt về `Pending` | `persistence::post_review::apply` (`ReviewDecision::Hold`) | Đưa `approved → pending` nên bình luận mất quyền công khai; vô hiệu hóa tương tác trên nhóm unread và Gateway lọc bỏ tại đọc/mở đích cho tới khi có kết luận duyệt lại. | Không ảnh hưởng Thấm tác phẩm. |
| Tác giả chuyển bài sang “Chỉ mình tôi” (`PUB-07`) hoặc kiểm duyệt bác bản đang đăng (`O-17@r2`) | `persistence::publication::set_private`, `persistence::moderation::conclude_case`, `persistence::appeals::conclude` | Thu hồi quyền đọc công khai đối với bản bị ẩn/rút qua Gateway (`N4`). Trên trang ẩn/review-withdrawn, thảo luận đã duyệt vẫn có cấu trúc đọc theo `PUB-07`/`O-17@r2`, nhưng inbox/mở đích không bao giờ rò rỉ tiêu đề/thân bài đã ẩn hay bản cũ bị bác. **Thấm trên revision bị bác/ẩn không phát thêm; nhóm unread không lộ preview bài.** | Ngừng mở danh sách Thấm công khai (`404` / unavailable) vì không còn `publications` công khai hiện hành; dữ liệu lịch sử trong `work_endorsements` không bị xóa. |
| Tác giả xóa bài từng công khai (`PUB-04`) hoặc admin gỡ toàn bài (`PUB-05 C`) | `domain::publication` (`PUB-04/05`, runtime xóa chưa triển khai — ghi phụ thuộc) | Với `PUB-05 C` (admin gỡ toàn bài): toàn bộ URL bài và thảo luận trả không tìm thấy; Gateway lọc các tương tác công khai liên quan. Chi tiết quyết định riêng của người viết vẫn kiểm quyền độc lập theo N3/P-12A. Với `PUB-04` (tombstone): bỏ tiêu đề/thân bài/tác giả bài khỏi hydration. | Không còn `publications` hiện hành → đóng đường đọc danh sách Thấm công khai. |
| Đình chỉ principal hoặc thu hồi Handle | `persistence::principal::suspend_principal`, `persistence::handle::retire` | Đình chỉ chặn người nhận dùng inbox qua `require_active`; thu hồi Handle làm Gateway chiếu lại `author` dạng `unavailable`/`deleted`, không rò `@handle` cũ. | Gateway áp phép chiếu `author` hiện hành cho từng dòng trong danh sách Thấm. |
| Xóa tài khoản hai DB (`SOC-DEP-01`) | Phụ thuộc ngoài Account/Identity (`docs/architecture/social-graph.md`) | Đóng mọi nhóm inbox thuộc tài khoản bị xóa; loại danh tính/avatar cũ khỏi `actors_preview` và mọi hydration theo nhãn “Tài khoản đã xóa” (hoặc ẩn đóng góp tùy vòng đời Social); không giữ PII sau xóa. | Dòng Thấm của tài khoản đã xóa (nếu giữ theo policy xóa tài khoản hoặc bị dọn) không bao giờ lộ tên/handle/ảnh cũ. |

### 4.3. Ví dụ Nhập → Xuất cho các ca bắt buộc (`AC-02`)

1. **Ví dụ A — Duyệt bình luận lúc tác phẩm đang ẩn rồi công khai lại (`COM-11`, `PUB-07`):**
   - *Trạng thái đầu:* Bài `W` ở `public` (`epoch = 1`). Độc giả `B` gửi bình luận gốc `C1` (`Pending`). Tác giả `A` chuyển bài sang “Chỉ mình tôi” (`set_private`, bài ở trang ẩn `PUB-07`, `epoch = 1, readable = false`).
   - *Sự kiện 1:* Kiểm duyệt viên duyệt `C1` (`moderate_comment(Approve)`). Lúc commit, `C1` thành `Approved`, hiển thị trên trang ẩn theo `COM-11`, nhưng `publications` không có hàng (`readable = false`).
   - *Kết quả 1:* Producer kiểm `readable == false` → **KHÔNG** ghi sự kiện `comment.root.approved` vào outbox; `A` không nhận badge/thông báo nào.
   - *Sự kiện 2:* `A` công khai lại bài (`request_public` cho đúng bản đã duyệt → `publications` đặt lại, `epoch = 2, readable = true`). Sau đó độc giả `D` gửi `C2` và được duyệt.
   - *Kết quả 2:* Việc công khai lại **KHÔNG** quét hay phát bù thông báo cho `C1` (duyệt lúc ẩn). Chỉ `C2` duyệt ở `(2, true)` mới báo cho `A`. Nếu một sự kiện duyệt khác đã ghi ở `(1, true)` mới được giao sau khi tái công khai, dấu cũ cũng không cho phép báo ở epoch 2.

2. **Ví dụ B — A trả lời reply của B dưới root của C (`P-01B`, một tầng reply phẳng):**
   - Tác phẩm `W` công khai; `C` viết root `R1` (approved); `B` viết reply `R2` (`root_id = R1`, `reply_to_id = None`, approved); nay `A` viết reply `R3` trả lời `R2` (`root_id = R1`, `reply_to_id = Some(R2)`). Khi `R3` được `Approve` trên `W` công khai:

   | Trường hợp vai trò | `reply_to_id` | Người nhận thực tế sau loại self & dedup | Sự kiện nguồn và nhóm theo người nhận | Ghi chú ngữ cảnh |
   | --- | --- | --- | --- | --- |
   | A, B, C đều khác nhau | `Some(R2)` | `[B, C]` | 1 envelope, 2 nhóm theo người nhận, cùng neo `R2` | `B` là người được trả lời trực tiếp; `C` là chủ root theo `P-01B`. |
   | `B == C ≠ A` | `Some(R2)` | `[B]` (đã dedup) | Chỉ 1 thông báo tới `B` | Không gửi đôi cho cùng một `PrincipalId`. |
   | `A == C ≠ B` | `Some(R2)` | `[B]` | Chỉ `B` nhận; `A` bị loại self | Chủ root trả lời B trong chuỗi do chính mình mở. |
   | `A == B ≠ C` | `Some(R2)` | `[C]` | Chỉ `C` nhận; `A` bị loại self | `A` tự trả lời bình luận của chính mình dưới root của `C`. |
   | `A == B == C` | `Some(R2)` | `[]` (rỗng) | **0** thông báo | Tự độc thoại trong chuỗi của chính mình. |
   | Reply trực tiếp root (`A ≠ C`) | `None` | `[C]` | 1 thông báo tới `C` (`group_key` neo `R1`) | Nếu `A == C` thì `[]`. |
   | B đã gỡ chữ của `R2`, cả ba tài khoản vẫn đủ điều kiện | `Some(R2)` | `[B, C]` sau loại self/dedup như trên | 1 envelope, tối đa 2 nhóm neo `R2` | Nguồn vẫn giữ `author_id`. `ReplyContext::Unavailable` che chữ và danh tính ở phần ngữ cảnh. |

   Khi hàng đích đã purge, [khóa ngoại của Discussion](../../services/vink-app/migrations/20261103000000_discussion.sql)
   có thể đặt `reply_to_id = NULL` trong khi `replied_to_reply = true`. Giữ `root_id` để mở chuỗi.
   Dùng neo đích trực tiếp nếu nguồn còn lưu. Nếu đã mất neo, dùng khóa riêng theo reply vừa duyệt,
   ghi ngữ cảnh không khả dụng và chỉ gửi cho người nhận còn xác định được, sau loại self/dedup.
   Trường hợp trả lời trực tiếp root có `replied_to_reply = false`.

3. **Ví dụ C — Tác phẩm duyệt → từ chối → duyệt lại (`P-11a/b`) và ca đối chứng âm:**
   - *Nhập 1:* Revision `rev1` của `W` chuyển `Pending → Approved` bởi AI/Moderator (`decision_version = 2`). → *Xuất:* phát `work.moderation.decided` (`outcome = approved, action_required = false`) tới tác giả `A`.
   - *Nhập 2:* Moderator mở ca can thiệp và bác `rev1` (`Approved → Rejected`, `decision_version = 3`, lý do yêu cầu tác giả sửa bài trước khi gửi lại). → *Xuất:* phát `work.moderation.decided` (`outcome = rejected, action_required = true`) tới `A`, mở màn chi tiết quyết định riêng cho `(W, rev1, 3)`.
   - *Nhập 3:* `A` khiếu nại và Admin duyệt lại `rev1` (`Rejected → Approved`, `decision_version = 4`). → *Xuất:* phát **một** sự kiện `work.moderation.decided` mới (`outcome = approved, action_required = false, decision_version = 4`) tới `A` (`P-11b`), không hồi sinh nhóm cũ đã đọc và không phát lại bình luận/Thấm cũ.
   - *Đối chứng âm (KHÔNG phát):* `NeedsAdmin` hoặc hết retry (tạo case hàng chờ nội bộ, **không** gắn “Cần xử lý” hay gửi tác giả theo `P-11a`); `request_public` trả `Reaffirmed` (giữ nguyên quyết định duyệt sẵn có); `finish_job` trả `Stale` hoặc `Refused(Withdrawn)` sau `set_private`; `conclude_case` lặp lại `Approved → Approved` trên cùng kết luận.

4. **Ví dụ D — Thấm → Same-state fence → Bỏ Thấm → Thấm lại (`P-02B`, `P-03A`):**
   - *Nhập 1:* Độc giả `U1` gửi `Select` trên `(W, rev1)` (công khai, `order 1 → 2, version 1 → 2, EndorsementDelta::Add`). → *Xuất:* phát `work.endorsement.tham.changed (Add)` với `first_selection = true` tới tác giả `A`; inbox tạo/cập nhật nhóm unread (`actor_count = 1, actors_preview = [U1]`), badge tăng 1 nếu trước tín hiệu này nhóm chưa nằm trong số nhóm chưa Seen. Nhóm đã được tính vẫn chỉ đóng góp 1.
   - *Nhập 2:* `U1` gửi lại `Select` hợp lệ ở `order 2 → 3` (same-state fence, `version` vẫn `2`, `EndorsementDelta::None`). → *Xuất:* **KHÔNG** phát sự kiện notification.
   - *Nhập 3:* `U1` bấm `Remove` (`order 3 → 4, version 2 → 3, EndorsementDelta::Remove`). → *Xuất:* phát `work.endorsement.tham.changed (Remove)`. Nếu nhóm của `A` vẫn `unread` và chỉ có `U1`: nhóm giảm về `0` và tự động **ẩn/hết hiệu lực** (`suppressed_empty`, badge giảm 1 nếu trước đó chưa `Seen`); nếu nhóm đã `Read`: giữ lịch sử nhóm đã đọc (không hồi sinh thành unread). Ở Endorsement, `U1` rời khỏi danh sách công khai.
   - *Nhập 4:* `U1` bấm `Select` lại (`order 4 → 5, version 3 → 4, EndorsementDelta::Add`). → *Xuất:* App vẫn phát `work.endorsement.tham.changed (Add)` với `first_selection = false` và `source_order` mới để đồng bộ membership. Notification Service thấy dấu `P-02B` đã từng báo cho `(A, W, rev1, U1)`: **chỉ đối soát âm thầm**, không làm tăng `unseen_group_count`, không đánh thức chuông, không mở nhóm mới từ trạng thái đã `Read` hay `suppressed_empty`. Danh sách công khai của Endorsement (`P-08`) hiển thị lại `U1` ngay lập tức.

---

## 5. Đặc tả UX hành vi bằng chữ — Chuông, Inbox, `Seen`/`Read` và Chi tiết quyết định (`AC-03`)

> **Ranh giới thiết kế:** Phần này là **đặc tả UX bằng chữ** về hành vi người dùng nhìn thấy và trạng thái
> logic; **chưa phải LayoutSpec** (`LAYOUT_SPEC: NOT_CREATED`) và chưa chọn hình dạng nút, bảng màu hay
> component Web/CMP.

### 5.1. Hành trình trọn vòng: Chuông → Mở inbox → Xem chi tiết / Mở đích → Quay lại

1. **Tại thanh điều hướng (Chuông & Badge):**
   - Khách chưa đăng nhập không thấy chuông inbox (hoặc không có dữ liệu inbox).
   - Người dùng đã đăng nhập thấy biểu tượng chuông kèm trạng thái badge của `unseen_group_count`:
     - *Đã có cập nhật mới (`fresh` & `count > 0`):* hiện số nhóm có tín hiệu mới chưa `Seen` (kèm nhãn đọc được cho trình đọc màn hình). Nếu vượt ngưỡng hiển thị gọn (giai đoạn Design chốt ngưỡng cụ thể như `99+`), vẫn phân biệt được với `0`.
     - *Không có cập nhật chưa xem (`fresh` & `count == 0`):* không hiện huy hiệu số mới (hoặc trạng thái tĩnh bình thường).
     - *Dữ liệu cũ quá hạn (`stale`):* vẫn cho thấy số cuối biết được kèm dấu hiệu/chữ cho biết thông tin có thể chưa cập nhật và lối tải lại; **không** tự hạ về `0`.
     - *Không xác minh được (`unavailable`):* hiện trạng thái chưa tải được số đếm và cho phép thử lại; **tuyệt đối không hiện `0`** như thể không có thông báo.
2. **Khi mở chuông (Mở inbox):**
   - Client tải trang inbox đầu tiên từ App Gateway; trong lúc tải hiện trạng thái đang tải rõ ràng (không giả danh sách trống).
   - Ngay khi trang inbox trả về `inbox_snapshot`, client gửi xác nhận `Seen` cho đúng `inbox_snapshot` vừa nhận (idempotent).
   - **Người dùng thấy gì:** Huy hiệu số mới trên chuông xóa/giảm tương ứng với `inbox_snapshot` vừa mở, **nhưng từng dòng thông báo trong danh sách vẫn giữ nguyên trạng thái chưa đọc (`unread`)** cho tới khi người dùng bấm vào dòng đó hoặc bấm “Đọc tất cả”. Các nhóm quá hạn 180 ngày hoặc bị vô hiệu hóa không xuất hiện.
3. **Khi bấm một dòng thông báo:**
   - Hệ thống ghi nhận `Read` cho đúng `(group_id, observed_group_version)` mà người dùng vừa nhìn thấy (độc lập với việc điều hướng tới đích có thành công hay không theo `P-10A`).
   - Phần đã xem chuyển sang đã đọc. Tín hiệu đến sau vẫn hiện là chưa đọc khi tải lại nhóm.
   - Tùy theo loại đích của dòng:
     - *Tương tác (bình luận gốc, phản hồi, Thấm):* điều hướng tới đúng tác phẩm/chuỗi bình luận hoặc mở neo tương ứng.
     - *Quyết định kiểm duyệt (tác phẩm hoặc bình luận):* điều hướng tới **màn chi tiết quyết định riêng** (`P-12A`) đúng `(work_id, revision_id, decision_version)` hoặc `(comment_id, moderation_version)`.
4. **Khi bấm “Đọc tất cả”:**
   - Chỉ áp dụng cho các dòng thuộc tới đúng `inbox_snapshot` đang hiển thị và đã được xác nhận.
   - Các tín hiệu đã có tại mốc đó được xác nhận đã đọc. Tín hiệu mới hơn trong cùng nhóm vẫn chưa đọc.
5. **Khi quay lại từ đích (Back / nút quay lại):**
   - Trở về danh sách inbox (hoặc trang trước khi mở chuông): dòng vừa bấm phản ánh phiên bản đã đọc và các tín hiệu phát sinh sau đó; tiêu điểm bàn phím (focus) và vị trí danh sách được khôi phục an toàn (yêu cầu chi tiết trả focus bàn giao Design/Web). Nếu trong lúc xem đích có sự kiện mới đến sau snapshot trước đó, badge/inbox phản ánh cập nhật mới khi poll/focus kế tiếp.

### 5.2. Người dùng thấy gì trong 6 tình huống biên bắt buộc (`AC-03`)

| Tình huống | Hành vi hệ thống | Người dùng nhìn thấy gì |
| --- | --- | --- |
| **1. Cập nhật đến sau khi inbox đã mở hoặc lúc bấm “Đọc tất cả”** | Người dùng mở snapshot S1 rồi xác nhận S1. Một tín hiệu mới cho cùng nhóm commit tại S2. `Seen(S1)` và “Đọc tất cả” ở S1 chỉ bao phủ tín hiệu đã có tại S1, dù lệnh đến sau S2. | Mở lại inbox vẫn thấy cập nhật S2 chưa đọc. Badge tiếp tục tính nhóm này tới khi người dùng Seen tới S2. |
| **2. Hai tab cùng tài khoản** | Tab 1 thấy G/v1. Tab 1 gửi `Read(G,v1)` sau khi G đã lên v2. Server ghi nhận phần tới v1, kể cả khi lệnh được gửi lại. Nếu v2 chỉ sửa preview, toàn bộ tín hiệu của nhóm đã được đọc. | Khi Tab 2 tải lại, tín hiệu mới ở v2 vẫn chưa đọc. Nếu Tab 2 đã đọc v2, ACK trễ của Tab 1 giữ nguyên kết quả đó. Khi Read tới trước tín hiệu mới, N5 tạo nhóm unread kế tiếp. |
| **3. Mất quyền giữa lúc xem danh sách và lúc mở đích** | Tại thời điểm liệt kê inbox (`S1`), bài/bình luận còn đọc được; ngay sau đó tác phẩm chuyển riêng tư (`PUB-07`), bị bác (`O-17@r2`), bị admin gỡ (`PUB-05 C`), bình luận bị gỡ/hold hoặc người dùng mất quyền. | Khi người dùng bấm dòng: dòng vẫn được ghi nhận `Read` theo `P-10A` (vì người dùng đã chạm tới dòng ở phiên bản thấy được) để không kẹt mãi là unread. Tại màn đích, Gateway kiểm quyền hiện hành (`N4`): hiện trang thông báo hợp lệ tương ứng (ví dụ trang “Nội dung đã được ẩn” / trang thay thế / thông báo nội dung không còn khả dụng) — **tuyệt đối không lộ chữ nội dung hay tên/avatar bị thu hồi**. Khi quay lại hoặc tải lại inbox, nhóm mất quyền tự động bị lọc bỏ (nếu không còn đóng góp hợp lệ nào). |
| **4. Màn chi tiết quyết định riêng của người viết (`P-12A`)** | Người viết bấm thông báo `work.moderation.decided` hoặc `comment.moderation.adverse`. Gateway chỉ mở khi người gọi là chính chủ sở hữu (tác giả bài hoặc người viết bình luận). | **Với tác phẩm:** thấy đúng tựa/phiên bản (`revision_id`), kết quả (`Được duyệt` / `Bị từ chối`), cờ “Cần xử lý” **chỉ khi có hành động bắt buộc** (`P-11a`), lý do chính sách hợp lệ và lối đi tới bước tiếp (sửa bản mới hoặc khiếu nại theo `MOD-10`) — kể cả khi bài đang để riêng tư hoặc bản đó đã bị bác khỏi trang công khai.<br>**Với bình luận bị từ chối hoặc bị gỡ:** thấy kết quả, lý do được phép hiển thị cho người viết và mốc phiên bản; nếu bình luận đã bị gỡ (`Removal::ByModeration`), **không phục hồi thân chữ đã gỡ**, chỉ nêu rõ bình luận đã bị gỡ kèm lý do hợp lệ; **không hiển thị tên người duyệt hay ghi chú nội bộ** (`MOD-12`). Người khác (kể cả tác giả tác phẩm đối với bình luận của độc giả) nhận từ chối không tìm thấy. |
| **5. Bỏ Thấm rồi Thấm lại (`P-02B`, `P-03A`)** | Theo dõi ba trường hợp riêng:<br>(a) Nhóm `unread` chỉ có 1 người `U1` Thấm, `U1` bỏ Thấm → `actor_count` về `0` → nhóm tự ẩn/hết hiệu lực.<br>(b) Nhóm `unread` có 4 người (`U1..U4`), người nằm trong hoặc ngoài 3 preview bỏ Thấm.<br>(c) Nhóm đã `Read`, `U1` bỏ Thấm rồi Thấm lại. | **(a)** Khi `U1` bỏ Thấm trước khi chủ bài mở chuông: số trên chuông giảm lại (nếu chưa Seen) và dòng biến mất sạch (không biến thành dòng đã đọc trống rỗng). Khi `U1` Thấm lại: không xuất hiện thông báo mới và không tăng badge (`P-02B`).<br>**(b)** Người dùng thấy số đếm giảm từ 4 xuống 3 và nếu người bỏ nằm trong 3 tên preview thì người thứ 4 còn hiệu lực được tự động đôn lên thế chỗ trong preview.<br>**(c)** Dòng đã đọc giữ nguyên lịch sử hợp lệ trong hạn 180 ngày (trừ khi toàn bộ nội dung/quyền mất hiệu lực); khi `U1` Thấm lại, **không** báo lại và **không** tạo nhóm mới; chỉ khi có một người hoàn toàn mới `U5` Thấm lần đầu thì mới tạo nhóm unread mới cho `U5`. (Lưu ý: dù không báo lại trong inbox, danh sách người Thấm công khai `P-08` luôn phản ánh đúng việc `U1` đang Thấm.) |
| **6. Hết hạn 180 ngày (`N8`, `P-06A`, `P-13B`)** | Tuổi nhóm tính từ lúc tạo nhóm (`created_at`), cập nhật mới (ví dụ ở ngày 170) không gia hạn mốc ngày 180. Sau 180 ngày nhóm và membership inbox bị xóa. | Sau mốc 180 ngày, nhóm cũ tự biến mất khỏi inbox (cả đã đọc lẫn chưa đọc); nếu nhóm chưa `Seen` hết hạn, số trên chuông tự cập nhật giảm tương ứng. Tương tác mới phát sinh sau khi nhóm cũ đã hết hạn (ngày 181+) tạo một nhóm mới bình thường — **trừ trường hợp người cũ relike Thấm** trên cùng `WorkId + RevisionId`: hệ thống vẫn nhớ dấu `P-02B` nên không báo lại ở inbox. Đồng thời lượt Thấm còn hiệu lực từ ngày 1 vẫn tiếp tục hiện trong **danh sách người Thấm công khai** (`P-08`). |

---

## 6. Đặc tả UX hành vi bằng chữ — Danh sách người Thấm công khai (`AC-04`, `P-08 R3`)

### 6.1. Quyền xem theo vai trò và nguồn dữ liệu

- **Khách chưa đăng nhập:** Khi tác phẩm đang có bản công khai hiện hành (`publications` tại `RevisionId`
  đó), khách thấy tổng số lượt Thấm và có thể mở/tải danh sách đầy đủ những người đang Thấm bản đó theo
  từng trang hữu hạn — **không bị ép đăng nhập**. Khi bài ở chế độ riêng tư, ẩn (`PUB-07`), bị bác
  (`O-17@r2`), bị gỡ hoặc ở revision cũ không còn công khai, danh sách không tồn tại/không đọc được.
- **Người có phiên (`Active`):** Thấy cùng danh sách công khai như khách (tên hiển thị hiện hành,
  `@handle`/liên kết hồ sơ nếu còn hiệu lực, hoặc nhãn trung tính nếu tài khoản đã xóa/không khả dụng),
  tách biệt với nút trạng thái riêng của bản thân (`Đã Thấm` / `Thấm`).
- **Chủ tác phẩm:**
  - Trong **Reader công khai** của bài đang đăng: xem danh sách Thấm công khai giống mọi người đọc
    khác (độc lập việc trong inbox chỉ hiển thị 3 tên preview `actors_preview <= 3`).
  - Trong **Inbox thông báo**: 3 tên là preview tóm tắt của nhóm thông báo đó (`N5`, chịu giới hạn
    180 ngày và `P-02B`); nếu muốn xem tất cả những người đang Thấm hiện tại của bài công khai,
    người dùng mở danh sách Thấm công khai trên tác phẩm (`P-08`).
- **Ràng buộc nguồn dữ liệu (`P-08`):** Danh sách Thấm công khai lấy từ membership hiện hành của
  Endorsement (`work_endorsements`) và phép chiếu danh tính công khai hiện hành; **không** ghép từ 3
  preview inbox, không đọc `suppression` hay `receipt` của Notifications, không lộ email/trường riêng
  tư, không tải toàn danh sách khi polling chuông, và **không triển khai cho Hữu ích bình luận**
  (`REMOVED_FROM_SCOPE`).

### 6.2. Ma trận trạng thái UI bắt buộc cho danh sách Thấm công khai

| Trạng thái | Điều kiện kích hoạt | Người dùng thấy gì & hành động hợp lệ |
| --- | --- | --- |
| **Đang tải (lần đầu)** | Người dùng mở danh sách/popover/panel người Thấm hoặc truy vấn trang đầu tiên. | Hiển thị chỉ báo đang tải rõ tên vùng (ví dụ “Đang tải danh sách người đã Thấm…”), khóa gọi trùng trang đầu, chưa khẳng định danh sách trống hay có 0 người. |
| **Đang tải (tải thêm)** | Người dùng bấm “Xem thêm” ở cuối trang `1..N`. | **Giữ nguyên** các trang đã tải ở trên và vị trí cuộn; chỉ vùng nút tải thêm chuyển sang đang tải để tránh giật trang hoặc mất chỗ đọc. |
| **Trống** | Yêu cầu thành công (`200`), danh sách hiện có 0 người đang Thấm trên revision hiện hành. | Hiện thông điệp trung tính (ví dụ “Chưa có lượt Thấm nào cho bản này.”), không nút “Xem thêm”, phân biệt hẳn với trạng thái lỗi. |
| **Có dữ liệu & Phân trang** | Có từ 1 người trở lên (đặc biệt > 3 người vượt preview inbox). | Hiện đúng các người đang Thấm hiện hành theo thứ tự ổn định, mỗi mục có tên hiển thị + liên kết hồ sơ hợp lệ (hoặc nhãn trung tính nếu tài khoản đã xóa/tạm không khả dụng). Nếu còn trang sau, hiện nút “Xem thêm”; hết trang ẩn/đổi thành trạng thái đã hết danh sách. |
| **Unlike giữa hai lần tải / Đổi revision** | Người ở trang 1 vừa bỏ Thấm trước khi tải trang 2, hoặc tác giả vừa đăng revision mới / ẩn bài giữa lúc xem. | Nếu người ở trang sau bỏ Thấm → không xuất hiện ở trang tiếp theo. Nếu revision hiện hành của bài đổi hoặc bài không còn công khai → server từ chối tiếp tục cursor của revision cũ (`409`/`404`), client bỏ cả các trang và danh tính đã tải cho revision đó, báo danh sách đã thay đổi hoặc bài không còn công khai rồi cho tải lại (không tự trộn người Thấm của revision cũ vào revision mới). |
| **Số đếm cũ / Không xác minh được** | Tổng số trên trang có từ snapshot trước hơi lệch với danh sách vừa tải (do vừa có người Thấm/bỏ Thấm) hoặc đầu đọc lỗi. | Khi có trang mới thành công, hiển thị khớp với thực tế phản hồi mới; nếu yêu cầu đọc lỗi/không xác minh được, **không hiển thị số đếm là `0`** hoặc danh sách là rỗng. |
| **Lỗi & Thử lại** | Lỗi mạng, `503 dependency_unavailable` hoặc timeout ở trang đầu hoặc trang `N+1`. | - *Lỗi trang đầu:* hiện thông báo không tải được danh sách kèm nút “Thử lại”.<br>- *Lỗi khi tải thêm:* giữ nguyên danh sách đã tải, hiện thông báo lỗi ngắn ngay dưới danh sách kèm nút “Thử lại” để gọi lại đúng cursor trang tiếp theo. |
| **Tab ẩn / Offline** | Trình duyệt chuyển nền hoặc mất mạng (`navigator.onLine == false`). | Ngừng mọi tự động tải/polling nền; khi người dùng chủ động bấm mở hoặc tải thêm lúc mất mạng, báo lỗi kết nối rõ ràng và giữ sẵn nút “Thử lại” khi mạng trở lại. |
| **Logout / Đổi tài khoản (`A → B → A`)** | Người dùng đăng xuất hoặc đổi sang tài khoản khác trong khi đang mở bài hoặc mở danh sách Thấm. | Xóa sạch cache, response đang bay và trạng thái riêng tư (`own_state`) của phiên `A`; hủy thao tác ghi đang chờ nếu không còn hợp lệ. Vì danh sách Thấm là dữ liệu **công khai** khi bài đang công khai, danh sách người Thấm vẫn có thể xem tiếp dưới góc nhìn khách hoặc phiên `B` sau khi Gateway xác minh lại quyền đọc đúng revision và client tải trạng thái riêng của phiên mới. |

### 6.3. Yêu cầu thao tác và tiếp cận bàn giao cho giai đoạn Design & Web (`AC-04`)

Các tiêu chí dưới đây là điều kiện nghiệm thu bắt buộc khi lập LayoutSpec và kiểm thử trình duyệt ở đợt
sau (chi tiết trực quan, token và selector thuộc đợt Design):

1. **Thao tác bàn phím & chạm:** Mở danh sách, di chuyển qua từng mục tác giả (link tới `/@handle` nếu có),
   bấm “Xem thêm” / “Thử lại” và đóng danh sách đều thực hiện được hoàn toàn bằng bàn phím (`Tab`, `Shift+Tab`,
   `Enter`, `Space`, `Escape`) lẫn chạm trên màn hình hẹp; mọi nút/vùng bấm đạt sàn hit target của
   UI System (`≥ 44×44 CSS px` theo token `size.hit-target`), không phụ thuộc vào thao tác hover.
2. **Trả focus (`Focus restoration`):**
   - Đóng hộp thoại/sheet danh sách Thấm (bằng nút đóng, bấm ra ngoài hoặc `Escape`) phải trả focus về
     đúng phần tử kích hoạt đã mở nó (nếu phần tử kích hoạt biến mất do đổi phiên/trạng thái thì trả về
     landmark hợp lệ gần nhất, không rơi về `<body>`).
   - Bấm “Xem thêm” ở cuối danh sách khi tải thành công không được làm mất focus về `<body>` (giữ focus
     hoặc chuyển có chủ đích vào mục đầu tiên vừa tải thêm / thông báo cuối trang theo LayoutSpec).
3. **Cỡ chữ 100%–200% & Đa kích thước màn hình (320–1440px):** Bố cục danh sách người Thấm, chuỗi tên
   tiếng Việt có dấu dài, nhãn nút “Xem thêm”, “Đang tải…”, “Thử lại” và hộp thoại/sheet phải tự xuống
   dòng ở độ phóng đại chữ 100% và 200%, không tràn ngang (`scrollX`), không cắt mất dấu thanh tiếng
   Việt, hỗ trợ cả Light/Dark và tôn trọng `prefers-reduced-motion`.

---

## 7. Lựa chọn kỹ thuật mở `T-01..T-08` và phụ thuộc ngoài

Chủ sản phẩm đã giao 8 điểm `TECHNICAL_OPEN` cho các issue kỹ thuật (`#346`, `#347`, `#348`) tự khảo sát
source, đo đạc local và chọn phương án phù hợp mà không mở lại luật sản phẩm `N1–N8` / `P-01..P-14`:

| ID / Lựa chọn | Phương án A / B | Bất biến bắt buộc phải giữ & phép kiểm phân biệt | Issue sở hữu |
| --- | --- | --- | --- |
| **`T-01` / `CHOICE-15`**<br>Producer, envelope & activation boundary | **A:** Mở rộng bảng/nền outbox hiện có ở App (tách giả định comment-only, giữ consumer cũ `admission_shadow.v1` và `operator_post_review.v1`);<br>**B:** Giữ outbox theo từng owner nghiệp vụ rồi chuẩn hóa tại lớp relay. | Mọi transition hợp lệ commit mutation + nghĩa vụ outbox trong **cùng transaction App**; rollback/no-op/stale không tạo event; `recipient` chốt tại nguồn; `activation_epoch` ghim theo commit nguồn (không backfill); payload không chứa body/email/secret/ghi chú nội bộ. | **`#346`** |
| **`T-02` / `CHOICE-16`**<br>Relay & delivery qua 2 process / 2 DB | **A:** Vòng đời relay có I/O riêng trong binary `vink-worker` hiện có;<br>**B:** Binary relay riêng thuộc App owner. | Claim ngắn với `claim_generation` → gửi ngoài transaction App → Notification Service commit nguyên tử `(receipt + hiệu ứng)` → Relay ACK có fence; lost ACK, expired lease, worker cũ ACK trễ và restart không nhân đôi hiệu ứng; cùng `event_id` khác payload bị từ chối/đối soát. | **`#346`** |
| **`T-03` / `CHOICE-17`**<br>Một nhóm unread & concurrency rollup | **A:** Partial unique index cho active-unread `group_key` + upsert/CAS;<br>**B:** Một dòng khóa (`lock row`) theo `group_key` để tuần tự hóa mọi chuyển trạng thái. | Tối đa 1 nhóm unread còn hiệu lực mỗi `group_key` khi nhiều worker cùng ghi, hoặc đua giữa `append` với `Read`/`expire`; không đếm đôi; không hồi sinh nhóm đã `Read`; đo hành vi dưới hot key/skew. | **`#347`** |
| **`T-04` / `CHOICE-18`**<br>Membership, preview & thứ tự `Add`/`Remove` | **A:** Trạng thái theo `(actor, target)` mang `source_order`/`version` + receipt chống trùng;<br>**B:** Ledger delta tối thiểu có hạn + projection membership. | Xử lý `Remove`/`Add` đảo thứ tự (`order` nguồn), no-op, bỏ actor ngoài 3 preview, bỏ actor cuối (`P-03A`); chứng minh suppression `P-02B` suốt vòng đời kể cả sau purge 180 ngày/restore (`P-13B`) mà không đọc App DB. | **`#347`** |
| **`T-05` / `CHOICE-19`**<br>`Seen` watermark, `group_version` & race `Read`/mark-all | **A:** `revision` tăng trong transaction serialize theo recipient (`Seen`/`Read` theo snapshot + `group_version`);<br>**B:** Change journal/checkpoint theo thứ tự commit công bố + ACK group versions. | Không dùng `now()`/`created_at`/UUID/sequence cấp trước commit để chứng minh thứ tự commit; đối soát toggle `P-02B` đổi dữ liệu nhưng không tự tạo `unseen`; kiểm 2 tx lấy ID rồi commit ngược, phản hồi fetch cũ, 2 tab và retry ACK. | **`#347`** |
| **`T-06` / `CHOICE-20`**<br>Batch hydration, quyền & pagination | **A:** Gateway lấy candidate từ Notification, kiểm quyền & hydrate theo lô ở App DB, quét bù có giới hạn tới `limit`;<br>**B:** Duy trì projection eligibility từ invalidation nguồn + Gateway kiểm quyền cuối theo lô. | Không N+1 query; không lọc hết một trang candidate rồi trả nhầm là hết danh sách (`next_cursor` continuation hữu hạn); cursor chịu được rollup đổi vị trí; tách lỗi/unavailable khỏi rỗng; `P-08` list phân trang lấy thẳng từ Endorsement. Phối hợp `#347` chốt hạn badge freshness `P-04B`. | **`#348`** |
| **`T-07` / `CHOICE-21`**<br>Transport & service auth nội bộ (Relay/Gateway ↔ Notification) | **A:** Unix socket xác minh peer UID + caller contract (giống pattern `vink-identity`);<br>**B:** HTTP nội bộ với service token/mTLS và scope rõ. | Loopback/IP riêng không đủ xác thực; thử sai caller/credentials trả `401`/`403`; đo timeout, crash/restart; payload độc lập Web (`P-14A`) để CMP/mobile dùng chung qua Gateway. | **`#347`**<br>(dùng chung bởi `#346`, `#348`) |
| **`T-08` / `CHOICE-22`**<br>Storage growth, retention, backup/restore & query budget | **A:** Bảng/index thường + purge theo lô hữu hạn;<br>**B:** Partition bảng lạnh/lịch sử, vẫn giữ ràng buộc unique cho nhóm active. | Ngăn cross-connect/DDL bằng role runtime; tuổi nhóm ≤ 180 ngày từ lúc tạo (`P-06A`); chọn và công bố hạn `badge freshness`, `late-event cutoff`, `replay/restore horizon` (`P-13B`); diễn tập restore Notification lùi mốc sau App ACK + chứng minh `P-02B` sau purge mà không hồi sinh inbox hết hạn. | **`#347`** |

### Phụ thuộc ngoài phạm vi Epic `#305`

| ID | Đầu vào phụ thuộc | Phần công việc bị chặn / bị giới hạn |
| --- | --- | --- |
| `SOC-DEP-01` | Account/Identity cung cấp xác minh xóa tài khoản, retention và điều phối replay hai DB (`docs/architecture/social-graph.md`). | Nghiệm thu E2E xóa tài khoản và dọn PII hai DB ở thông báo; phần App/Notification vẫn giữ fence cục bộ và đối soát nhãn “Tài khoản đã xóa”. |
| `SOC-DEP-02` | Account/Identity cung cấp capability hạn chế đăng/bình luận tách khỏi đình chỉ toàn phần. | Phân biệt người bị hạn chế viết nhưng vẫn được mở inbox với người bị đình chỉ toàn phần (`principal_suspensions`). |
| `D-05` (`VINK_AUTH_SIGN_IN`) | Mở khóa đăng nhập thật trên môi trường production (hiện `D-05` vẫn `PROVISIONAL`). | Mọi kiểm thử inbox/Seen/Read/quyết định riêng ở môi trường thật cần phiên xác thực thật; trong kiểm thử local dùng Identity scratch/dev. |

---

## 8. Bàn giao sang hợp đồng kỹ thuật, triển khai và thiết kế

1. **Hợp đồng liên dịch vụ & Gateway ngoài Web (`AC-05`):** Xem toàn bộ ngữ nghĩa envelope, receipt/ACK,
   `inbox_snapshot`, `group_version`, `cursor`, mã lỗi, quy tắc polling và **5 bộ ví dụ JSON dùng được
   ngoài Web** tại [Hợp đồng Notifications v1](../../services/vink-app/contracts/http/v1/notifications.md).
2. **Truy vết & điều kiện bắt đầu các issue kế tiếp (`AC-01`, `AC-05`):** Xem bảng ánh xạ `N1–N8`,
   `K1–K6`, `P-01..P-14`, `Epic AC-01..19`, trạng thái code hiện có và danh mục bàn giao sang
   `#346`, `#347`, `#348` cùng danh sách hạng mục chờ LayoutSpec/Web tại
   [Registry phát triển Notifications](../development/notifications.md).
