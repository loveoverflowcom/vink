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
| **Discussion & Outbox** | `EventKind::CommentSubmitted`, `OperatorReviewRequested` ([outbox.rs](../../services/vink-app/src/domain/discussion/outbox.rs)); `EffectHandler` chạy trong App DB tx không network ([persistence/outbox.rs](../../services/vink-app/src/persistence/outbox.rs)); `vink-worker` ([vink-worker.rs](../../services/vink-app/src/bin/vink-worker.rs)) với 30 ngày retention; giao dịch `moderate_comment` ghi lịch sử `comment_moderation_decisions` theo `(comment_id, version)`, còn `remove_own_comment` xóa thân bình luận ([discussion.rs](../../services/vink-app/src/persistence/discussion.rs)), `post_review::apply` ([post_review.rs](../../services/vink-app/src/persistence/post_review.rs)); cấu trúc phẳng `CommentPlacement` & `ReplyContext`/`OwnOverlay`. | Producer và outbox/relay cho `comment.root.approved`, `comment.reply.approved`, `comment.moderation.adverse`, invalidation; cơ chế giữ/đồng bộ vượt 2 DB sang Notification Service (`#346`). |
| **Publication & Moderation** | `request_public`, `set_private` ([publication.rs](../../services/vink-app/src/persistence/publication.rs)), `finish_job` ([evaluation.rs](../../services/vink-app/src/persistence/evaluation.rs)), `conclude_case` ([moderation.rs](../../services/vink-app/src/persistence/moderation.rs)), `appeals::conclude` ([appeals.rs](../../services/vink-app/src/persistence/appeals.rs)); [lịch sử quyết định theo revision](../../services/vink-app/src/persistence/moderation_write.rs) và điều kiện thông báo đúng đợt công khai trong `Publication::should_notify_author` ([domain/publication.rs](../../services/vink-app/src/domain/publication.rs)); trang `review_withdrawn` (`O-17@r2`) và `hidden` (`PUB-07`). | Producer `work.moderation.decided` (`P-11a/b`), đường đọc chi tiết quyết định riêng `P-12A` qua Gateway (`#346`, `#348`); runtime `PUB-04` tác giả xóa bài & `PUB-05 C` admin gỡ toàn bài (vẫn là phụ thuộc vòng đời bài). |
| **Endorsement (Thấm)** | `set_endorsement`, `endorsement_summaries` (tổng số), `own_endorsements` trên `work_endorsements` & `comment_endorsements` ([persistence/endorsement.rs](../../services/vink-app/src/persistence/endorsement.rs), [HTTP v1](../../services/vink-app/contracts/http/v1/README.md#literary-endorsement-lve-73)); pure `decide_endorsement` trả `EndorsementDelta` ([domain/endorsement.rs](../../services/vink-app/src/domain/endorsement.rs)). | Producer `work.endorsement.tham.changed` (chỉ Thấm, bỏ `None`, bỏ Hữu ích — `#346`); endpoint danh sách người Thấm công khai có phân trang cho khách `P-08` (`#348`). |
| **Profile, Follow & Identity** | `identity::Resolution` ([identity/mod.rs](../../services/vink-app/src/identity/mod.rs)), [view public_author_attribution](../../services/vink-app/migrations/20261125010000_public_author_attribution.sql) và [phép chiếu AuthorAttribution](../../services/vink-app/src/application/profile.rs). [PR #331](https://github.com/loveoverflowcom/vink/pull/331) đã triển khai Follow của #294. Thông báo Follow vẫn `DEFERRED`. `services/postgres/provision.sh` (chỉ có DB `vink` & `vink_identity`) và `infra/production/backup.sh`. | DB PostgreSQL riêng + role migrate/runtime + binary `vink-notifications` + backup manifest mở rộng (`#347`, phần đầu đã có, xem §7); giao thức nội bộ xác thực peer/service (`T-07`, §7.1); `SOC-DEP-01` xóa tài khoản hai DB & `D-05` mở khóa đăng nhập production. |
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
| `P-05 / CHOICE-05` | **CUSTOM (`TYPE_ONLY_NOW`):** V1 dành trước type quản trị. Hướng về sau chỉ cho chính chủ sản phẩm có quyền admin gửi tới tập người nhận được chỉ định. Luồng gửi/soạn/sửa/rút/ủy quyền/broadcast vẫn `DEFERRED`. | PRD `NOTIF-01`, `notifications.md` §4.1 | `#345` (contract), `#347` (type enum); gửi thực tế `DEFERRED`. |
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
   - Chọn vị trí chuông và badge trên desktop/mobile, thể hiện từng trạng thái `fresh`, `stale`, `unavailable` và số 0 đã xác minh.
     Vùng chạm ≥ 44×44 CSS px.
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

---

## 6. Hiện trạng `#346`: producer, outbox và relay phía App

Nhánh `feat/346-notification-producers-relay` (gốc `develop@ab5ecaf`). Bốn chiều trạng thái:

| Chiều | Ghi nhận |
| --- | --- |
| Sản phẩm | `PRODUCT_CONFIRMED` trong brief `#305` R3; `VDR-NOTIFICATIONS@r1` vẫn `PENDING`. |
| Kiến trúc | `T-01 / CHOICE-15 = A` và `T-02 / CHOICE-16 = A` là lựa chọn kỹ thuật của `#346` (§6.1–6.2), mức `DESIGN_PROPOSED`; không làm `VDR-NOTIFICATIONS-CONTRACT@r1` thành `ACCEPTED`. |
| Triển khai | `persistence-api`: producer trong giao dịch nguồn, outbox `notification_events`/`notification_deliveries`, kích hoạt theo epoch, relay ACK có fence (thư viện `vink_app::notification_relay`), lệnh vận hành `vink-app notifications …`. Relay chạy trong `vink-worker` khi cấu hình socket ingest của `#347` (§7; không cấu hình thì log `notification_relay = "no_transport"`). |
| Bằng chứng | Unit domain (`domain::notification::tests`, 12 ca, gồm hai digest ví dụ của hợp đồng §3 khớp từng byte) và PostgreSQL thật (`tests/postgres/notification.rs`, 15 ca) với receiver fixture. Hai process/hai DB: `NOT_RUN` (chờ `#347`). |

### 6.1. `T-01 / CHOICE-15`: chọn A — mở rộng nền outbox hiện có

Producer dùng chung vòng đời thuần của outbox Discussion (`domain::discussion::outbox`: claim
generation là fence, lease chỉ là đồng hồ sống, backoff có trần, hạn chót, requeue chỉ cho dead
letter) nhưng ghi vào bảng riêng `notification_events` + `notification_deliveries` của App.
Không sửa `discussion_events` vì bảng đó buộc `comment_id NOT NULL` + FK và `EffectHandler` chạy
trong giao dịch, cấm mạng; đổi giả định đó phải kiểm lại hai consumer `admission_shadow.v1` và
`operator_post_review.v1`. Phương án B (outbox từng owner) phải đối soát bốn nguồn. Đối chiếu bằng:

- **Rollback/no-op:** `emit` chạy trong giao dịch nguồn sau mọi lock của path; lỗi enqueue làm
  rollback mutation (`an_enqueue_failure_rolls_the_decision_back`); chuyển trạng thái không đổi,
  stale hay replay không ghi gì (unit + PG).
- **Cutover tại commit:** `notification_activation` (một dòng). Producer đọc `FOR KEY SHARE`, lệnh
  `activate|deactivate` khóa `FOR UPDATE`, nên mỗi giao dịch commit trọn trước hoặc trọn sau mốc.
  Bảng khởi tạo ở trạng thái tắt; mỗi lần bật mở epoch mới; không backfill. Trên schema trước
  migration `20261127000000` (chỉ gặp ở test dựng schema theo mốc), thiếu bảng được coi là "chưa
  từng bật".

Mỗi sự kiện lưu envelope chuẩn hóa (không có `payload_digest`) và SHA-256 của nó, ghi một lần ở
producer; mọi lần gửi và replay dùng đúng chuỗi đó. `dedup_key` (loại, phạm vi, `source_order`, và
actor với Thấm) bảo đảm một nghĩa vụ cho một chuyển trạng thái.

### 6.2. `T-02 / CHOICE-16`: chọn A — relay là vòng lặp của worker App

`NotificationRelay::run_once`: claim ngắn (`FOR UPDATE SKIP LOCKED`, generation + 1, lease) rồi
commit → gửi một lô qua port `NotificationSink` ngoài mọi giao dịch App, timeout ngắn hơn lease →
ghi từng kết quả trong giao dịch khóa delivery và kiểm generation. `applied`/`already_applied` →
`succeeded`; `payload_mismatch`, `rejected_invalid` → dead letter chờ vận hành;
`transient_unavailable`, lỗi transport, timeout, thiếu kết quả → retry có backoff, hết lượt/hạn →
dead letter. Một lời gọi mỗi relay, tối đa 100 sự kiện; nhiều process an toàn nhờ `SKIP LOCKED`
và fence. Dừng: ngừng claim, chờ `drain`, lô bỏ dở không ghi gì và lease tự hết hạn. Chọn A vì
cùng pool/giám sát với outbox hiện có; tách binary (B) khi `#347` đo được backlog/restart cần cô lập.

### 6.3. Điểm phát trong giao dịch nguồn

| Sự kiện | Seam | Quy tắc đã cài |
| --- | --- | --- |
| `comment.root.approved` / `comment.reply.approved` | `persistence::discussion::moderate_comment` (Approve) | Khóa `publications FOR SHARE` sau khóa bình luận, trước standing fence; chỉ phát khi non-public → public và bài đọc được tại commit. Root → chủ bài; reply → tác giả đích trực tiếp (nếu còn hàng) + tác giả root; loại actor, dedup, sắp UUID tăng dần. |
| `comment.moderation.adverse` | `moderate_comment` (Reject/Remove) | Gửi người viết, `actor_id = null`, `action_required = false`; Rejected → Rejected hay gỡ lặp không phát. |
| `work.moderation.decided` | `moderation_write::write_decision` (mọi đường: `finish_job`, `conclude_case`, `appeals::conclude`, `request_public`) | Chỉ khi kết luận mới `approved`/`rejected` khác kết luận gần nhất của revision (hàng hiện tại + lịch sử). `action_required = true` chỉ với `rejected`. Không lộ người duyệt hay lý do. |
| `work.endorsement.tham.changed` | `persistence::endorsement::set_endorsement` (Work, `Add`/`Remove`) | `source_order = accepted_order`; `first_selection = (version sau ghi == 2)`; `None`, replay, refusal và Hữu ích không phát. |
| `notification.invalidation` (`comment`) | `moderate_comment` (gỡ/từ chối bình luận đã công khai), `remove_own_comment`, `post_review::apply` (Hold) | Không có người nhận; `source_order = moderation_version` mới. |
| `notification.invalidation` (`work`) | `moderation_write::apply_pointer_effect` (Clear), `publication::set_private` | Khi thật sự xóa con trỏ công khai; `source_order` = epoch vừa kết thúc. |
| `notification.invalidation` (`principal`) | `suspend_principal`, `reinstate_principal` | `source_order` = số lần đình chỉ + số lần gỡ đình chỉ. |

### 6.4. Vận hành

`vink-app notifications status | activate <ghi chú> | deactivate <ghi chú> | dead [<limit>] |
requeue <event-uuid> <ghi chú> | replay <since-rfc3339> <ghi chú>`. Kích hoạt, requeue và replay
đều có audit append-only. **Replay horizon** = 30 ngày (`REPLAY_HORIZON_DAYS`): `vink-worker` và
`vink-app maintenance prune` chỉ xóa sự kiện đã ACK quá hạn này; sự kiện còn `ready`/`leased`/`dead`
được giữ. Chỉ bật kích hoạt khi receiver `#347` sẵn sàng. Backout:
[`down.sql`](../../services/vink-app/migrations-rollback/20261127000000_notification_outbox.down.sql)
từ chối khi còn delivery chưa xong.

### 6.5. Tiêu chí của `#346` theo bằng chứng

| AC | Kết quả ở ranh giới App | Còn chờ |
| --- | --- | --- |
| AC-01 | PASS (unit + PG): root chỉ báo chủ bài, loại self; bảng 7 ca §4.3 B; pending/submitted không phát; duyệt lúc bài không đọc được không phát và tái công khai không bù. | Ca "ẩn" ở PG mô phỏng bằng xóa con trỏ trong SQL; `set_private` thật được dùng ở luồng quyết định tác phẩm nhưng chưa kết hợp với duyệt bình luận. |
| AC-02 | PASS (unit + PG): adverse đúng người viết; P-11 qua evaluator thật + ca người duyệt (approved → rejected, `action_required`); Thấm add/remove/relike với `first_selection`; stale/không đổi không phát. | `Rejected → Approved` qua appeal chỉ có unit; đối soát âm thầm P-02B ở bên nhận (`#347`). |
| AC-03 | PARTIAL: invalidation cho ẩn/rút/gỡ/hold/đình chỉ; envelope chỉ ID/version/mã (kiểm khóa và chuỗi cấm); cutover theo epoch; type quản trị không có producer. | Cutoff sự kiện trễ và horizon ở bên nhận (`#347`); `PUB-04/05` xóa bài chưa có runtime nên chưa có producer. |
| AC-04 | PASS (PG): ép lỗi enqueue làm rollback quyết định; nghĩa vụ còn nguyên khi relay dừng và được giao sau restart. | Notification Service dừng thật (`#347`). |
| AC-05 | PASS với receiver fixture: mất ACK, lease hết hạn + ACK trễ của worker cũ, retry/backoff/dead letter, requeue, payload mismatch, replay sau restore. | Hai process/hai DB, transport `T-07`, diễn tập restore lệch mốc với `#347`. |

Lệnh đã chạy (PostgreSQL 17.11 container tạm): `cargo test -p vink-app --lib domain::notification`,
`cargo test -p vink-app --test postgres` (toàn suite). `cargo xtask verify quick` được chạy trước khi
mở PR; CI không chờ theo yêu cầu của chủ dự án.

---

## 7. Hiện trạng `#347`: Notification Service

Phần đầu ở [PR #373](https://github.com/loveoverflowcom/vink/pull/373) (service, DB, transport,
rollup); phần hai trên nhánh `feat/347-notification-recovery` thêm cổng recovery, runbook khôi phục,
diễn tập AC-06 và số đo ngân sách (§7.3, §7.7). Các mục ở §7.6 vẫn mở. Bốn chiều trạng thái:

| Chiều | Ghi nhận |
| --- | --- |
| Sản phẩm | `PRODUCT_CONFIRMED` trong brief `#305` R3; `VDR-NOTIFICATIONS@r1` vẫn `PENDING`. |
| Kiến trúc | `T-07 = A`, `T-03 = B`, `T-04 = A`, `T-05 = A`, `T-08 = A` (§7.1) là lựa chọn kỹ thuật của `#347`, mức `DESIGN_PROPOSED`; không làm `VDR-NOTIFICATIONS-CONTRACT@r1` thành `ACCEPTED`. |
| Triển khai | `persistence-api` + `worker-provider` cục bộ: crate/binary [`vink-notifications`](../../services/vink-notifications/README.md), DB `vink_notifications` với role migrator/runtime trong [`provision.sh`](../../services/postgres/provision.sh), dump thứ ba trong [`backup.sh`](../../infra/production/backup.sh), [hợp đồng socket v1](../../services/vink-notifications/contracts/rpc/v1/README.md), sink socket của relay ([`notification_transport.rs`](../../services/vink-app/src/notification_transport.rs)) mà `vink-worker` chạy khi có `VINK_NOTIFICATIONS_INGEST_SOCKET` + `VINK_NOTIFICATIONS_SERVER_UID` (log `notification_relay = "socket"`, không thì vẫn `"no_transport"`). Chưa có image/service trong compose production, chưa có client Gateway (`#348`). |
| Bằng chứng | §7.5. |

### 7.1. Lựa chọn kỹ thuật

- **`T-07 / CHOICE-21 = A` (Unix socket xác minh peer).** Cùng mẫu đã chạy với Identity: HTTP/1.1
  trên Unix socket, service kiểm OS peer user (`SO_PEERCRED`) của từng kết nối, App kiểm ngược UID
  của tiến trình phục vụ. Hai socket cho hai phạm vi (ingest cho relay, inbox cho Gateway); route
  của phạm vi này không tồn tại trên socket kia. Production chạy mọi service cùng UID, nên phạm vi
  được tách bằng mount: chỉ worker thấy socket ingest, chỉ app thấy socket inbox. Phương án B (HTTP
  nội bộ có token/scope) cần thêm secret, rotation và TLS nội bộ mà topology một host hiện tại chưa
  đòi hỏi; chuyển sang B khi service tách host.
- **`T-03 / CHOICE-17 = B` (serialize trên dòng khóa kèm constraint).** Mọi thay đổi của một hộp thư
  (hiệu ứng ingest, Read, Seen, MarkAllRead, đóng nhóm hết hạn) khóa `inboxes(recipient)` trước; partial
  unique index `notification_groups_one_open` giữ tối đa một nhóm mở cho mỗi `group_key` như một
  constraint. Khóa theo người nhận (thô hơn `group_key`) vì Read/expire/append và version của `T-05`
  cùng tranh chấp trên một hộp thư. Hot key là một tác giả nhận nhiều tương tác cùng lúc: các ghi
  của người đó chạy tuần tự; số đo ở §7.7. Count và preview của nhóm được tính bằng một aggregate
  SQL theo cùng luật với `rollup::preview` (test PG đối chiếu hai bên); bản đầu kéo cả membership về
  Rust nên hot key chỉ đạt 46 sự kiện/s.
- **`T-04 / CHOICE-18 = A` (state theo actor+target có thứ tự nguồn).** `tham_marks` giữ trạng thái
  chọn mới nhất của (người nhận, actor, work, revision) cùng `accepted_order`; sự kiện cũ hơn không
  đè sự kiện mới hơn. Chỉ `add` có `first_selection = true`, bài đọc được lúc commit và chưa từng có
  tín hiệu mới tạo tín hiệu (`P-02B`); mọi `add`/`remove` khác chỉ đối soát membership của nhóm mở
  (`P-03A`), về 0 thì nhóm đóng và ẩn. Fact lần đầu là `first_selection` của App (`#346`), không phải
  việc thiếu receipt; dấu chống báo lại vì vậy nằm ở cả hai phía: App giữ suốt đời theo dòng
  `work_endorsements`, service giữ dấu 400 ngày cho thứ tự/replay.
- **`T-05 / CHOICE-19 = A` (version trong transaction serialize theo người nhận).** `inboxes.last_seq`
  tăng khi giữ khóa dòng hộp thư, nên thứ tự số là thứ tự commit; `inbox_snapshot = s<last_seq>`,
  `group_version`/`signal_version` là các giá trị của dãy này. Hai transaction lấy số rồi commit ngược
  không thể xảy ra trong cùng hộp thư. ACK snapshot cũ là no-op; token vượt số đã commit bị từ chối.
- **`T-08 / CHOICE-22 = A` (bảng/index thường + purge theo lô).** Purge mỗi giờ, 1 000 hàng mỗi lệnh,
  theo `expires_at` có index. Hàng `inboxes` không bao giờ bị xóa để dãy số không quay lại (token cũ
  không thể xác nhận tín hiệu mới). Partition chỉ cân nhắc khi số đo dung lượng/WAL/vacuum đòi hỏi.

### 7.2. Nhóm, đóng góp và invalidation

| Loại | `group_key` (`target_key`) | Đếm | Đóng góp |
| --- | --- | --- | --- |
| `comment.root.approved` | `work/{work_id}` | Người khác nhau | Bình luận (`comment_id`) |
| `comment.reply.approved` | `root/{root_id}`, `reply/{reply_target_id}`, hoặc `purged/{comment_id}` khi neo đã mất (§4.3 B) | Người khác nhau | Bình luận |
| `work.endorsement.tham.changed` | `revision/{work_id}/{revision_id}` | Người khác nhau | Revision (một mỗi actor) |
| `work.moderation.decided` | `revision/{work_id}/{revision_id}/{approved\|rejected}` | Quyết định | — |
| `comment.moderation.adverse` | `comment/{comment_id}/{rejected\|removed}` | Quyết định | — |

Invalidation `comment` bỏ đóng góp của bình luận đó khỏi mọi nhóm còn hạn (đã đọc hoặc chưa) và
ghi fence `comment_fences`, nên một lần duyệt cũ hơn đến muộn không phục hồi được. Invalidation
`work` và `principal` chỉ có receipt (`noted`): Gateway kiểm quyền hiện hành ở mỗi lần đọc (`N4`).

### 7.3. Retention, sự kiện muộn và phục hồi

- Nhóm sống tối đa 180 ngày từ `created_at = min(now, occurred_at của sự kiện tạo nhóm)`; cập nhật
  không gia hạn; nhóm hết hạn vô hình và không bao giờ được nhập lại, kể cả trước khi purge.
- Sự kiện xảy ra quá 30 ngày (`late_event_cutoff`, bằng replay horizon của App) chỉ có receipt và
  đối soát, không tạo/hồi sinh nhóm.
- Receipt và fence 45 ngày, dấu Thấm 400 ngày, câu trả lời lệnh ACK 7 ngày.
- **Recovery horizon:** 30 ngày, bằng replay horizon của App (`REPLAY_HORIZON_DAYS`): App chỉ xóa sự
  kiện đã ACK quá 30 ngày, nên mọi sự kiện trong cửa sổ này phát lại được bằng
  `vink-app notifications replay <since> <ghi chú>`. Backup giữ 7 bộ hằng ngày, đều nằm trong horizon.
- **Bất biến khôi phục:** Notification không bao giờ được phục vụ khi đi trước App. `backup.sh` dump
  `vink_notifications` **trước** `vink`, nên restore cùng một bộ luôn để Notification ở mốc bằng hoặc
  sớm hơn App; replay lấp khoảng giữa. Sự kiện App đã mất không bao giờ còn hiệu ứng trong inbox.
- **Cổng recovery:** `vink-notifications maintenance recovery begin|end <ghi chú>` (audit append-only).
  Khi đóng, socket inbox trả `503 recovering` cho mọi lời gọi; ingest vẫn mở để replay chạy; readiness
  báo `checks.recovery`. `pg_restore` đưa cổng về trạng thái lúc dump, nên runbook đóng lại ngay sau
  restore, trước khi `serve` chạy.

| Sự cố | Làm gì | RPO / RTO |
| --- | --- | --- |
| Process `vink-notifications` chết (kill, OOM, restart) | Service manager khởi động lại; relay retry, lease hết hạn thì worker khác nhận | Không mất sự kiện; trễ bằng thời gian restart + backoff relay (≤ 10 s đầu, lease 30 s nếu worker cũng chết) |
| Chỉ DB `vink_notifications` hỏng | Dừng service → `pg_restore` bộ gần nhất → `recovery begin` → `serve` → `vink-app notifications replay <mốc dump − biên>` → chờ backlog App về 0 → `recovery end` | Sự kiện: 0 trong horizon. Seen/Read sau mốc dump mất (nhóm hiện lại chưa đọc); nhóm tạo lại giữ tuổi từ sự kiện gốc |
| Chỉ DB `vink` (App) restore lùi | `recovery begin` → dừng worker → restore App → dừng service → restore `vink_notifications` **cùng bộ** (dump trước App) → `recovery begin` → `serve` + worker → replay từ mốc dump Notification → chờ backlog về 0 → `recovery end` | App mất writes sau mốc dump của App (RPO App = chu kỳ backup); inbox khớp đúng App sau đối soát |
| Mất cả instance PostgreSQL | Restore ba DB từ cùng bộ, rồi như dòng trên | Như dòng trên cho cả ba owner |

### 7.4. Vận hành

`vink-notifications serve|migrate|maintenance purge|maintenance status|maintenance recovery begin|end`
([README](../../services/vink-notifications/README.md)).
Thứ tự bật: provision → `vink-notifications migrate` → `serve` → cấu hình relay trong `vink-worker`
→ chỉ sau đó `vink-app notifications activate` (`#346`). Backout:
[`down.sql`](../../services/vink-notifications/migrations-rollback/20261128000000_notification_inbox.down.sql)
từ chối khi còn nhóm trong hạn; backout cổng recovery từ chối khi cổng đang đóng. Dừng relay trước,
nghĩa vụ App nằm chờ và được gửi lại sau khi migrate tiến.

### 7.5. Tiêu chí của `#347` theo bằng chứng

| AC | Kết quả | Còn chờ |
| --- | --- | --- |
| AC-01 | PASS phần service/DB/role/caller: binary riêng, DB riêng, role migrator/runtime qua `provision.sh`; runtime không DDL/TEMP; kết nối chéo giữa App/Identity/Notification bị từ chối; migrate bằng runtime thoát 1; socket sai/thiếu caller `401`, route sai phạm vi `404`; type quản trị bị từ chối và không lưu (lane `verify postgres`). Client Gateway có từ [PR #391](https://github.com/loveoverflowcom/vink/pull/391) (§8). | Image OCI/compose production: ngoài phạm vi lượt này theo yêu cầu chủ dự án. |
| AC-02 | PASS (PG + hai process): receipt + hiệu ứng một transaction; mất ACK → `already_applied`; payload mismatch không ghi; cùng event hai relay một hiệu ứng; 12 writer đồng thời một nhóm mở; Read đua append không tạo hai nhóm mở; event sau Read tạo nhóm mới; đếm actor khác nhau, preview ≤ 3 mới nhất (aggregate SQL đối chiếu `rollup::preview` qua 30 sự kiện). `kill -9` service rồi worker dưới tải 400 sự kiện: đúng 400 receipt và 400 thành viên; 65 sự kiện trả `already_applied` vì đã commit trước khi mất ACK. | Rollback có fault injection giữa chừng chỉ kiểm gián tiếp (lỗi database rollback cả sự kiện). |
| AC-03 | PASS (PG): Thấm báo một lần; unlike giảm count/bù preview cả trong lẫn ngoài 3 tên; về 0 ẩn nhóm; relike âm thầm; nhóm đã Read giữ lịch sử; Remove trước Add cũ không đánh thức; relike sau purge 181 ngày vẫn im lặng. | Relike sau restore Notification thiếu dấu: chỉ dựa vào `first_selection` của App (thiết kế); diễn tập restore dùng sự kiện duyệt nhận xét, chưa có ca Thấm riêng. |
| AC-04 | PASS (PG + route): badge đếm nhóm chưa Seen; Seen theo snapshot, ACK cũ no-op, token tương lai `422`; Read theo phiên bản giữ tín hiệu mới; hai tab; MarkAllRead chỉ tới snapshot; cùng khóa khác lệnh `422`. Service trả `fresh`, `503 dependency_unavailable` hoặc `503 recovering`, không bao giờ `0` thay lỗi. | `stale` thuộc Gateway (§8.2); kiểm qua process thật ở `#348` AC-06. Đo hội tụ dưới backlog production. |
| AC-05 | PASS (PG, clock điều khiển): 180 ngày không gia hạn, ngày 181 tạo nhóm mới, purge xóa nhóm+membership, giữ dấu Thấm; sự kiện muộn không hiện. Purge 50 000 nhóm hết hạn kèm 150 000 thành viên: ≥ 24 000 nhóm/s, WAL khoảng 11–12 MiB (§7.7). | Số đo trên phần cứng production. |
| AC-06 | PASS ở máy phát triển (lane `verify postgres`, hai process và hai DB thật): backout migration trên bản sao (từ chối khi còn nhóm trong hạn, sạch sau purge, migrate tiến lại); `kill -9` service và worker (dòng AC-02); Notification restore lùi sau mốc App đã ACK: cổng recovery đóng, inbox `503 recovering`, replay đưa projection về **đúng** bản trước sự cố, 500 sự kiện trả `already_applied`; App restore lùi sau Notification: restore Notification từ dump sớm hơn **cùng bộ**, replay, 50 sự kiện App đã mất không còn hiệu ứng. Recovery horizon và RPO/RTO theo loại sự cố (riêng process, riêng DB Notification, riêng DB App, mất cả instance) ở §7.3. Workload hot key, plan, dung lượng, WAL, purge và ngân sách ở §7.7; relay xả 500 sự kiện trong 6,84 s (73 sự kiện/s đầu-cuối). | Số đo production và WAL/purge lag dưới tải liên tục. Mất cả instance chưa diễn tập riêng: quy trình là restore ba DB cùng bộ rồi như ca App lùi mốc (ca đó đã diễn tập; restore từng DB đã có ở `verify-backup`). |

Lệnh phần đầu chạy trên `e58dbff` (đã merge `develop@c8db486`), giữ làm lịch sử: `cargo test
--locked --workspace --lib --bins`, `cargo clippy --locked --workspace --all-targets -- -D warnings`,
`cargo xtask verify quick` (13/13) và `RUST_TEST_THREADS=4 cargo xtask verify postgres` (36/36) đều
PASS; chi tiết ở [PR #373](https://github.com/loveoverflowcom/vink/pull/373).

Lệnh phần hai chạy trên `develop@e60593c` cộng thay đổi của nhánh, PostgreSQL 17.11 container tạm,
mỗi lệnh trong scope `systemd-run` `MemoryMax=8G` với `CARGO_BUILD_JOBS=6`:

- `cargo clippy --locked -p vink-notifications --all-targets -- -D warnings`: PASS.
- `cargo test --locked -p vink-notifications --test postgres`: PASS 26, 2 bỏ qua (phép đo ngân sách).
- `cargo test --locked -p vink-notifications --test postgres budget:: -- --ignored --nocapture
  --test-threads=1`: PASS 2/2 (lượt 3 ở §7.7).
- `RUST_TEST_THREADS=4 cargo xtask verify postgres`: PASS 40/40 bước (App 290, Identity 55,
  Notifications 26 test PostgreSQL; provisioning bốn owner; relay hai process; bốn diễn tập AC-06;
  backup/restore).
- `cargo xtask verify quick`: lượt đầu FAIL ở unit test `http::tests::batches_and_paths_are_bounded_before_any_database_call`
  vì cổng recovery là middleware đọc DB trước khi kiểm path (request sai dạng thành `503`). Cổng
  chuyển vào handler sau bước kiểm request; lượt lại PASS 13/13. Sau bản sửa, test PostgreSQL và
  clippy của service chạy lại đạt; `verify postgres` chạy lại ở nhánh `#348` xếp chồng (§8.5).

### 7.6. Còn mở

- Image OCI và service `notifications` trong `infra/production/compose.yaml` (+ mount socket theo phạm vi);
  theo yêu cầu của chủ dự án, lượt này không build image production.
- ~~Client inbox của Gateway và badge `stale` (`#348`)~~: xem §8 (Gateway lọc nhóm theo quyền hiện
  hành, gồm tác phẩm ẩn và principal đình chỉ; invalidation `work`/`principal` không đổi projection).
- Số đo trên phần cứng production và WAL/purge lag dưới tải liên tục; số ở §7.7 là máy phát triển.
- `admin.notice.reserved` vẫn `TYPE_ONLY_NOW`: bị từ chối ở ingest, không có bảng hay route gửi.

### 7.7. Ngân sách và số đo

Đo bằng các test `#[ignore]` trong
[`tests/postgres/budget.rs`](../../services/vink-notifications/tests/postgres/budget.rs) (chạy tay:
`cargo test -p vink-notifications --test postgres budget:: -- --ignored --nocapture --test-threads=1`),
máy phát triển dùng chung 22 CPU, PostgreSQL 17.11 trong container, cấu hình mặc định. Lượt 1–2
chạy liên tiếp trước rebase; lượt 3 chạy sau rebase lên `develop@e60593c`. Ngân sách là lựa chọn kỹ
thuật (`DESIGN_PROPOSED`) và được assert trong chính test đo.

| Phép đo | Lượt 1 | Lượt 2 | Lượt 3 (`e60593c`) | Ngân sách |
| --- | --- | --- | --- | --- |
| Ingest rải đều (2 000 sự kiện, 200 hộp thư, 8 writer) | 794/s, p50 7,8 ms, p99 27,9 ms | 623/s, p50 10,1 ms, p99 24,9 ms | 499/s, p50 8,2 ms, p99 189 ms | ≥ 200/s, p99 ≤ 250 ms |
| Hot key (2 000 Thấm đầu tiên vào một nhóm, 8 writer) | 143/s, p50 41,3 ms, p99 212 ms | 147/s, p50 49,1 ms, p99 164 ms | 119/s, p50 49,5 ms, p99 432 ms | ≥ 100/s, p99 ≤ 500 ms |
| WAL mỗi sự kiện có tín hiệu | 2,2 KiB (rải), 2,6 KiB (hot) | như trên | 2,1 KiB (rải), 2,5 KiB (hot) | ≤ 8 KiB |
| Dung lượng mỗi nhóm có 3 thành viên (bảng + index) | 1 273 B (110 000 nhóm: nhóm 61,5 MiB, thành viên 72,1 MiB) | 1 280 B | 1 284 B (nhóm 61,8 MiB, thành viên 72,9 MiB) | ≤ 4 KiB |
| Badge count, hộp thư 10 000 nhóm / 100 nhóm (`EXPLAIN ANALYZE`, Index Scan `notification_groups_inbox`) | 4,99 ms / 0,12 ms | 3,30 ms / 0,20 ms | 3,97 ms / 0,15 ms | ≤ 50 ms / ≤ 5 ms |
| Trang đầu, hộp thư 10 000 nhóm (Index Scan + Incremental Sort, 5 buffer) | 0,12 ms | 0,10 ms | 0,15 ms | ≤ 20 ms |
| Purge 50 000 nhóm hết hạn kèm 150 000 thành viên, lô 1 000 | 24 596 nhóm/s, WAL 11,0 MiB | 28 548 nhóm/s, WAL 11,0 MiB | 25 793 nhóm/s, WAL 11,9 MiB | ≥ 2 000 nhóm/s |

Diễn giải: hot key bị giới hạn bởi khóa hộp thư (`T-03`) và aggregate membership O(n) mỗi lần cập
nhật nhóm; 140+ tín hiệu/s vào **một** tác giả vượt xa tải dự kiến của Vink, nên chưa cần partition
hay đếm tăng dần. Purge một lần mỗi giờ với 100 000 dòng/lượt tối đa (100 lô × 1 000) có biên lớn.
Độ trễ đuôi dao động theo tải của máy dùng chung: p99 lượt 3 (189 ms rải, 432 ms hot key) vẫn trong
ngân sách nhưng sát trần hot key, nên số production phải đo lại trước khi coi ngân sách là đủ.

---

## 8. Hiện trạng `#348`: Gateway inbox, quyết định riêng và danh sách Thấm

Nhánh `feat/348-notification-gateway-reads` (gốc `develop@002cc12`). Bốn chiều trạng thái:

| Chiều | Ghi nhận |
| --- | --- |
| Sản phẩm | `PRODUCT_CONFIRMED` trong brief `#305` R3 và quyết định 20/11/2026 về trích đoạn/Ưng (`#365`); [VDR-NOTIFICATIONS@r1](../../decisions/proposals/VDR-NOTIFICATIONS.r1.toml) và [VDR-NOTIFICATIONS@r2](../../decisions/proposals/VDR-NOTIFICATIONS.r2.toml) giữ `PENDING`. |
| Kiến trúc | `T-06 / CHOICE-20 = A` (§8.1), độ dài trích đoạn 140 grapheme, MAC token theo phiên, `cursor_key` của danh sách Thấm và hợp đồng Ưng (`#365`) ghi ở [VDR-NOTIFICATIONS-CONTRACT@r2](../../decisions/proposals/VDR-NOTIFICATIONS-CONTRACT.r2.toml) (`PENDING`, mức `DESIGN_PROPOSED`). |
| Triển khai | `persistence-api`: client socket inbox (`notification_inbox.rs`), lõi thuần `domain::notification_inbox`, đọc lô `persistence::notification_reads`, route `http::notifications` ([hợp đồng §2.8](../../services/vink-app/contracts/http/v1/notifications.md#28-route-gateway-đã-triển-khai-348), [fixture JSON](../../services/vink-app/contracts/http/v1/fixtures/notifications-gateway.json)), seam `contributions` của service ([RPC §3](../../services/vink-notifications/contracts/rpc/v1/README.md#3-inbox-socket-inbox)), migration `20261201000000_public_endorser_list`. Producer/rollup/DTO runtime của Ưng (`#365`) còn chờ; UI thuộc `#364`. |
| Bằng chứng | §8.3. |

### 8.1. `T-06 / CHOICE-20 = A`

Gateway quét ứng viên có giới hạn từ service, kiểm quyền/hydrate theo lô ở App rồi trả continuation
hữu hạn. Số đo (test PostgreSQL, fake service đếm lời gọi): một trang inbox tốn tối đa 3 lời gọi
service × 50 nhóm và 5 truy vấn App mỗi lô (works, comments, tựa revision của quyết định, quyết định
bình luận, danh tính công khai), không phụ thuộc số nhóm; 160 nhóm bị lọc trên 1 nhóm hợp lệ cho trang
đầu rỗng với `filtered_page_scan_bound` sau đúng 3 lời gọi, trang sau trả nhóm hợp lệ. Count: một lời
gọi `count` của service khi bằng 0; nếu không, tối đa 2 lô; vượt thì `count_is_lower_bound`. Chọn A vì
service đã bỏ đóng góp bình luận khi invalidation tới; phần còn lọc ở Gateway là theo tác phẩm/epoch
và quyền sở hữu, nên mật độ ứng viên hợp lệ thường cao. Chuyển sang B (projection eligibility) khi số
đo thật cho thấy tỉ lệ trang `filtered_page_scan_bound` hoặc độ trễ count vượt ngân sách.

Danh sách Thấm công khai với 20 000 hàng (18 000 đang chọn) trên một revision: `EXPLAIN (ANALYZE,
BUFFERS)` là index scan dừng sau 51 hàng, 417 buffer, khoảng 0,5 ms, không có seq scan.

### 8.2. Hạn và giới hạn công bố

| Hạn | Giá trị |
| --- | --- |
| `max_staleness_seconds` của badge `stale` | 60 (bộ nhớ trong tiến trình App, theo principal, tối đa 10 000 mục) |
| Lời gọi service | 2,5 s mỗi lời gọi (service tự cắt ở 5 s) |
| Trang inbox | `limit` 1..50 (mặc định 20), tối đa 3 lô × 50 ứng viên |
| Count | tối đa 2 lô × 50 nhóm chưa Seen |
| Đóng góp mỗi nhóm bình luận | 16 mới nhất + tổng số |
| Trích đoạn | 140 extended grapheme cluster sau NFC và gộp khoảng trắng |
| Danh sách Thấm | `limit` 1..50 (mặc định 20) |

### 8.3. Tiêu chí của `#348` theo bằng chứng

| AC | Kết quả | Còn chờ |
| --- | --- | --- |
| AC-01 | PASS (PG + route): chủ inbox lấy từ phiên; khách `401` trước mọi lời gọi; thiếu `Origin` `403`; principal đình chỉ `403 principal_suspended`, service không nhận lệnh; token của phiên/principal khác `422`; chưa cấu hình socket `503`; danh sách Thấm đọc được không cần phiên. | Kiểm qua Identity thật (fixture `HeaderIdentity`); browser A → B → A thuộc `#364`. |
| AC-02 | PASS (PG + route): count theo quyền, `fresh`/`stale` có hạn/`503`, không `0` giả; Seen/Read/MarkAllRead chuyển đúng snapshot/version đã xác thực; count bằng 0 không quét inbox. Ngữ nghĩa ACK cũ/hai tab/mark-all ở service: test PG của `#347`. | Đo polling dưới backlog thật và độ trễ hội tụ. |
| AC-03 | PASS (unit + PG): lọc theo tác phẩm/epoch/chủ sở hữu; chọn lại đóng góp hợp lệ khi bình luận mới nhất bị gỡ/giữ; nhóm không còn đóng góp bị ẩn; trang bị lọc dày có continuation hữu hạn; không lộ thân, tên hoặc `PrincipalId`. | Nhóm đổi vị trí chỉ được lập luận theo keyset (không lặp, hiện ở đầu khi tải lại), chưa có test riêng; mở đích là route Reader hiện có. |
| AC-04 | PASS (PG + route): chi tiết quyết định bình luận đúng người viết, đúng version, `comment_body = null` khi đã gỡ, không lộ người duyệt; chủ tác phẩm và người khác `404`; quyết định tác phẩm chỉ chủ đọc. | Chuỗi duyệt → bác → kháng nghị duyệt lại qua route chưa có test riêng (`is_current` có trong SQL). |
| AC-05 | PASS (PG + route): khách đọc đủ thành viên qua nhiều trang (> 3), bỏ Thấm giữa trang, `304`, đổi revision `409`/trang đầu revision cũ `404`, bài không công khai `404`; nguồn là `work_endorsements`, không phụ thuộc inbox. | — |
| AC-06 | PARTIAL: ví dụ JSON trong hợp đồng đã đối chiếu runtime; test dùng client HTTP ngoài Leptos. | Fixture HTTP trong `contract.rs`; đặc tả polling chi tiết cho `#364` ở hợp đồng §2.7. |

`#365`: AC-01/02 phần Gateway PASS (trích đoạn, cắt theo grapheme, các ca gỡ/hold/ẩn/mất quyền và
lùi về đóng góp hợp lệ); PRD `NOTIF-07` đã cập nhật. Còn lại: proposal VDR cho trích đoạn (AC-03) và
toàn bộ thông báo Ưng (AC-04..06: producer `comment.endorsement.ung.changed`, rollup ở service, đích
`work_comment_endorsement`) — chưa triển khai.

Lệnh đã chạy (PostgreSQL 17.11 container tạm, scope `systemd-run` 6/8 GiB, `CARGO_BUILD_JOBS=6`):
`cargo test -p vink-app --lib` (unit mới), `cargo test -p vink-app --test postgres`,
`cargo test -p vink-notifications --test postgres`, `cargo clippy -p vink-app -p vink-notifications
--all-targets -- -D warnings`. Kết quả từng lượt nằm trong mô tả PR.

### 8.4. Còn mở

- Kiểm thử hai process thật (App Gateway → `vink-notifications` qua socket inbox) trong
  `cargo xtask verify postgres`; hiện client socket có test socket thật, route có test với fake service.
- Proposal VDR cho trích đoạn; thông báo Ưng (`#365` AC-04..06).
- Bộ nhớ count `stale` là theo tiến trình; nhiều replica App sẽ có bộ nhớ riêng.
