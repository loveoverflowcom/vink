# Thông báo (Notifications v1) — Truy vết SSOT, hiện trạng và bàn giao `#345`

[PRD §16](../../prd/requirements.md#16-thông-báo-cộng-đồng-và-danh-sách-thấm-công-khai-notifications-v1)
sở hữu các yêu cầu sản phẩm `NOTIF-01..10`.
[Đặc tả miền & UX bằng chữ](../architecture/notifications.md) sở hữu ranh giới miền, Event Inventory
v1 và hành vi người dùng cho chuông, inbox, quyết định riêng và danh sách người **Thấm** công khai.
[Hợp đồng sự kiện & Gateway v1](../../services/vink-app/contracts/http/v1/notifications.md) sở hữu
ngữ nghĩa envelope, receipt/ACK, snapshot/version, cursor, lỗi và bộ ví dụ JSON dùng được ngoài Web.
Tài liệu này sở hữu bảng truy vết `N1–N8`, `K1–K6`, `P-01..P-14`, `Epic AC-01..19`, hiện trạng
khả năng chạy thật trên `develop` và điều kiện bắt đầu cho `#346`, `#347`, `#348` cùng đợt Design/Web.

---

## 1. Snapshot kho quyết định TOML dùng khi bàn giao

Truy vấn trên `develop@1a0c4e315742d887c53ed409b53127a1278794fa` trước khi thêm proposal cho
snapshot `43865e626ee7ce88e8f453140666e10d3b5f3ecf5fe6c52d7a44b6887300fb22`. Sau khi thêm ba
proposal bằng `cargo xtask decision propose`, snapshot mới của kho là
`176802577ad07a5429f8aabadabbe3538e87b4032642532f6ceeb369f6ad0c1d`. Các SHA-256 bên dưới dùng để
rà soát và signoff đúng revision theo [VDR-DECISION-SIGNOFF@r1](../../decisions/signoffs/VDR-DECISION-SIGNOFF.r1.toml);
lượt `#345` **không** chạy `decision signoff` và giữ nguyên lịch sử `@r1`.

| Revision | Trạng thái kho | Hiệu lực hiện hành | SHA-256 payload | File nguồn |
| --- | --- | --- | --- | --- |
| `LVE-PR-02-03-CHOICE-12@r1` | `DESIGN_PROPOSED` | **Current** (đang hiệu lực trong kho) | `a38e54ad78a7a4798b7104b3574115e9d530d5b0517aabcd01821be7f4986223` | [decision-log-20261111.toml](../../decisions/decision-log-20261111.toml) |
| `LVE-PR-02-03-CHOICE-12@r2` | `PENDING` | Chờ signoff (mở danh sách Thấm công khai; Hữu ích ngoài scope `#305`) | `0c0e978ea59408b426554d7ecea3113f22fcbb15d70bb1b6ad96883844cda850` | [LVE-PR-02-03-CHOICE-12.r2.toml](../../decisions/proposals/LVE-PR-02-03-CHOICE-12.r2.toml) |
| `VDR-NOTIFICATIONS@r1` | `PENDING` | Chờ signoff (tổng hợp `N1–N8` & `P-01..P-14` theo `#305` R3) | `c38b3940b1a34b98dc02727dd314be7f10fb7c80b856cb141fb0fa24ae6830dd` | [VDR-NOTIFICATIONS.r1.toml](../../decisions/proposals/VDR-NOTIFICATIONS.r1.toml) |
| `VDR-NOTIFICATIONS-CONTRACT@r1` | `PENDING` | Chờ signoff (hướng `K1–K6` & phân giao `T-01..T-08`) | `4edffa1fd7fbe0bcbcdf171ccd6a992fb85f947bf3d910bdbbcd73696a1a9580` | [VDR-NOTIFICATIONS-CONTRACT.r1.toml](../../decisions/proposals/VDR-NOTIFICATIONS-CONTRACT.r1.toml) |

---

## 2. Hiện trạng khả năng chạy thật trên `develop`

Bảng dưới tách rõ phần nền đã có trên `develop@1a0c4e31` và phần Notifications còn `NOT_IMPLEMENTED`
để các issue kế tiếp bám đúng file sở hữu, không trùng lặp hoặc tưởng nhầm đã có runtime:

| Miền / Hạ tầng | Đã có trên `develop` (chỉ là nền, chưa phát Notifications) | Chưa có (`NOT_IMPLEMENTED` cho Notifications v1) |
| --- | --- | --- |
| **Discussion & Outbox** | `EventKind::CommentSubmitted`, `OperatorReviewRequested` ([outbox.rs](../../services/vink-app/src/domain/discussion/outbox.rs)); `EffectHandler` chạy trong App DB tx không network ([persistence/outbox.rs](../../services/vink-app/src/persistence/outbox.rs)); `vink-worker` ([vink-worker.rs](../../services/vink-app/src/bin/vink-worker.rs)) với 30 ngày retention; commit seams `moderate_comment`, `remove_own_comment` ([discussion.rs](../../services/vink-app/src/persistence/discussion.rs)), `post_review::apply` ([post_review.rs](../../services/vink-app/src/persistence/post_review.rs)); cấu trúc phẳng `CommentPlacement` & `ReplyContext`/`OwnOverlay`. | Producer và outbox/relay cho `comment.root.approved`, `comment.reply.approved`, `comment.moderation.adverse`, invalidation; cơ chế giữ/đồng bộ vượt 2 DB sang Notification Service (`#346`). |
| **Publication & Moderation** | `request_public`, `set_private` ([publication.rs](../../services/vink-app/src/persistence/publication.rs)), `finish_job` ([evaluation.rs](../../services/vink-app/src/persistence/evaluation.rs)), `conclude_case` ([moderation.rs](../../services/vink-app/src/persistence/moderation.rs)), `appeals::conclude` ([appeals.rs](../../services/vink-app/src/persistence/appeals.rs)); pure helper `Publication::notification_stamp` & `should_notify_author` ([domain/publication.rs](../../services/vink-app/src/domain/publication.rs)); trang `review_withdrawn` (`O-17@r2`) và `hidden` (`PUB-07`). | Producer `work.moderation.decided` (`P-11a/b`), đường đọc chi tiết quyết định riêng `P-12A` qua Gateway (`#346`, `#348`); runtime `PUB-04` tác giả xóa bài & `PUB-05 C` admin gỡ toàn bài (vẫn là phụ thuộc vòng đời bài). |
| **Endorsement (Thấm)** | `set_endorsement`, `endorsement_summaries` (tổng số), `own_endorsements` trên `work_endorsements` & `comment_endorsements` ([persistence/endorsement.rs](../../services/vink-app/src/persistence/endorsement.rs), [HTTP v1](../../services/vink-app/contracts/http/v1/README.md#literary-endorsement-lve-73)); pure `decide_endorsement` trả `EndorsementDelta` ([domain/endorsement.rs](../../services/vink-app/src/domain/endorsement.rs)). | Producer `work.endorsement.tham.changed` (chỉ Thấm, bỏ `None`, bỏ Hữu ích — `#346`); endpoint danh sách người Thấm công khai có phân trang cho khách `P-08` (`#348`). |
| **Profile, Follow & Identity** | `identity::Resolution` ([identity/mod.rs](../../services/vink-app/src/identity/mod.rs)); `public_author_attribution` & `profile` ([profile.rs](../../services/vink-app/src/persistence/profile.rs)); Follow runtime `#331` ([follow.rs](../../services/vink-app/src/persistence/follow.rs)); `services/postgres/provision.sh` (chỉ có DB `vink` & `vink_identity`) và `infra/production/backup.sh`. | DB PostgreSQL riêng + role migrate/runtime + binary `vink-notifications` + backup manifest mở rộng (`#347`); giao thức nội bộ xác thực peer/service (`T-07`); `SOC-DEP-01` xóa tài khoản hai DB & `D-05` mở khóa đăng nhập production. |
| **Web / Design / CMP** | Shell thích ứng, Reader `/tac-pham/:id`, Explore `/`, `/toi` (`Auth Web`). | Toàn bộ `LayoutSpec`, chuông/badge, popover/trang inbox, màn chi tiết quyết định riêng, UI danh sách Thấm công khai, bộ điều phối polling 30–60s (`LAYOUT_SPEC: NOT_CREATED`; CMP/native `DEFERRED`). |

---

## 3. Bảng truy vết đầy đủ (`AC-01`)

### 3.1. Truy vết `N1–N8` (Quyết định nghiệp vụ cốt lõi)

| ID | Yêu cầu PRD | Owner | Trạng thái 4 chiều | Issue / Phần để sau |
| --- | --- | --- | --- | --- |
| `N1` | `NOTIF-01` | PRD §16, `notifications.md` §1 & §4.1 | `PRODUCT_CONFIRMED` (brief `#305` R3) · `VDR-NOTIFICATIONS@r1 PENDING` · `NOT_IMPLEMENTED` · `NOT_RUN` | `#346` (5 nhóm phát v1), `#347` (type admin reserved `P-05`). Follow & gửi admin `DEFERRED`; Hữu ích `REMOVED_FROM_SCOPE`. |
| `N2` | `NOTIF-02`, `NOTIF-03` | PRD §16, `notifications.md` §4.1 & §4.3 | `PRODUCT_CONFIRMED` · `VDR-NOTIFICATIONS@r1 PENDING` · `NOT_IMPLEMENTED` · `NOT_RUN` | `#346` (tính recipient tại commit nguồn, loại self/dedup `PrincipalId`). |
| `N3` | `NOTIF-02`, `NOTIF-03` | PRD §16, `notifications.md` §4.1 & §5.2 | `PRODUCT_CONFIRMED` · `VDR-NOTIFICATIONS@r1 PENDING` · `NOT_IMPLEMENTED` · `NOT_RUN` | `#346` (eligibility tại commit, `COM-11`), `#348` (quyền riêng chi tiết quyết định `P-12A`). |
| `N4` | `NOTIF-07` | PRD §16, `notifications.md` §2 & §3, `contracts/.../notifications.md` §2 | `PRODUCT_CONFIRMED` · `VDR-NOTIFICATIONS@r1 PENDING` · `NOT_IMPLEMENTED` · `NOT_RUN` | `#346` (invalidation plumbing), `#348` (BFF kiểm quyền & hydrate theo lô, không rò nội dung/danh tính). |
| `N5` | `NOTIF-04`, `NOTIF-05` | PRD §16, `notifications.md` §4.1 & §5.2, `contracts/.../notifications.md` §2.2 | `PRODUCT_CONFIRMED` · `VDR-NOTIFICATIONS@r1 PENDING` · `NOT_IMPLEMENTED` · `NOT_RUN` | `#347` (Write-time Rollup, duy nhất 1 nhóm unread/`group_key`, `actors_preview <= 3`, `actor_count`), `#348` (tách khỏi full list Thấm `P-08`). |
| `N6` | `NOTIF-06` | PRD §16, `notifications.md` §5.1, `contracts/.../notifications.md` §2.2–2.3 | `PRODUCT_CONFIRMED` · `VDR-NOTIFICATIONS@r1 PENDING` · `NOT_IMPLEMENTED` · `NOT_RUN` | `#347` (Seen watermark vs Read item version), `#348` (API Gateway), Web/Design (`AC-17`). |
| `N7` | `NOTIF-01` | PRD §16, `notifications.md` §1 | `PRODUCT_CONFIRMED` · `VDR-NOTIFICATIONS@r1 PENDING` · `NOT_IMPLEMENTED` · `NOT_RUN` | Mặc định bật mọi loại đã triển khai (`#346`/`#347`); preference UI `DEFERRED`; không bật `admin.notice.reserved`. |
| `N8` | `NOTIF-09` | PRD §16, `notifications.md` §5.2, `contracts/.../notifications.md` §1.2 | `PRODUCT_CONFIRMED` · `VDR-NOTIFICATIONS@r1 PENDING` · `NOT_IMPLEMENTED` · `NOT_RUN` | `#346` (activation epoch, không backfill), `#347` (purge ≤ 180 ngày từ lúc tạo nhóm, receipt horizon). |

### 3.2. Truy vết `K1–K6` (Hướng kiến trúc)

| ID | Owner sở hữu | Trạng thái 4 chiều | Lựa chọn kỹ thuật mở liên quan | Issue phụ trách |
| --- | --- | --- | --- | --- |
| `K1` | `notifications.md` §2, `VDR-NOTIFICATIONS-CONTRACT@r1` | Sản phẩm: N/A · Kiến trúc: `DESIGN_PROPOSED` (`ARCHITECTURE_DIRECTION_CONFIRMED` trong brief, `PENDING` ở TOML) · `NOT_IMPLEMENTED` · `NOT_RUN` | `T-08 / CHOICE-22` | `#347` (DB riêng trên cùng instance PG, role/migration/backup riêng). |
| `K2` | `notifications.md` §2, `VDR-NOTIFICATIONS-CONTRACT@r1` | Như trên | `T-07 / CHOICE-21` | `#347` (process/binary Rust `vink-notifications` độc lập). |
| `K3` | `notifications.md` §2 & §3, `contracts/.../notifications.md` §1 | Như trên | `T-01 / CHOICE-15`, `T-02 / CHOICE-16`, `T-03..05` | `#346` (App outbox/relay) + `#347` (atomic receipt + effect, fenced ACK). |
| `K4` | `notifications.md` §2 & §4, `contracts/.../notifications.md` §1.1 & §2.4 | Như trên | `T-01`, `T-06 / CHOICE-20` | `#346` (minimal event + recipient), `#348` (BFF authz + batch hydration). |
| `K5` | `notifications.md` §2 & §6, `contracts/.../notifications.md` §2.1 | Như trên | `T-06`, `T-07 / CHOICE-21` | `#348` (Gateway-only client, tách inbox có phiên vs public Thấm list Endorsement cho khách). |
| `K6` | `notifications.md` §7, `contracts/.../notifications.md` §2.3 & §2.7 | Như trên | `T-06`, `T-08` | `#348` (count endpoint nhẹ), Design/Web (polling 30–60s khi mở/ẩn/offline). |

### 3.3. Truy vết `P-01..P-14` (`CHOICE-01..14`, cập nhật R3)

| ID / Choice | Kết quả R3 & Delta với `CHOICE-12` | Owner tài liệu | Issue / Phần để sau |
| --- | --- | --- | --- |
| `P-01 / CHOICE-01` | **B (`PRODUCT_CONFIRMED`):** Reply của A vào reply B dưới root C → gửi B+C, loại A, dedup Principal; cấu trúc vẫn 1 tầng phẳng. | PRD `NOTIF-02`, `notifications.md` §4.1 & §4.3 | `#346` |
| `P-02 / CHOICE-02` | **B (`PRODUCT_CONFIRMED`):** Thấm chỉ báo 1 lần suốt vòng đời `(actor, loại, WorkId+RevisionId)`; relike đối soát âm thầm; Hữu ích ngoài scope. | PRD `NOTIF-05`, `notifications.md` §4.1, §4.3, §5.2 | `#346`, `#347` (`T-04`, `T-08`) |
| `P-03 / CHOICE-03` | **A (`PRODUCT_CONFIRMED`):** Nhóm unread giảm count/bù preview, về 0 thì ẩn/hết hiệu lực (không giả `Read`); nhóm đã đọc giữ lịch sử hợp lệ trong retention. | PRD `NOTIF-05`, `notifications.md` §4.1 & §5.2 | `#347` (`T-04`) |
| `P-04 / CHOICE-04` | **B (`PRODUCT_CONFIRMED`):** Cho trễ count để tiết kiệm tài nguyên theo hạn hữu hạn công bố; tách `stale`/`unavailable` với `0`; quyền nội dung vẫn kiểm hiện hành. | PRD `NOTIF-06`, `notifications.md` §5.1, `contracts/.../notifications.md` §2.3 | `#347` + `#348` (`T-06`, `T-08`) |
| `P-05 / CHOICE-05` | **CUSTOM (`TYPE_ONLY_NOW`):** Chỉ dành type quản trị; mọi gửi/soạn/sửa/rút/ủy quyền/broadcast `DEFERRED`. | PRD `NOTIF-01`, `notifications.md` §4.1 | `#345` (contract), `#347` (type enum); gửi thực tế `DEFERRED`. |
| `P-06 / CHOICE-06` | **A (`PRODUCT_CONFIRMED`):** Tuổi nhóm tối đa 180 ngày từ tạo; không gia hạn; không hồi sinh nhóm hết hạn. | PRD `NOTIF-09`, `notifications.md` §5.2 | `#347` (`T-03`, `T-08`) |
| `P-07 / CHOICE-07` | **A (`PRODUCT_CONFIRMED`):** Root theo bài, reply theo đích trực tiếp trong root, Thấm theo `WorkId+RevisionId`, quyết định gắn đúng revision/decision & nghĩa; đếm người khác nhau. Hữu ích ngoài scope R3. | PRD `NOTIF-04`, `notifications.md` §4.1 | `#346`, `#347` |
| `P-08 / CHOICE-08` | **CUSTOM (`PUBLIC_PAGINATED_THAM_ONLY` — R3 delta):**<br>• **P-08a:** Danh sách đầy đủ người đang Thấm theo `WorkId+RevisionId` công khai có phân trang cho mọi người đủ quyền đọc (gồm guest); nguồn từ `work_endorsements`, vượt 3 preview inbox.<br>• **P-08b:** **Hữu ích `REMOVED_FROM_SCOPE`** (không triển khai trong Epic `#305`).<br>• **Delta với `LVE-PR-02-03-CHOICE-12`:** giữ nguyên bản ghi lịch sử `LVE-PR-02-03-CHOICE-12@r1` (`DESIGN_PROPOSED`, marker `active`); đề xuất mới `LVE-PR-02-03-CHOICE-12@r2` (`PENDING`) chỉ mở rộng cho Thấm tác phẩm. | PRD §12 & `NOTIF-08`, `literary-endorsement.md`, `notifications.md` §6, `contracts/.../notifications.md` §2.1 & §2.5, `LVE-PR-02-03-CHOICE-12.r2.toml` | `#348` (API phân trang Endorsement), Design/Web (UI danh sách Thấm). Chờ signoff `CHOICE-12@r2`. |
| `P-09 / CHOICE-09` | **A (`PRODUCT_CONFIRMED`):** Badge đếm nhóm có tín hiệu chưa Seen; mở chuông chỉ ACK Seen tới snapshot. | PRD `NOTIF-06`, `notifications.md` §5.1, `contracts/.../notifications.md` §2.2 | `#347` (`T-05`), `#348`, Design/Web |
| `P-10 / CHOICE-10` | **A (`PRODUCT_CONFIRMED`):** Bấm dòng ghi `Read` phiên bản đã thấy dù mở đích lỗi; “Đọc tất cả” chỉ tới snapshot đã xác nhận. | PRD `NOTIF-06`, `notifications.md` §5.1–5.2, `contracts/.../notifications.md` §2.2 | `#347` (`T-05`), `#348`, Design/Web |
| `P-11 / CHOICE-11` | **CUSTOM (`TWO_AXES`):** P-11a chỉ “Cần xử lý” khi tác giả phải hành động (`NeedsAdmin`/hết retry không phải); P-11b `approved → rejected → approved` phát lượt duyệt mới (`approved → approved`/reaffirmation/no-op không phát). | PRD `NOTIF-03`, `notifications.md` §4.1 & §4.3 | `#346` (producer & eligibility) |
| `P-12 / CHOICE-12` | **A (`PRODUCT_CONFIRMED`):** Màn chi tiết quyết định riêng của người viết theo đúng decision/revision hoặc comment/version; không phục hồi body đã gỡ, không lộ moderator/ghi chú nội bộ. | PRD `NOTIF-03`, `notifications.md` §5.2, `contracts/.../notifications.md` §2.4 | `#348` (endpoint riêng), Design/Web (UI màn chi tiết) |
| `P-13 / CHOICE-13` | **B (`PRODUCT_CONFIRMED`):** Xóa inbox và danh sách người tương tác của Notifications sau tối đa 180 ngày; receipt/suppression kỹ thuật có hạn độc lập để bảo vệ `P-02B`/khôi phục nhưng không tái dựng inbox đã purge và không xóa lượt Thấm còn hiệu lực ở Endorsement. | PRD `NOTIF-09`, `notifications.md` §5.2, `contracts/.../notifications.md` §1.2–1.3 | `#346`, `#347` (`T-04`, `T-08`) |
| `P-14 / CHOICE-14` | **A + API dùng lại (`PRODUCT_CONFIRMED`):** Web trước; API/model/đích nghiệp vụ dùng lại cho CMP/mobile; tách public Thấm list cho khách với inbox có phiên. | PRD `NOTIF-10`, `contracts/.../notifications.md` §2 & §3 | `#348` (API & fixture ngoài Web), Design/Web (Web UI); CMP/native UI `DEFERRED`. |

### 3.4. Truy vết `Epic #305 AC-01..AC-19`

| Epic AC | Nội dung tóm tắt | Issue phụ trách thực hiện & kiểm chứng | Boundary mục tiêu | Trạng thái tại `#345` |
| --- | --- | --- | --- | --- |
| `AC-01` | Root comment được duyệt → chỉ tác giả bài, loại self, không leak pending/rejected, giữ `COM-11`. | `#346` | Pure + PostgreSQL thật | `NOT_RUN` (chỉ có inventory & ví dụ ở `#345`) |
| `AC-02` | Approved reply → `P-01B` (`B+C`, 1 tầng phẳng, loại self, dedup Principal). | `#346` | Pure + PostgreSQL thật | `NOT_RUN` (chỉ có ma trận 7 ca ở `#345`) |
| `AC-03` | 1 nhóm unread/`group_key`; sau Read tạo nhóm mới; race Read/expire/write. | `#347` | PG thật (`vink_notifications`) + race test | `NOT_RUN` |
| `AC-04` | Concurrent writes, retry, lost ACK, reorder, duplicate ID, payload mismatch. | `#346` + `#347` | Hai process + hai DB PG thật | `NOT_RUN` |
| `AC-05` | `P-02B` suppression suốt đời & `P-03A` bỏ Thấm giảm/ẩn unread, relike im lặng. | `#346` + `#347` | Pure + hai DB PG thật + clock purge | `NOT_RUN` |
| `AC-06` | `P-09A` badge/Seen, `P-10A` click Read dù lỗi đích, mark-all snapshot, 2 tab. | `#347` + `#348` (+ Web AC-17) | PG + HTTP Gateway + Web | `NOT_RUN` |
| `AC-07` | Ẩn/gỡ/hold/mất quyền không rò rỉ; `P-04B` `stale`/`unavailable` ≠ `0`; `N3` quyết định riêng. | `#348` (+ `#346` invalidation) | HTTP Gateway + PG thật | `NOT_RUN` |
| `AC-08` | Batch authz/hydration không N+1; pagination ổn định; continuation hữu hạn. | `#348` | Query count / EXPLAIN + HTTP/PG | `NOT_RUN` |
| `AC-09` | Enqueue lỗi rollback tx App; sau commit, Notification/Relay chết không đảo nghiệp vụ. | `#346` + `#347` | Fault injection hai process + PG thật | `NOT_RUN` |
| `AC-10` | DB/service/role tách biệt, negative cross-connect/DDL, service auth `T-07`, không PII trong log. | `#347` (+ `#346`, `#348`) | `provision.sh` + credential thật + log scan | `NOT_RUN` |
| `AC-11` | Inventory đủ 5 nhóm runtime + invalidation + `P-05` reserved; chỉ Thấm, bỏ Hữu ích. | **`#345`** (tài liệu/contract) + `#346` (kiểm producer thật) | Tài liệu/contract (**PASS ở `#345`**) · Code (`#346`: `NOT_RUN`) | Phần tài liệu/hợp đồng xong ở `#345`; producer thật chờ `#346`. |
| `AC-12` | `P-11a` “Cần xử lý”, `P-11b` re-approval, `P-12A` chi tiết quyết định riêng. | `#346` + `#348` (+ Web) | Pure + PG + HTTP Gateway | `NOT_RUN` |
| `AC-13` | `P-06A` tuổi 180 ngày không gia hạn, `P-13B` purge & receipt horizon, không backfill. | `#346` + `#347` | Controlled clock + PG + restore test | `NOT_RUN` |
| `AC-14` | `P-05` định nghĩa type quản trị dành trước (`TYPE_ONLY_NOW`); gửi thực tế `DEFERRED`. | **`#345`** (contract) + `#347` (type enum) | Contract (**PASS ở `#345`**) · Type enum (`#347`: `NOT_RUN`) | Phần hợp đồng xong ở `#345`; enum code chờ `#347`. |
| `AC-15` | Diễn tập migration/backout, backup/restore lệch mốc App/Notification, query/storage budget. | `#347` (+ `#346` replay) | Bản sao DB thật + `pg_dump`/`pg_restore` + EXPLAIN | `NOT_RUN` |
| `AC-16` | **SSOT/PRD/TOML/spec/inventory/registry trước code (`#345`);** LayoutSpec (`Design`). | **`#345`** (SSOT & UX bằng chữ) + Design sau | Repo checks + `decision validate` (**PASS phần SSOT ở `#345`**); LayoutSpec `NOT_CREATED` | Phần SSOT của `#345` hoàn tất; phần `LayoutSpec` chờ giai đoạn Design. |
| `AC-17` | Web qua Gateway, polling 30–60s, ẩn/offline, logout/đổi tài khoản, phím/chạm/focus/100–200%. | `#348` (tiền đề BE & handoff polling) + Design/Web | Chrome headless (`verify web`) | `NOT_RUN` (chờ Design/Web; đã mô tả yêu cầu ở `#345`). |
| `AC-18` | `P-08` R3 danh sách người Thấm công khai phân trang cho khách/user/chủ bài, độc lập 180d inbox. | **`#345`** (spec/contract/TOML `PENDING`) + `#348` (API) + Design/Web | HTTP/PG (`#348`) + Browser (Design/Web) | Đặc tả & contract xong ở `#345`; API & UI `NOT_RUN`. |
| `AC-19` | `P-14A` API/model/fixture dùng lại cho CMP/mobile ngoài Web; tách public vs phiên. | **`#345`** (ví dụ hợp đồng) + `#348` (fixture HTTP ngoài Web) | Non-Web HTTP client test (`#348`) | Hợp đồng & 5 bộ ví dụ JSON xong ở `#345`; fixture chạy thật chờ `#348`; CMP UI `DEFERRED`. |

---

## 4. Điều kiện bắt đầu và bàn giao sang 3 issue Backend (`#346`, `#347`, `#348`)

Khi hợp đồng ở `#345` được đưa vào nhánh, ba nhánh backend bắt đầu theo thứ tự phụ thuộc sau:

| Issue & Nhánh đề xuất | Phạm vi sở hữu | Lựa chọn `T-*` phải chốt | Đầu vào từ `#345` & Điểm ghép hai đầu | Lane kiểm chứng mục tiêu |
| --- | --- | --- | --- | --- |
| **[#346 · Thông báo 2/4](https://github.com/loveoverflowcom/vink/issues/346)**<br>`feat/346-notification-producers-relay` | `services/vink-app`: producer ở 5 nhóm commit seams (`discussion`, `evaluation`, `moderation`, `appeals`, `endorsement`), invalidation plumbing, activation cutoff và Relay + ACK fenced. | **`T-01 / CHOICE-15`**<br>**`T-02 / CHOICE-16`** | Đọc Inventory §4 (`notifications.md`) và Envelope/Receipt/Fenced ACK §1 (`contracts/.../notifications.md`). Có thể dựng bằng receiver fixture; **phối hợp `#347`** chốt giao thức `T-07` và diễn tập replay sau restore. | `cargo xtask verify quick` + `cargo xtask verify postgres` (commit nguyên tử, rollback, fault injection); ghép 2 process thật với `#347`. |
| **[#347 · Thông báo 3/4](https://github.com/loveoverflowcom/vink/issues/347)**<br>`feat/347-notification-service-inbox` | Process `vink-notifications`, DB PostgreSQL + role + `provision.sh` + `backup.sh`, ingestion receipt + Write-time Rollup, `P-02B`/`P-03A`, `Seen`/`Read`, `admin.notice.reserved` type-only, retention 180 ngày & diễn tập restore. | **`T-03 / CHOICE-17`**<br>**`T-04 / CHOICE-18`**<br>**`T-05 / CHOICE-19`**<br>**`T-07 / CHOICE-21`**<br>**`T-08 / CHOICE-22`** | Chốt `T-07` cho cả `#346` và `#348` dùng; công bố `max_staleness_seconds` (`P-04B`), `late_event_cutoff`, `recovery_horizon`/RPO (`P-13B`). Độc lập phát triển bằng fixture từ `#345`, rồi ghép `#346` (mất ACK/restore) và `#348` (inbox/ACK). | `cargo xtask verify quick` + `cargo xtask verify postgres` + diễn tập `provision.sh`, role âm, 2 process/2 DB, clock 180 ngày, `pg_dump`/`pg_restore` lệch mốc. |
| **[#348 · Thông báo 4/4](https://github.com/loveoverflowcom/vink/issues/348)**<br>`feat/348-notification-gateway-reads` | App Gateway `/api/v1/*`: badge freshness, trang inbox, `Seen`/`Read`/`MarkAllRead`, màn chi tiết quyết định riêng (`P-12A`), **endpoint danh sách Thấm công khai có phân trang (`P-08 R3`)**, batch authz/hydration & fixtures ngoài Web. | **`T-06 / CHOICE-20`**<br>(phối hợp `#347` về badge freshness và dùng `T-07` của service) | **Có thể làm ngay sau `#345`:** hợp đồng & endpoint danh sách Thấm công khai (`work_endorsements` + `public_author_attribution`), endpoint chi tiết quyết định riêng (`P-12A`) và fixtures HTTP ngoài Web. Ghép inbox thật khi `#347` sẵn sàng; kiểm E2E từ nghiệp vụ thật cùng `#346` + `#347`. | `cargo xtask verify quick` + `cargo xtask verify postgres` (session, origin, standing fence, query count / `EXPLAIN (ANALYZE, BUFFERS)` có skew, HTTP client ngoài Leptos). |

---

## 5. Bàn giao riêng cho giai đoạn Design & Web (`Epic AC-16` & `AC-17`)

Sau khi `#345` hoàn tất đặc tả bằng chữ và `#348` cung cấp API/fixtures, giai đoạn Design/Web cần
thực hiện đủ các mục sau (**hiện toàn bộ ở trạng thái `NOT_CREATED` / `NOT_RUN`**):

1. **Tài liệu LayoutSpec (`vink-layout-spec`, `vink-ui-design` — phần còn lại của `Epic AC-16`):**
   - Bề mặt chuông & badge trên desktop rail và mobile bottom bar (`fresh`, `stale`, `unavailable` ≠ `0`,
     vùng chạm ≥ 44×44 px).
   - Popover/sheet/trang Inbox: dòng gom nhóm (`actors_preview <= 3`), phân biệt `unread`/`read`, hành
     động “Đọc tất cả”, nút tải tiếp, trạng thái trống/đang tải/lỗi/bị thu hồi quyền.
   - Màn chi tiết quyết định riêng (`P-12A`): tách rõ nhãn “Cần xử lý” (`P-11a`), ẩn thân bình luận đã
     gỡ (`comment_body: null`), giữ lối sửa bài / khiếu nại (`MOD-10`).
   - Bề mặt danh sách người Thấm công khai (`P-08 R3`) trên Reader: từ tóm tắt số lượt Thấm mở danh
     sách phân trang cho khách lẫn người đăng nhập, trạng thái “Xem thêm”, `unlike` giữa trang,
     `409 revision_changed`, trống/lỗi/thử lại.
2. **Triển khai Web & Kiểm thử Chrome headless (`vink-leptos-web` — `Epic AC-17`):**
   - Bộ điều phối polling 30–60 giây cho badge count nhẹ, tạm dừng khi ẩn/offline, backoff/resume, hủy
     phản hồi muộn khi đăng xuất hoặc đổi tài khoản (`A → B → A`).
   - Kịch bản trình duyệt ở 320/390/834/1024/1440 px, Light/Dark, chữ 100%–200%: phím `Tab`/`Enter`/`Space`/`Escape`,
     trả focus về phần tử kích hoạt sau khi đóng sheet/hộp thoại và không rớt focus khi bấm “Xem thêm”,
     SSR khách đọc danh sách Thấm công khai, kiểm logout/đổi tài khoản không rò cache.
3. **Phạm vi giữ ở `DEFERRED`:** CMP/native UI (`P-14A` chỉ yêu cầu API dùng lại được ở `#348`,
   không dựng app native trong Epic `#305`), kênh gửi thông báo quản trị (`P-05`) và Follow Notifications.
