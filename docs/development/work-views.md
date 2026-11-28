# Lượt xem tác phẩm — registry triển khai và bàn giao

Registry này ghi mức triển khai, bản đồ từ quyết định sang issue và kịch bản nghiệm thu xuyên
suốt cho Lượt xem. Luật, ví dụ và vòng đời nằm ở [đặc tả nghiệp vụ](../architecture/work-views.md).
Yêu cầu có ID nằm ở [PRD §17](../../prd/requirements.md#17-lượt-xem-tác-phẩm-work-views).
Nguồn: [Epic #323](https://github.com/loveoverflowcom/vink/issues/323),
[#340](https://github.com/loveoverflowcom/vink/issues/340) và các issue con
[#341](https://github.com/loveoverflowcom/vink/issues/341),
[#342](https://github.com/loveoverflowcom/vink/issues/342),
[#343](https://github.com/loveoverflowcom/vink/issues/343).

## 1. Trạng thái bốn chiều

| Chiều | Trạng thái |
| --- | --- |
| Quyết định sản phẩm | VIEW-01..12 `r1` là `PRODUCT_CONFIRMED` ngày 21/11/2026 ([VDR-WORK-VIEWS@r1](../../decisions/proposals/VDR-WORK-VIEWS.r1.toml) + [signoff](../../decisions/signoffs/VDR-WORK-VIEWS.r1.toml), revision đang kích hoạt của marker VDR). Delta 25/11/2026 (`#323`/`#340`: ngưỡng 5 giây foreground cấu hình tập trung, `OQ-1=A`, `OQ-2`, `OQ-3A/B`, `OQ-4`, `UX-01..10`, giữ riêng tư, `D-01` Thấm → Nhận xét → Lượt xem, `D-02` bỏ nút Đọc ở Bàn viết) được chủ sản phẩm chốt trên issue và ghi thành [VDR-WORK-VIEWS@r2](../../decisions/proposals/VDR-WORK-VIEWS.r2.toml) (`PENDING` chờ signoff đúng payload/SHA-256 theo `VDR-DECISION-SIGNOFF@r1`). |
| Quyết định kiến trúc | CHOICE-02 A, CHOICE-03 và CHOICE-06 A `DESIGN_PROPOSED` bởi #341 (§7.1); CHOICE-05 A `DESIGN_PROPOSED` bởi #342 (§9.1); CHOICE-01, CHOICE-04 và CHOICE-POLICY `DESIGN_PROPOSED` bởi #343 (§10). Thiết kế r3 Bàn viết ([LayoutSpec](../design/work-views.md)) giữ `DESIGN_PROPOSED`; `DQ-05` và `DQ-10` giữ `OPEN`. Chưa lựa chọn kiến trúc nào `Accepted`. |
| Mức triển khai | Đường ghi #341: `core` + PostgreSQL + dọn hạn `vink-worker`. Đọc #342: `persistence-api` (`views` của `GET /api/v1/author/works`). #343: `vink-work-view-policy` + HTTP `POST /api/v1/works/{work_id}/views` + cookie khách + Reader visibility tracker & retry hữu hạn. #514 (ghép thêm commit trong PR #519 theo yêu cầu chủ sản phẩm): UI Bàn viết Design r3 (`AuthorWorkRow`, `DeskIndex`, `#views-explainer`), bỏ nút Đọc (`D-02`), thứ tự `Thấm → Nhận xét → Lượt xem` (`D-01`), và đồng bộ `docs/design/*`. Xóa tài khoản tác giả (#550) gọi `erase_work` trong giao dịch fence (`OQ-2`); race, rollback và tài khoản người đọc kiểm ở §13. Route xóa tác phẩm và admin gỡ cả bài: `CONDITIONAL_DEFERRED` (owner Publication). |
| Bằng chứng | #340 r1: kiểm tài liệu (§6); #341: unit + PostgreSQL (§7.6); #342: unit/contract + PostgreSQL qua HTTP (§9.5); #519 tại §12, vòng đời xóa tài khoản tại §13 và nghiệm thu Web của #323 tại §14. |

## 2. Snapshot quyết định

Tra bằng `cargo xtask decision` tại nhánh `docs/340-dac-ta-luot-xem`, base
`develop@9f68c513b5dae224b6a9f896d04bc9d1d035e1b8`, sau khi ghi signoff:
snapshot `8a322aed0446142317f78323e9bcb1748deae7e50e6cfbc892c360f2bc68aaaf`
(137 revision). Trước signoff, ngay sau khi tạo proposal, snapshot là
`176154f6ec916e58eb3e4d5db8419e7e256d8c2dfcc8c3b8a643e67708fa8330`. Snapshot đổi khi kho có
revision hay signoff mới; người tiếp tục tra lại.

| ID@rN | Trạng thái | SHA-256 payload | Vai trò |
| --- | --- | --- | --- |
| `VDR-WORK-VIEWS@r1` | `PRODUCT_CONFIRMED` (`current`, signoff 21/11/2026) | `cfb789cc2c5f7a99a2f8c8e24f0057815d736d7b02cdacde50d327c8a525e008` | Revision lịch sử đã signoff, còn đang trỏ bởi marker `active` tới khi `@r2` có signoff. |
| `VDR-WORK-VIEWS@r2` | `PENDING` (proposal 25/11/2026, chưa signoff) | `f6a3bcb7b8d27b188d18878e68efa1c04bb776ccaf0a06c3f27c57aded904a60` | Delta 25/11/2026 (`VIEW-01` 5s cấu hình chung, `OQ-1..4`, `UX-01..10`, `D-01/D-02`, 2 PR). |
| `O-17@r2` | `PRODUCT_CONFIRMED` | `cb30cb43040a3fad50bd4930096e537057a2f3c6037b387d8c332be497fa15aa` | Trang rút sau kiểm duyệt trả HTTP 200; không phải lượt. |
| `USER-02@r1` | `PRODUCT_CONFIRMED` | `2293bf58194809957b892b74bc3891d56a05998e7b97008ad1c8fbfd98fdc97b` | Hai audience. |
| `LVE-PR-01-CHOICE-02@r1` | `DESIGN_PROPOSED` | `766485bf6d2c6eabc3292921fc478d01a13376b46bed36bee9a00150a6ad6a03` | Thấm gắn `RevisionId`; Lượt xem cố ý khác (P-06). |
| `VDR-DECISION-SIGNOFF@r1` | `PRODUCT_CONFIRMED` | `add9c28677a66d759cc4c352eada0ebf92cc64dcc0849d489f2d89f16a022dc2` | Brief chưa tự là signoff của payload TOML. |
| `VDR-AUTHOR-WORK-FEEDBACK@r1` | `PENDING` | `4616ced97f723fac18cc31b1c11639d1eb7649b83340bb2d702d92999a3a14a7` | Mẫu “—” khác “0”; ẩn Thấm/Nhận xét khi riêng tư (`OQ-1 = A`). |
| `O-08@r1` | `OPEN` | `410cb3637a3d013c49b9bd746ec8b5ddb292761fed6b987c3134b281c95b4375` | Purge vật lý của tác phẩm, tách khỏi luật xóa số liệu (OQ-2). |

## 3. Mức triển khai theo yêu cầu

| ID | Mức hiện tại | Issue sở hữu | Ghi chú |
| --- | --- | --- | --- |
| VIEW-01 | Kiểm tại lúc ghi: `core` + PostgreSQL (#341). Ngưỡng 5s + điểm phát Reader + HTTP intake: PR1 (#343) | #343 (ngưỡng 5s, điểm phát, HTTP), #341 (kiểm tại lúc ghi) | Điểm hiển thị: `ReaderResponse::Published` → `PublicationView`; ngưỡng dùng chung `WORK_VIEW_MIN_VISIBLE_MS = 5_000` trong `vink-work-view-policy`. Hủy khi rời/unmount/đổi bài/đổi revision thực sự/mất công khai/đổi authority phiên; chờ foreground khi tab nền (OQ-3B); không tự đếm khi cập nhật nền từ trang ẩn/rút (OQ-3A). |
| VIEW-02 | PR1 (#343) | #343 | No-JS không đếm theo luật. `is_speculative` (HTTP) + `!document.prerendering` (client). |
| VIEW-03 | `core` + PostgreSQL (#341) | #341 | Cửa sổ trượt neo tại lượt được tính; đúng 24 giờ vẫn chặn. Receipt theo sự kiện tách khỏi cửa sổ. |
| VIEW-04 | `core` + PostgreSQL (#341); cookie `__Host-vink_guest` ở PR1 (#343) | #341, #343 (cấp cookie) | `GuestToken` UUID v4, thời hạn 24 giờ cố định, không làm mới. |
| VIEW-05 | `core` + PostgreSQL (#341) | #341 | So principal với `works.owner_id` tại lúc ghi; client hủy nếu đổi authority phiên. |
| VIEW-06 | Khóa ghi: `core` + PostgreSQL (#341). Đọc: `persistence-api` (#342) | #341 (khóa ghi), #342 (đọc không reset) | Mọi bảng khóa theo `WorkId`; revision chỉ là căn cứ ghi trong receipt. Đọc không nhìn revision. |
| VIEW-07 | `persistence-api` (#342) + `web` (#514) | #342, #514 | Chỉ `GET /api/v1/author/works` của phiên chủ; không API công khai; D-01 thứ tự Thấm → Nhận xét → Lượt xem; D-02 bỏ hẳn nút “Đọc” trên Bàn viết. |
| VIEW-08 | Phía ghi: `core` + PostgreSQL (#341). Đọc: `persistence-api` (#342) + `web` (#514) | #342 (đọc), #341 (không tăng khi hết công khai), #514 (hiển thị lịch sử) | Tổng không bao giờ giảm khi ẩn/rút (`OQ-1 = A`, kiểm PostgreSQL). Admin gỡ cả bài (`OQ-4`): luật đọc theo `WorkId` không nhìn lifecycle, nhưng chưa có route admin nên chưa có ca runtime (`CONDITIONAL_DEFERRED`, §13). |
| VIEW-09 | Mốc lưu ở `work_view_counting` (#341). Dữ liệu nhãn: `persistence-api` (`counted_since`, #342). Nhãn hiển thị: `web` (#514) | #342, #514 | Không backfill; mốc là lần quyết định sự kiện đầu tiên; hiển thị “Tính từ dd/mm/yyyy” theo `Asia/Ho_Chi_Minh`. |
| VIEW-10 | Idempotency: `core` + PostgreSQL (#341). Retry hữu hạn: `web` (#343) | #343 (retry), #341 (idempotency) | Kỳ vọng khoảng 1 phút, không SLA. Retry cùng UUIDv7 trong tab; lỗi thống kê không cản đọc. |
| VIEW-11 | `core` + PostgreSQL + `vink-worker` (#341); xóa theo tài khoản tác giả: `persistence-api` (#550) | #341, #550 (xóa tài khoản), PR có route xóa tác phẩm (`OQ-2`) | Ba loại dữ liệu; cửa sổ và receipt xóa sau 24 giờ (chu kỳ dọn mỗi giờ). `erase_work` chạy trong giao dịch fence xóa tài khoản (§13). Route xóa tác phẩm: `CONDITIONAL_DEFERRED`. |
| VIEW-12 | Trạng thái dữ liệu: `persistence-api` (`views.state`, #342). UI + copy + dialog giải thích: `web` (#514) | #342 (dữ liệu), #514 (hiển thị & UX-01..10) | Giải thích `UX-07` (`#views-explainer`) lấy ngưỡng từ cùng nguồn `vink-work-view-policy`. |

## 4. Bản đồ P/CHOICE sang issue và kế hoạch 2 PR

| Issue | Kết quả | P-ID | CHOICE | Câu chốt 25/11 liên quan |
| --- | --- | --- | --- | --- |
| [#341](https://github.com/loveoverflowcom/vink/issues/341) Ghi và chống trùng (đã merge) | Use case ghi, migration, adapter PostgreSQL, purge mã hết hạn | P-03, P-04, P-05, P-11 (P-06, P-01/P-08 tại commit) | CHOICE-02, CHOICE-03, CHOICE-06 | `OQ-2` (sẵn sàng `erase_work` theo `WorkId`) |
| [#342](https://github.com/loveoverflowcom/vink/issues/342) Đọc theo quyền (API đã merge) | Query/read-model, DTO, contract đọc của tác giả (`views`) | P-07, P-08, P-09 (P-06, VIEW-12 dữ liệu) | CHOICE-05 (A, §9.1) | `OQ-1 = A`, `OQ-4` (giữ số khi còn quyền); giữ mở tới khi có route xóa/admin remove (`OQ-2`) |
| **PR1 (`#340` + `#343`)** | Delta SSOT + `VDR-WORK-VIEWS@r2` (`PENDING`), `vink-work-view-policy`, HTTP intake + `__Host-vink_guest`, Reader 5s visibility tracker + retry hữu hạn, kiểm xuyên suốt qua API `#342` | P-01 (5s), P-02, P-04 (cookie), P-10, D-01/D-02 SSOT | CHOICE-01, CHOICE-03 (cookie/`503`), CHOICE-04, CHOICE-06 (HTTP), CHOICE-POLICY | `OQ-3A/B`, cấu hình ngưỡng 5s dùng chung |
| **PR2 (`#514`, ghép commit vào PR #519)** | Đồng bộ [LayoutSpec Lượt xem r3](../design/work-views.md) + R4.3/R4.4 + `docs/design/desk-author-feedback.md`, tích hợp Work Views Design r3 vào `AuthorWorkRow` và `DeskIndex` (thứ tự `Thấm → Nhận xét → Lượt xem`, bỏ hẳn nút “Đọc”, icon `visibility-outline`, định dạng `1.234`, `counted`/`not_started`/`never_public`/`unavailable`, `Tính từ dd/mm/yyyy`, lịch sử `OQ-1=A`, nút + sheet `#views-explainer`) | VIEW-07, VIEW-09, VIEW-12, D-01, D-02 | — | `OQ-1 = A`, `UX-01..10`, `D-01/D-02` (`DQ-05`/`DQ-10` giữ `OPEN`) |

Kế hoạch ban đầu ngày 25/11/2026 tách 2 PR (`#340+#343` rồi `#514`); theo chỉ dẫn trực tiếp của chủ sản phẩm, phần triển khai `#514` được bổ sung bằng commit kế tiếp trên cùng PR `#519` (`feat/343-work-views-intake`) rồi rà soát và merge vào `develop`.

## 5. Kịch bản nghiệm thu xuyên suốt

Dùng làm oracle chung cho `#341`–`#343` (và `#514` ở phần hiển thị Bàn viết). Giả định mặc định
theo delta 25/11: tác phẩm W công khai lần đầu sau mốc bật; ngưỡng hiển thị dùng chung
`WORK_VIEW_MIN_VISIBLE_MS = 5.000 ms`; T là tác giả; A là thành viên; G là khách; D là admin.
Tổng là số tác giả đọc được qua `GET /api/v1/author/works` (`views.count`).

| Bước | Thao tác | Tổng mong đợi | Ví dụ, vòng đời | Lane |
| --- | --- | --- | --- | --- |
| 1 | T mở Bàn viết | `0` thật (hoặc `not_started` trước mốc toàn cục) | L-02 | #342 HTTP, #514 UI |
| 2 | 08:00 A mở W < 5s (ví dụ 4.9s rồi quay lại) | `0` (không phát request) | E-09, E-38 | #343 browser |
| 3 | A mở W foreground 3s → ẩn tab 10s → quay lại foreground thêm 2s (đủ 5s hiển thị) | `1` (ẩn không tính, cộng dồn đúng 5s) | E-01, E-39, E-40 | #343 browser |
| 4 | A mở ở tab nền (`hidden`); chưa chuyển tới tab đó; sau mới chuyển sang và ở 5s | `1` lúc nền → giữ; +1 khi foreground đủ ngưỡng (nếu ngoài 24h / người khác) | E-07 (OQ-3B) | #343 unit + browser |
| 5 | A refresh, Back rồi Forward trong 24h dù ở lại ≥ 5s | Không tăng | E-14, E-15 | #343 browser |
| 6 | Gửi lại cùng mã sự kiện sau khi mất phản hồi / 503; bấm đúp | Chỉ tính tối đa `1` lần cho mã đó | E-27, E-28, E-32 | #341 PG, #343 HTTP/browser |
| 7 | G mở link chia sẻ W ở tab mới đủ 5s, rồi mở tab thứ hai trong 24h | +1 cho lần đầu (kèm `__Host-vink_guest`), +0 tab thứ hai | E-05, E-19, E-29 | #343 browser |
| 8 | T đăng nhập mở W ở Reader đủ 5s; mở nháp ở Bàn viết | +0 (self-view loại ở server) | E-22, E-23 | #341, #343 |
| 9 | D mở ca duyệt của W; rồi mở W ở Reader công khai đủ 5s | Ca duyệt +0, Reader +1 | E-25, E-26 | #341, #343 |
| 10 | Trình duyệt tắt JS mở W; bot/prefetch tải W | +0 | E-31, E-06, E-08, E-18 | #343 HTTP/browser |
| 11 | r2 thay r1; trong tab đang mở A bấm “Xem bản mới” (hoặc mở lại W lúc 10:00) | Tab đang mở: +0 (hủy bộ đếm chưa gửi); mở lại trong 24h: +0, không reset | E-35, E-37, L-06 | #341, #342, #343 |
| 12 | T chuyển W sang “Chỉ mình tôi” (hoặc bị rút kiểm duyệt); A mở URL cũ; sau đó T công khai lại trong khi tab A đang mở trang ẩn | Mở trang thay thế +0; Bàn viết/API giữ số cũ (`OQ-1 = A`, `L-07`/`L-08`); cập nhật nền sang thân bài không tự đếm (`OQ-3A`) | E-12, E-13, E-36, L-07, L-08 | #342, #343 |
| 13 | T công khai lại; A chủ động mở mới W đủ 5s lúc 08:01 hôm sau | +1 tiếp từ số cũ | E-17, L-09 | #341, #343 |
| 14 | Làm lỗi nguồn số liệu khi đọc | `unavailable` (“—” kèm lý do); tên, trạng thái, link sửa còn | L-05 | #342, #514 |
| 15 | Làm lỗi đường ghi khi A ở lại đủ 5s | Reader đọc bình thường; retry hữu hạn cùng `event_id`; +1 nếu thành công, +0 nếu hết lượt, không bao giờ +2 | E-32, E-33 | #343 |
| 16 | Tác phẩm khác V công khai trước mốc bật | Số kèm `counted_since` (“Tính từ dd/mm/yyyy”) | L-12 | #342, #514 |
| 17 | Override `WorkViewDurationPolicy` sang giá trị khác mặc định (ví dụ `1.200 ms`) | Ngưỡng đếm và chuỗi giải thích `UX-07` đổi đồng bộ | E-43 | #343, #514 unit |

## 6. Bằng chứng của PR tài liệu #340

Chỉ kiểm tài liệu; không chạy runtime. Kết quả ghi ở mục cuối sau khi chạy trên nhánh.

| Phép kiểm | Mục đích |
| --- | --- |
| `git diff --check` | Không có lỗi khoảng trắng. |
| Kiểm link tương đối của các file đã sửa | Mọi đường dẫn và anchor nội bộ trỏ đúng. |
| `cargo xtask decision validate` | Kho TOML hợp lệ sau khi thêm proposal. |
| `cargo xtask decision query VDR-WORK-VIEWS` | `pending_only` trước signoff, `current` sau signoff. |
| `wc -c AGENTS.md` | Dưới 24.000 byte. |
| Đối chiếu P-01..P-11 của #323 | Mỗi câu có dòng VIEW-ID, dòng trong bảng quyết định và ví dụ. |

### Đối chiếu 11 câu của #323

| #323 | PRD | Bảng quyết định | Ví dụ, vòng đời |
| --- | --- | --- | --- |
| P-01 | VIEW-01 | Có | E-01, E-09..E-13, E-36 |
| P-02 | VIEW-02 | Có | E-01..E-08, E-18, E-31 |
| P-03 | VIEW-03 | Có | E-14..E-17, E-21, E-27..E-29 |
| P-04 | VIEW-04 | Có | E-19, E-20, E-30 |
| P-05 | VIEW-05 | Có | E-22..E-26 |
| P-06 | VIEW-06 | Có | E-35, L-06 |
| P-07 | VIEW-07 | Có | Cột “Ai thấy gì” |
| P-08 | VIEW-08 | Có | E-12, E-13, E-34, L-07..L-09 |
| P-09 | VIEW-09 | Có | L-12, L-13 |
| P-10 | VIEW-10 | Có | E-28, E-32..E-34, L-04, L-05 |
| P-11 | VIEW-11 | Có | L-10, L-11 |

### AC của #340

| AC | Phép kiểm | Kết quả, gap |
| --- | --- | --- |
| AC-01 | Mỗi P-ID dẫn về lời chốt 21/11/2026; OQ-1..OQ-4 ghi `OPEN` kèm ca bị ảnh hưởng | Đủ trong PRD §17 và đặc tả §3–§4. |
| AC-02 | Bảng ví dụ phủ đủ danh sách của brief | Đặc tả §6 (E-01..E-37). |
| AC-03 | Vòng đời theo trạng thái, độc giả đọc được khi thống kê lỗi, nhãn đúng độ phủ | Đặc tả §7, §8. Xóa tác phẩm/tài khoản và admin gỡ cả bài chỉ ở mức target. |
| AC-04 | PRD, đặc tả, proposal, registry trỏ đúng owner; bản đồ P/CHOICE | §4 của registry này. Proposal giữ `PENDING` tới lúc có lời duyệt. |
| AC-05 | Trình payload `VDR-WORK-VIEWS@r1` cho chủ sản phẩm; signoff chỉ khi có lời duyệt đúng ID@r1 | Chủ sản phẩm trả lời nguyên văn “Duyệt r1” cho câu hỏi duyệt `VDR-WORK-VIEWS@r1` (sha `cfb789cc…`) trong phiên #340 ngày 21/11/2026. Đã chạy `decision signoff` với đúng SHA-256 và trích lời đó. |

## 7. Đường ghi (#341)

Nguồn: [#341](https://github.com/loveoverflowcom/vink/issues/341), base
`develop@d478a0633a4772fc3b6bfdb49e29af2418b25089`, đặc tả `VDR-WORK-VIEWS@r1`
(`PRODUCT_CONFIRMED`). Code:
[`domain/work_view.rs`](../../services/vink-app/src/domain/work_view.rs) (luật thuần),
[`application/work_view.rs`](../../services/vink-app/src/application/work_view.rs) (use case),
[`persistence/work_view.rs`](../../services/vink-app/src/persistence/work_view.rs) (transaction,
dọn hạn, xóa theo `WorkId`),
[migration](../../services/vink-app/migrations/20261130000000_work_views.sql),
[backout](../../services/vink-app/migrations-rollback/20261130000000_work_views.down.sql) và vòng
dọn hạn của `vink-worker`. Chưa có route HTTP: chưa có lượt nào được đếm trong runtime.

### 7.1 Lựa chọn kỹ thuật (`DESIGN_PROPOSED`)

Merge code không làm các lựa chọn này thành `Accepted`.

| CHOICE | Lựa chọn | Lý do | Giới hạn đã biết |
| --- | --- | --- | --- |
| CHOICE-02 | **A**: một transaction cho mỗi sự kiện: khóa chia sẻ pointer công khai → advisory lock theo sự kiện → hàng cửa sổ `FOR UPDATE` → `decide_view` → cộng tổng, tổng ngày, receipt → commit. | Ít thành phần; số có ngay sau commit nên không cần chứng minh hội tụ trễ; tổng và tổng ngày luôn khớp vì cùng transaction. | Mọi lượt được tính của một tác phẩm tuần tự hóa trên hàng tổng. Đo local: 32 lượt đồng thời của một bài mất 0,5–1 s (máy dev đang tải). Chuyển sang B (sự kiện bền + tổng hợp trễ) khi đo được tranh chấp thật. |
| CHOICE-03 | Thành viên: `PrincipalId` từ phiên. Khách: `GuestToken` (UUID v4) first-party, sống 24 giờ **cố định** từ lúc cấp, không làm mới. Không gộp khách với tài khoản (E-30: lần hai +1). Phiên không hợp lệ hay hết hạn là khách. Thành viên bị đình chỉ vẫn được tính như người đọc: đọc không phải thao tác ghi có thẩm quyền nên không lấy standing fence. Mã lưu là HMAC-SHA-256 theo khóa server của (loại người đọc, id, `WorkId`). | Không thêm loại định danh nào ngoài đặc tả. Khóa server khiến một bản sao DB không đủ để thử “thành viên X có đọc W không”. Gắn `WorkId` để không nối hàng của một người đọc qua nhiều tác phẩm. | Đổi khóa làm mọi cửa sổ đang mở mất hiệu lực: mỗi (người đọc, tác phẩm) có thể được tính thêm tối đa một lần trong 24 giờ sau; chỉ đổi khi lộ khóa. Người có cả DB lẫn khóa vẫn thử được trong 24 giờ lưu giữ. |
| CHOICE-06 | **A** (luật ở #341, áp tại HTTP ở #343): kiểm input (event UUIDv7, mã khách UUID v4), idempotency theo sự kiện, gộp theo cửa sổ, chân trời thời gian của sự kiện ±11 giờ, `INGEST_LIMIT` 120 yêu cầu/10 phút/mạng **trong bộ nhớ tiến trình**, không tính yêu cầu speculative (`is_speculative`: `Sec-Purpose`/`Purpose`/`X-Purpose` chứa `prefetch` hay `prerender`). | So sánh bên dưới. B cần khóa ký và một mã riêng cho mỗi lần mở trong phản hồi Reader, trái với Reader công khai đang cache theo `ETag` (READ-01/02). | Bot chạy JavaScript và xoay mã khách vẫn cộng được tối đa một lượt/mã/24 giờ trong giới hạn tần suất. Mạng dùng chung vượt giới hạn có thể mất lượt (VIEW-10 chấp nhận). Đồng hồ máy khách lệch quá 11 giờ thì mất lượt. Không lưu khóa mạng nào của lưu lượng lượt xem. |

So sánh CHOICE-06 theo các ca issue yêu cầu:

| Ca | A (đã chọn) | B (bằng chứng mở bài do server cấp) |
| --- | --- | --- |
| Bấm đúp, hai tab | Một thân bài hiện → một sự kiện; nhiều sự kiện của cùng người đọc gộp theo cửa sổ (+1). | Như A, thêm một lần cấp mã. |
| Mã khách hết hạn | Cookie hết 24 giờ sau lúc cấp → mã mới → có thể +1 sớm hơn 24 giờ kể từ lượt trước của trình duyệt đó, như khi khách xóa cookie (VIEW-04). Không làm mới nên không mã nào sống quá một cửa sổ. | Như A. |
| Thiết bị dùng chung mạng | Chung giới hạn tần suất; NAT lớn có thể mất lượt, không bao giờ cản đọc. | Như A nếu vẫn giới hạn theo mạng. |
| Chi phí | Không có bí mật mới cho mỗi lần mở; một bộ đếm trong bộ nhớ cho mỗi mạng. | Khóa ký, mã trong phản hồi Reader (phá cache công khai) hoặc thêm một request trước khi gửi. |

### 7.2 Luật ghi đã chọn

`decide_view` kiểm theo thứ tự, và adapter chỉ thực hiện đúng quyết định trả về:

| # | Điều kiện | Kết quả | Ghi gì |
| --- | --- | --- | --- |
| 1 | Đã có receipt của mã sự kiện, cùng tác phẩm | `Replayed(<quyết định đầu>)`, bất kể người đọc hay đồng hồ hiện tại | Không ghi |
| 1b | Receipt của mã đó thuộc tác phẩm khác | `Ignored(event_reused)` | Không ghi |
| 2 | Thời điểm trong UUIDv7 ngoài [now − 11 giờ, now + 11 giờ] | `Ignored(outside_horizon)` | Không ghi |
| 3 | Không có tác phẩm | `Ignored(work_not_found)` | Không ghi |
| 4 | Không có pointer công khai lúc quyết định | `Skipped(not_public)` | Receipt |
| 5 | Thành viên là chủ tác phẩm | `Skipped(own_work)` | Receipt |
| 6 | Cửa sổ còn mở: `now − counted_at ≤ 24 giờ` | `Skipped(within_window)`, không kéo dài cửa sổ | Receipt |
| 7 | Còn lại | `Counted` | Cửa sổ neo tại `now`, +1 tổng, +1 ngày (`Asia/Ho_Chi_Minh`), receipt |

- **Đúng mốc 24 giờ** vẫn chặn; sau mốc một micro giây thì tính (khớp “đã quá 24 giờ”).
- **Gửi lại sau khi receipt hết hạn.** Receipt giữ 24 giờ, lớn hơn bề rộng chân trời (11 + 11
  giờ), nên mọi lần gửi lại còn trong chân trời đều gặp receipt của nó; lần gửi lại muộn hơn bị
  bỏ qua vì quá cũ. Gửi trùng kỹ thuật vì thế không thành lượt mới kể cả sau khi dữ liệu chống
  trùng đã xóa (AC-03).
- **Mất phản hồi của khách mới.** Lần gửi đầu đã commit nhưng phản hồi (và cookie khách) bị mất,
  lần gửi lại mang mã khách mới: receipt theo mã sự kiện, không theo người đọc, nên vẫn là gửi lại.
- **Đồng hồ.** Thời điểm quyết định do caller truyền (HTTP dùng đồng hồ tiến trình), cắt về micro
  giây, không trộn với `now()` của DB. Cửa sổ, receipt, ngày và dọn hạn dùng cùng một đồng hồ, nên
  test PostgreSQL điều khiển được dòng thời gian.

### 7.3 Dữ liệu và lưu giữ (VIEW-11)

| Bảng | Nội dung | Lưu giữ |
| --- | --- | --- |
| `work_view_totals` | `work_id`, `views ≥ 1`, `updated_at` | Tới khi xóa tác phẩm |
| `work_view_days` | `work_id`, `day` (`Asia/Ho_Chi_Minh`), `views ≥ 1`; tổng các ngày = tổng | Tới khi xóa tác phẩm |
| `work_view_windows` | `work_id`, `reader_digest` (HMAC 32 byte), `counted_at` | Xóa khi `counted_at < now − 24 giờ` |
| `work_view_receipts` | `event_digest` (SHA-256 của mã sự kiện), `work_id`, `verdict`, `basis_revision_id` (căn cứ publication), `decided_at`; **không có người đọc** | Xóa khi `decided_at < now − 24 giờ` |
| `work_view_counting` | `started_at`: lần quyết định sự kiện đầu tiên (mốc VIEW-09) | Một hàng, giữ mãi |

Trigger giữ: tổng và tổng ngày chỉ tăng đúng 1, cửa sổ chỉ tiến, receipt và mốc không bao giờ bị
sửa. Mọi bảng cascade theo `works`; `erase_work` xóa theo `WorkId` trong transaction của caller
(OQ-2). `vink-worker` dọn mỗi giờ (`prune_work_views`, lô 1.000, tối đa 100 vòng, `SKIP LOCKED`), nên
hàng hết hạn bị xóa vật lý trong khoảng một giờ sau mốc 24 giờ. Không có IP, fingerprint, danh
sách người xem hay lịch sử đọc; không backfill. Backout từ chối khi đã có tổng.

### 7.4 Hợp đồng cho #342 và #343

API nội bộ: `Vink::record_work_view(&ViewDigestKey, Viewer, RecordWorkView { work_id, event_id },
now) -> Result<WorkViewRecord, WorkViewError>`. `WorkViewRecord` có `result`, `basis` (revision
công khai mà quyết định đầu thấy), `decided_at` và `delta()`.

| Kết quả | Tổng | Đề xuất cho HTTP #343 |
| --- | --- | --- |
| `Counted` | +1 | Một phản hồi thành công thống nhất (ví dụ `204`) cho mọi kết quả `Ok`, để phản hồi không lộ quyền tác giả, trạng thái riêng tư hay cửa sổ. |
| `Skipped(within_window \| own_work \| not_public)` | 0 | Như trên. |
| `Replayed(..)` | 0 | Như trên; tab ngừng gửi lại. |
| `Ignored(outside_horizon \| event_reused)` | 0 | Như trên; tab ngừng gửi lại. |
| `Ignored(work_not_found)` | 0 | **Bắt buộc** giống hệt `not_public`. |
| `Err(Invalid(field))` | — | `400`/`422` kèm `field`; không gửi lại. |
| `Err(Unavailable)` | — | `503`; gửi lại cùng mã sự kiện trong giới hạn. |
| `Err(Internal)` | — | `500`; không có gì được commit. |

#343 cần thêm: (1) chọn `Viewer` (phiên hợp lệ → thành viên; còn lại → cookie khách; chưa có thì
`GuestToken::generate()` và `Set-Cookie` tiền tố `__Host-`, `HttpOnly`, `Secure`, `SameSite=Lax`,
`Path=/`, `Max-Age` = `GUEST_TOKEN_TTL` = 86.400 giây, không làm mới cookie còn hạn); (2) một khóa
bí mật 32 byte từ cấu hình (đề xuất `VINK_WORK_VIEW_DIGEST_KEY`, 64 chữ số hex qua
`ViewDigestKey::parse_hex`), không vào DB, log hay bundle; (3) `is_speculative` trên header và
chờ `document.prerendering === false` ở client; (4) `INGEST_LIMIT` theo mạng của
`abuse::client_address`, trong bộ nhớ, vượt giới hạn thì bỏ qua lặng lẽ; (5) truyền
`OffsetDateTime::now_utc()`; (6) tab sinh UUIDv7 một lần cho mỗi thân bài đã hiện, retry cùng mã
và kết thúc retry sớm hơn nhiều so với 11 giờ.

#342 đọc: tổng = `work_view_totals.views`, vắng hàng là 0 chỉ khi `work_view_counting` đã có hàng
(chưa có hàng: chưa bắt đầu đếm, không phải “0”); nhãn “Tính từ” khi ngày công khai đầu tiên trước
`started_at`; tổng giữ nguyên khi bài ẩn/rút (VIEW-08). Đọc bằng phiên chủ tác phẩm, `no-store`.

Fixture dùng chung trong
[`tests/fixtures/work-views/`](../../services/vink-app/tests/fixtures/work-views/):
`decisions.json` (28 vector đầu vào → quyết định, gắn E-xx và VIEW-ID) và `scenario.json` (26 bước
dòng thời gian cùng tổng, tổng ngày, số hàng cửa sổ/receipt và mốc bắt đầu cuối cùng). Unit test
chạy cả hai qua quyết định thuần và một store trong bộ nhớ; test PostgreSQL chạy `scenario.json`
qua adapter thật. #343 dùng lại hai file cho HTTP (ánh xạ người đọc sang phiên/cookie); #342 dùng
trạng thái cuối của `scenario.json` làm oracle đọc.

### 7.5 Ranh giới chưa kiểm và phần còn mở

- HTTP thu nhận, cookie khách, bộ giới hạn tần suất, cấu hình khóa và tín hiệu Web: #343.
- API đọc, “0”/“—”/“Tính từ”: #342 (§9). Chưa có số liệu nào được hiển thị trên giao diện.
- [Account Deletion #297](../architecture/account-deletion.md) gọi `erase_work` trong giao dịch
  rút bài. Account giữ WorkId làm tombstone, nên các FK cascade sẽ không chạy. VIEW-11 ở luồng
  xóa tác phẩm riêng vẫn theo owner Publication.
- OQ-3 không đổi luật ghi; OQ-1, OQ-4 thuộc phía đọc.
- Tranh chấp bài nóng chỉ đo local (32 lượt đồng thời), chưa đo tải thật.
- Không có quy trình đổi khóa hai khóa song song.
- Không có mô hình Quint: luật nhỏ, đã có vector và oracle dòng thời gian chạy trên cả model trong
  bộ nhớ lẫn PostgreSQL; lane `verify discussion` không đổi.

### 7.6 Bằng chứng và nghiệm thu

Chạy trên nhánh `feat/341-ghi-luot-xem` (base `develop@d478a06`), Linux, PostgreSQL 17.11 trong
container tạm.

| Phép kiểm | Ranh giới | Kết quả |
| --- | --- | --- |
| `cargo test -p vink-app --lib work_view` | unit | PASS, 19 test |
| `cargo test -p vink-app --test postgres work_view` | postgres | PASS, 15 test |
| `cargo xtask verify unit app` | unit/contract | PASS, 410 test (406 + 4) |
| `cargo xtask verify quick` | fmt, clippy server và wasm, doctest, Reader SSR boundary, unit, repository checks (decisions, links, boundaries) | PASS |
| `cargo xtask verify postgres` | postgres: store 6; App 278 PASS, 7 ignored; Identity 55; Notifications 23; binary thật, backup/restore | PASS |

| AC | Phép kiểm | Kết quả, gap |
| --- | --- | --- |
| AC-01 | 28 vector của E-xx (`shared_decision_vectors`); kiểm tại commit: rút trước → `not_public`, sự kiện giữ khóa pointer trước → `counted` rồi rút (`a_withdrawal_committed_before_the_decision_skips_the_view`, `a_view_decided_before_the_withdrawal_commits_counts`); r2 thay r1 qua evaluator thật giữa cửa sổ (bước 10–11 của `scenario.json`) | PASS unit + postgres |
| AC-02 | Cùng sự kiện ×8 đồng thời → 1; hai tab ×8 → 1; 32 người đọc → 32 và tổng ngày khớp; mất phản hồi + pool mới (khởi động lại) + mã khách mới → gửi lại; lỗi chèn trước commit → không còn dấu vết, gửi lại +1 đúng một lần; oracle dòng thời gian so trên hàng đã commit | PASS postgres |
| AC-03 | Mốc bắt đầu ghi ở sự kiện đầu; bài có sẵn đếm từ sự kiện đầu; Thấm không sinh lượt (`nothing_is_inferred_from_earlier_activity`); dọn hạn (bước 23: 5 cửa sổ, 13 receipt; `retention_skips_a_window_a_decision_holds`); gửi lại sau hết hạn bị bỏ qua (bước 24, 26); xóa theo `WorkId` (`erasing_a_work_removes_only_its_view_rows`); xử lý đồng bộ nên không cần tiêu chí hội tụ trễ | PASS; route xóa còn chờ OQ-2 |
| AC-04 | Hợp đồng §7.4, fixture, giới hạn §7.1 và §7.5 | Tài liệu; HTTP chưa kiểm |

## 8. Điều kiện để bắt đầu việc tiếp theo

- **#341:** đường ghi ở §7. #342 và #343 dùng API, bảng và fixture ở §7.4.
- **#342:** đọc theo quyền ở §9; #343 dùng hợp đồng ở §9.4 để nghiệm thu xuyên suốt.
- **Lượt thiết kế:** dùng hai fixture ở §9.3 làm dữ liệu mẫu có nhãn. Cần câu trả lời OQ-1 trước
  khi chốt hàng Bàn viết của bài riêng tư.
- **#343:** cần thêm cấu hình khóa digest và cookie khách (§7.4) trước khi bật thu nhận.

## 9. Đọc theo quyền (#342)

Nguồn: [#342](https://github.com/loveoverflowcom/vink/issues/342), base
`develop@002cc12e1ca106e9593b0a2c513dd81cb27b5bd2`, đặc tả `VDR-WORK-VIEWS@r1`
(`PRODUCT_CONFIRMED`). Code:
[`domain/work_view/count.rs`](../../services/vink-app/src/domain/work_view/count.rs) (luật đọc
thuần), [`persistence/author_works.rs`](../../services/vink-app/src/persistence/author_works.rs)
(`AUTHOR_VIEWS_SQL`),
[`application/author_works.rs`](../../services/vink-app/src/application/author_works.rs)
(`AuthorWorkSummary.views`), [`http/author_works.rs`](../../services/vink-app/src/http/author_works.rs)
(`WorkViews`) và [hợp đồng HTTP](../../services/vink-app/contracts/http/v1/README.md#lượt-xem-của-tác-giả-342-view-0709-view-12).
Không migration, không route mới, không đổi DTO công khai hay consumer Web.

### 9.1 CHOICE-05 (`DESIGN_PROPOSED`)

Chọn **A**: thêm `views` vào từng thẻ của read-model tác giả `GET /api/v1/author/works`, đọc theo
lô của trang. Merge code không làm lựa chọn này thành `Accepted`.

| Tiêu chí | A: trong read-model tác giả (đã chọn) | B: endpoint summary/batch riêng |
| --- | --- | --- |
| Số câu lệnh | Trang + Thấm/Nhận xét (≤ 1) + Lượt xem (1). Đo bằng sự kiện câu lệnh của sqlx: 7 ở trang 1, 24 và 100; mỗi loại SELECT đúng 1 | Danh sách như cũ, cộng một request HTTP: resolve phiên lần nữa và ít nhất một câu lệnh |
| Lỗi riêng nguồn thống kê | Savepoint riêng: `views` thành `unavailable`, tên, trạng thái và Thấm/Nhận xét giữ nguyên; lỗi Thấm cũng không làm mất Lượt xem | Request lỗi riêng; client tự ghép hai kết quả có thể lệch thời điểm |
| Phạm vi quyền | ID lấy từ chính trang đã lọc theo chủ trong cùng snapshot; câu lệnh lặp lại `owner_id` | Kiểm chủ cho từng `WorkId` client gửi; thêm một bề mặt dò ID |
| Cache, toàn văn | Route vốn `no-store`, không validator; DTO và `ETag` công khai không đổi; danh sách không chứa toàn văn | Như A, thêm một route `no-store` |
| Độ tươi | Mỗi lần tải danh sách đọc số mới | Làm mới số mà không tải danh sách; chưa cần vì VIEW-12 không realtime |

Giới hạn đã biết: representation tác giả rộng hơn (một object mỗi thẻ); số chỉ mới khi tải lại
trang danh sách. Nếu sau này cần làm mới số riêng, B thêm được mà không đổi luật đọc ở §9.2.

### 9.2 Luật đọc

`read_view_count` đọc ba sự thật trong cùng snapshot của trang: tổng (`work_view_totals`), lần
công khai đầu (`work_first_publications`) và mốc bắt đầu đếm (`work_view_counting`). Luật không
nhìn trạng thái publication, nên đổi revision, chuyển riêng tư, ẩn, rút sau duyệt và công khai lại
đều không đổi số (VIEW-06, VIEW-08); không có đường nào suy số từ Thấm/Nhận xét hay log (VIEW-09).

| # | Điều kiện | `state` | `count` | `counted_since` |
| --- | --- | --- | --- | --- |
| 1 | Có tổng nhưng tổng ≤ 0, chưa có mốc, hoặc bài `never_public` (dữ liệu không đường ghi nào tạo ra) | `unavailable` | null | null |
| 2 | Không có tổng, bài `never_public` (L-01) | `never_public` | null | null |
| 3 | Không có tổng, chưa có mốc | `not_started` | null | null |
| 4 | Còn lại | `counted` | tổng, hoặc 0 khi vắng hàng (L-02) | Mốc, trừ khi bài công khai lần đầu (`exact`) tại hoặc sau mốc (L-12, L-13) |
| — | Câu lệnh Lượt xem lỗi | `unavailable` cho mọi thẻ của trang (L-05) | null | null |

- **“Tính từ” cho lịch sử không chính xác.** `legacy` (đã công khai trước khi có bảng lần công
  khai đầu, tức trước mọi mốc) và `unknown` (không chứng minh được) luôn mang mốc: câu “tính từ
  mốc” đúng trong mọi trường hợp. Hệ quả: một bài `unknown` thật ra chưa từng công khai hiện
  `counted` 0 kèm mốc thay vì `never_public`, vì không có bằng chứng phân biệt.
- **Mốc là toàn hệ thống.** Trước sự kiện đầu tiên được quyết định ở bất kỳ tác phẩm nào, mọi bài
  từng công khai là `not_started`. Bước 1 của §5 (“0 thật”) chỉ đúng sau khi mốc đã có.
- **Bài riêng tư** (OQ-1 chưa trả lời): áp P-08 A, thẻ vẫn có số cũ trong khi `current_public` và
  Thấm/Nhận xét vắng.
- **Quyền.** Chủ lấy từ phiên (`Actor`); không nhận owner ID từ client; khách `401`, Identity lỗi
  `503`, đều `no-store`. Đọc của chính tác giả không đi qua standing fence, như phần còn lại của
  read-model (đình chỉ vẫn đọc được bài mình); quyền sở hữu không chuyển được.
- **OQ-2, OQ-4** (ghi lúc #342; hiện trạng ở §13). Lúc đó chưa có route xóa nào. Xóa tài khoản
  tác giả (#550) nay gọi `erase_work`; `OQ-4` đã chốt trong `VDR-WORK-VIEWS@r2` (`PENDING`) và
  vẫn chưa có route admin gỡ cả bài.

### 9.3 Hợp đồng và ví dụ

[Hợp đồng HTTP](../../services/vink-app/contracts/http/v1/README.md#lượt-xem-của-tác-giả-342-view-0709-view-12)
mô tả bốn trạng thái. Hai fixture là dữ liệu mẫu có nhãn cho lượt thiết kế:

- [`author.works.views.json`](../../services/vink-app/contracts/http/v1/fixtures/author.works.views.json):
  sau mốc, gồm “Tính từ” (L-12), 0 thật (L-02), bài riêng tư (L-07) và bài bị rút sau duyệt
  (L-08) giữ số, bài chưa công khai (L-01), lỗi đọc số trong khi Thấm/Nhận xét vẫn có (L-05).
- [`author.works.views-not-started.json`](../../services/vink-app/contracts/http/v1/fixtures/author.works.views-not-started.json):
  trước mốc.

Unit test của provider so serializer với từng fixture. Test PostgreSQL so hình dạng (key, kiểu
giá trị) của phản hồi HTTP sinh từ dữ liệu đường ghi thật với ví dụ cùng trạng thái.

### 9.4 Bàn giao cho #343 và lượt thiết kế

- Nguồn ngưỡng hiển thị: `WORK_VIEW_MIN_VISIBLE_MS` (`5_000 ms`) nằm trong `vink-work-view-policy` (`#343`); giao diện Bàn viết (`#514`) dùng cùng nguồn này cho giải thích `UX-07`.
- Nghiệm thu xuyên suốt §5 đọc số qua `views` của `GET /api/v1/author/works` bằng phiên tác giả:
  bước 1 (`counted`/0 khi mốc đã có, ngược lại `not_started`), 11–13 (giữ qua r2, riêng tư,
  công khai lại), 14 (`unavailable`), 16 (`counted_since`). Ghi đồng bộ (CHOICE-02 A) nên số có
  ngay sau commit; không có trạng thái “chưa hội tụ”.
- Tải lại danh sách là cách duy nhất để thấy số mới; không validator, không `304`. `ETag` của
  Reader, danh sách công khai và `/publication` không phụ thuộc số (§9.5).
- Consumer Web (`apps/vink-web/src/contract/author_works.rs`, `ui::adapters::author_work_summary`, `AuthorWorkRow`, `DeskIndex`) đã nối `views` theo `#514` ([§11](#11-giao-diện-bàn-viết-design-r3-514) và [LayoutSpec Lượt xem r3](../design/work-views.md)): `D-01` (**Thấm → Nhận xét → Lượt xem**), `D-02` (bỏ hẳn nút/link “Đọc” / “Đọc bản công khai”), `UX-01..10` (icon `visibility-outline`, định dạng `1.234`, “—” kèm lý do cho `not_started`/`unavailable` và nhãn ẩn cho `never_public`, “Tính từ dd/mm/yyyy” theo `Asia/Ho_Chi_Minh`, giải thích `#views-explainer` dùng ngưỡng `WorkViewDurationPolicy::DEFAULT`).

### 9.5 Bằng chứng và nghiệm thu

Chạy trên nhánh `feat/342-work-view-reads` (base `develop@002cc12`), Linux, PostgreSQL 17.11
trong container tạm.

| Phép kiểm | Ranh giới | Kết quả |
| --- | --- | --- |
| `cargo test -p vink-app --lib -- author_work work_view::count` | unit/contract | PASS, 4 test |
| `cargo test -p vink-app --test postgres -- work_view author_work --test-threads=4` | postgres + router HTTP | PASS, 24 test, 1 measurement cũ ignored |
| `cargo xtask verify unit app` | unit/contract | PASS, 413 test (409 + 4) |
| `cargo xtask verify postgres` (`RUST_TEST_THREADS=4`) | postgres: store 6; App 283 PASS, 7 ignored; Identity 55; Notifications 23; binary thật, backup/restore | PASS |
| `cargo xtask verify quick` | fmt, clippy server và wasm, doctest, Reader SSR boundary, unit, repository checks (decisions, links, boundaries) | PASS |
| `cargo xtask decision query VDR-WORK-VIEWS` | registry | `current`, r1, `PRODUCT_CONFIRMED`, SHA-256 `cfb789cc…`, snapshot `8a322aed…` không đổi |

| AC | Phép kiểm | Kết quả, gap |
| --- | --- | --- |
| AC-01 | `only_the_owner_reads_views_and_no_public_surface_depends_on_them`: chủ đọc được (kể cả khi gửi `If-None-Match`); thành viên khác không thấy W; khách `401`; Identity lỗi `503`; tất cả `no-store`, không `ETag`. Trạng thái qua thay revision, riêng tư, công khai lại (`shared_timeline_oracle_on_postgres` đọc lại sau từng bước của `scenario.json`), rút sau duyệt qua route ca duyệt thật rồi công khai r2 (`views_survive_review_withdrawal_and_count_on_after_republication`) | PASS postgres. Xóa tác phẩm và admin gỡ cả bài: không có route (OQ-2, OQ-4). API công khai: không có theo VIEW-07 |
| AC-02 | Luật thuần 15 ca (`read_rule_distinguishes_measured_zero_from_no_number`); trên PostgreSQL: `not_started`, `never_public`, “Tính từ”, 0 thật rồi 1 (`author_reads_the_states_the_write_path_produces`); lỗi bảng tổng → `unavailable` mà tên/trạng thái/Thấm còn, lỗi Thấm mà Lượt xem còn, một hàng không nhất quán chỉ mất số của nó (`a_views_failure_is_independent_of_the_rest_of_the_row`) | PASS unit + postgres. Trễ hội tụ: không áp dụng (ghi đồng bộ) |
| AC-03 | Câu lệnh đếm từ sự kiện sqlx: trang 1/24/100 đều 7, mỗi loại SELECT đúng 1 (`views_read_is_one_batch_statement_for_any_page_size`); sau một lượt mới, Reader và danh sách công khai trả `304` với validator cũ, byte Reader không đổi, không JSON/HTML công khai nào có `views`/`counted_since`/“Lượt xem”; chuyển riêng tư thì validator cũ nhận trang ẩn `200` `no-store` | PASS postgres |
| AC-04 | §9.2–§9.4, hợp đồng HTTP, hai fixture; unit so serializer với fixture; PostgreSQL so hình dạng phản hồi thật với ví dụ cùng trạng thái | PASS unit + postgres. Chưa có browser vì không đổi Web |

## 10. Thu nhận HTTP và tín hiệu Reader (#343 · PR1)

Nguồn: [#343](https://github.com/loveoverflowcom/vink/issues/343) và delta `#340` ngày 25/11/2026.

### 10.1 Lựa chọn kỹ thuật (`DESIGN_PROPOSED`)

Merge code không làm các lựa chọn dưới đây thành `Accepted`.

| CHOICE | Lựa chọn đã chọn | Lý do | Giới hạn đã biết |
| --- | --- | --- | --- |
| **CHOICE-POLICY** | **A — crate thuần dùng chung `packages/work-view-policy` (`vink-work-view-policy`)** chứa hằng số typed duy nhất `WORK_VIEW_MIN_VISIBLE_MS = 5_000` (`WorkViewDurationPolicy::DEFAULT`), validator `WorkViewDurationPolicy::from_millis(...)` và cửa sổ `VIEW_WINDOW_HOURS = 24`. Crate không chứa câu chữ: `vink-web` (CSR standalone lẫn SSR/hydrate) dùng ngưỡng cho timer Reader và in cả hai giá trị trong giải thích `UX-07` (`views_explainer_lead_vi`); `vink-app` dẫn `VIEW_WINDOW` từ `VIEW_WINDOW_HOURS` và không nhận ngưỡng phía trình duyệt. Đổi một giá trị chỉ sửa 1 dòng trong `packages/work-view-policy/src/lib.rs` rồi build lại workspace; test domain App ghim `VIEW_WINDOW` đúng 24 giờ theo `VDR-WORK-VIEWS@r1`. | Không thêm HTTP call hay trường JSON vào `/reader` → giữ nguyên `ETag`/`304` (`READ-01`/`READ-02`), không tăng độ trễ mở bài, không lệch giữa CSR standalone và SSR, không cần service/DB cấu hình riêng. Khi rolling deploy nếu client cũ giữ ngưỡng 5s còn API mới vẫn nhận `POST /views` với `event_id` hợp lệ thì tương thích ngược hoàn toàn. | Đổi ngưỡng cần rebuild workspace; browser không tin tham số thời lượng do client tự khai làm bằng chứng server. |
| **CHOICE-01** | **Tách state machine thuần (`apps/vink-web/src/work_view.rs::ViewVisitTracker`) nối tại `ReaderLoaded` / `PublicationView` trong `apps/vink-web/src/pages/mod.rs`.** | Cho phép unit test thuần đầy đủ trên đồng hồ monotonic giả lập (ngưỡng, `< threshold`, ẩn/hiện, `OQ-3A/B`, hủy đổi tác phẩm/revision/`Session`), trong khi UI chỉ khởi tạo một `Timeout` thích ứng (không polling/heartbeat định kỳ) khi `PublicationView` thực sự render thân bài công khai ở tab visible & `!document.prerendering`. | Phụ thuộc API `visibilityState`/`prerendering`/`performance.now` của trình duyệt (browser tắt JS không đếm theo `VIEW-02`). |
| **CHOICE-04** | **Retry hữu hạn trong tab (`RetryPlan`: tối đa 3 lần retry = 4 lần gửi, backoff `1s → 3s → 9s`) với cùng UUIDv7 `event_id`.** | Không dựng hàng đợi bền qua đóng tab; thử lại khi lỗi mạng/`5xx`/`503` (bao gồm lúc `IdentityResolver` trả `Unavailable`), dừng ngay khi `204`, `4xx` (`403`/`404`/`422`), unmount, đổi bài/revision, ẩn/rút hoặc đổi phiên; thất bại không bao giờ hiện lên UI Reader hay chặn điều hướng. | Đóng tab hoặc offline quá cửa sổ ~13s sẽ mất lượt (`VIEW-10` chấp nhận), nhưng không bao giờ cộng trùng vì cùng `event_id`. |

### 10.2 Cấu hình và định danh HTTP (`CHOICE-03`, `CHOICE-06`)

- **`POST /api/v1/works/{work_id}/views`**: nhận JSON `{"event_id":"<UUIDv7>"}`, ép `require_same_origin`, trả `Cache-Control: no-store`.
- **`VINK_WORK_VIEW_DIGEST_KEY`** (32 byte = 64 chữ số hex qua `ViewDigestKey::parse_hex`):
  - Nếu có giá trị nhưng sai định dạng → App thoát mã `2` (`ConfigError`) ngay lúc khởi động.
  - Ở `VINK_PROFILE=local`: nếu bỏ trống, dùng khóa phát triển cố định nội bộ (không bao giờ lọt vào bundle client hay log).
  - Ở `VINK_PROFILE=production`: nếu bỏ trống, `Config.work_view_digest_key` là `None` và `POST /api/v1/works/{work_id}/views` **fail-closed `503 dependency_unavailable`** thay vì dùng khóa yếu cố định (giữ nguyên tương thích boot với các manifest hiện hữu trước khi operator cấp secret).
- **Identity & Cookie khách:** `Resolution::Principal(id)` và `Resolution::Suspended(id)` đều dùng `Viewer::Member(id)` (khớp CHOICE-03 §7.1 và `VIEW-05`: thành viên bị đình chỉ vẫn chống trùng 24 giờ theo `PrincipalId` mà không cấp cookie khách, và tác giả bị đình chỉ vẫn không tự đếm lượt bài mình). Khi `IdentityResolver` trả `Resolution::Unavailable` → trả `503 dependency_unavailable` để không đếm nhầm thành viên/tác giả thành khách. Khách hoặc session không có/chưa gắn principal dùng cookie `__Host-vink_guest` (UUIDv4, `Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age=86400`, cố định theo `GUEST_TOKEN_TTL`, **không gia hạn** khi còn hạn; thiếu, trùng nhiều header hoặc hỏng mới cấp cookie mới kèm `Set-Cookie` khi ghi thành công/204).
- **Speculative & Rate limit:** `is_speculative` (`Sec-Purpose`/`Purpose`/`X-Purpose`) trả `204` trước khi đụng DB hay cấp cookie; vượt `INGEST_LIMIT` (120 req / 600s / `Network` trong bộ nhớ tiến trình theo `abuse::client_address`) trả `204` lặng lẽ không đụng DB.
- **Ẩn trạng thái riêng tư:** mọi `Ok(_)` (`Counted`, `Skipped(_)`, `Replayed(_)`, `Ignored(_)`, bao gồm `NotPublic` và `WorkNotFound`) đều trả cùng `204 No Content` + `no-store` (+ `Set-Cookie` đồng nhất nếu cần cấp cookie khách mới) để không tạo existence oracle.

## 11. Giao diện Bàn viết Design r3 (#514)

Theo chỉ dẫn trực tiếp của chủ sản phẩm ngày 25/11/2026, phần giao diện Bàn viết (`#514`) được ghép thêm commit lên cùng PR `#519` trên nhánh `feat/343-work-views-intake` (không nâng `VDR-WORK-VIEWS@r2` khỏi `PENDING`, không nâng `CHOICE-*` khỏi `DESIGN_PROPOSED`, giữ `DQ-05` và `DQ-10` ở trạng thái `OPEN`).

### 11.1 Thay đổi chính theo r3

- **Icon & token:** bổ sung `visibility-outline` từ `google/material-design-icons@10ecc14790612ca8fd164e3f4ffe9c16fc50d5b7` vào [`icons.json`](../../packages/ui-system/shared/icons.json) (đối chiếu SHA-256) và tái sinh `icons.rs`/`VinkIcons.kt` bằng `cargo xtask ui-tokens`; bổ sung helper `work_view_date_hcm` (`Asia/Ho_Chi_Minh`) trong [`timestamp.rs`](../../packages/vink-patterns/leptos/src/timestamp.rs) và `format_vi_count` trong [`vink-patterns`](../../packages/vink-patterns/leptos/src/lib.rs).
- **Hàng tác phẩm `AuthorWorkRow` (`D-01`/`D-02`):** giữ thứ tự cố định **Thấm → Nhận xét → Lượt xem** (`pen-flight` 22px, `chat-bubble-outline` 20px, `visibility-outline` 20px), bỏ hoàn toàn nút/link “Đọc” / “Đọc bản công khai”, vùng giữ chỗ và tab stop cũ (`.vk-author-work__reader`); nối API thật cho 4 trạng thái `counted` (gồm `0` và `Tính từ dd/mm/yyyy`), `not_started` (`— Chưa bắt đầu thống kê`), `never_public` (ẩn khỏi dòng số liệu, giữ nhãn ẩn screen reader) và `unavailable` (`— Chưa tải được`); giữ số lịch sử (`OQ-1 = A`) ở bài riêng tư/đã ẩn sau kiểm duyệt; dùng tonal chọn lọc cho nhãn trạng thái và nâng cấp nút `Xem kiểm duyệt` sang `ButtonVariant::Tonal` + icon `verified-user`.
- **Giải thích độ phủ (`UX-07`):** thêm nút `#views-explainer-trigger` (chỉ hiện khi danh sách có tác phẩm không ở `never_public`) và sheet `#views-explainer` (`DialogKind::Sheet`, trả focus về trigger khi đóng/Escape/Back), đọc ngưỡng (`WorkViewDurationPolicy::DEFAULT`) và cửa sổ (`VIEW_WINDOW_HOURS`) từ `vink-work-view-policy` để giữ một nguồn cấu hình duy nhất; câu chữ nằm ở `apps/vink-web/src/work_view.rs`.

### 11.2 Bằng chứng đơn vị, E2E và tự kiểm UI năm chiều

Đây là bằng chứng của lần bàn giao trước review độc lập. Xem §12 để biết kết quả ở revision tích hợp.

| Phép kiểm | Ranh giới | Kết quả thực tế |
| --- | --- | --- |
| `cargo test --locked -p vink-patterns --lib` | unit/SSR (`vink-patterns`) | PASS, 22/22 test (`format_vi_count`, `work_view_date_hcm`, `AuthorWorkRow` với thứ tự D-01, bỏ Đọc D-02, 4 trạng thái views, lịch sử `OQ-1=A`) |
| `cargo xtask verify unit web --filter author_work` | unit/contract (`vink-web` SSR) | PASS, 3/3 test (fixtures `author.works.views*.json`, enum lạ, count âm, `views = None`) |
| `cargo xtask verify unit ui` | unit (`vink-ui`) | PASS, 70/70 test |
| `cargo xtask verify web --scenario desk --capture full` | Chrome + App + PG + Tailwind | PASS toàn bộ scenario `desk` tại run `1795597787117725000-33327`. Sau sửa 1 dòng CSS ở `#views-explainer-trigger` (`margin-inline-start`), pha `authorWorks` PASS (gồm assertion 1 dòng 44–48px tại 1440px, 24 ma trận `320/390/834/1440 × light/dark × 100/150/200%`, mở/đóng sheet `#views-explainer`) tại run `1795599062264813000-43827`; `draftDashboard` cùng run gặp timeout hydration dưới tải swap. |
| `cargo xtask verify web --scenario desk --scenario reader --scenario standalone --capture checks` | Chrome đầy đủ | **NOT_RUN** (đã dừng tác vụ nền để bàn giao theo yêu cầu người dùng). |

| Chiều UX | Kết quả / giới hạn |
| --- | --- |
| Functional | **PASS** unit/SSR/contract (`vink-patterns` 22/22, `author_work` 3/3) và Chrome `authorWorks` trước/sau sửa trigger; ca tích hợp end-to-end trong `work-views-smoke.mjs` (`reader` + `standalone`) cần chạy lại ở phiên review. |
| Geometry | **PASS** 24 cấu hình trong `authorWorks` (`320/390/834/1440 × light/dark × text100/150/200%`): không tràn ngang, card bo 20px khe 8px, icon 22/20/20px, trigger `#views-explainer-trigger` 44–48px một dòng ở 1440px và nút `Xem kiểm duyệt` nằm trên bottom bar tại 390/text200%. |
| Visual | **SCREENSHOT_INSPECTED** (mở trực tiếp PNG từ run `1795599062264813000-43827` đối chiếu bộ r3 `DESIGN_SPEC` / `LAYOUT_SPEC`): `1440` sáng/tối, `390` sáng, `320/150%` sáng, `320/200%` tối, `no-draft 390/200%` tối; kết hợp kiểm sheet `#views-explainer` `1440` sáng và `390` tối ở run `1795597787117725000-33327`. Giữ `DESIGN_PROPOSED`; giữ `DQ-05` (`<Timestamp>` đầy đủ) và `DQ-10` (secondary-container, không dùng tertiary) đang `OPEN`. |
| Accessibility/Responsive | **PASS** tương phản CSS token từ runner `desk` (tối thiểu ≥ 4,5), focus title trên sheet `#views-explainer`, Escape/Close trả focus về `#views-explainer-trigger`, văn bản ẩn screen reader cho `never_public` và nhãn trạng thái. Screen reader có tên, OS touch/mobile thật, zoom trình duyệt thật: **NOT_RUN**. |
| Interaction | **PASS** mở/đóng sheet `#views-explainer` bằng phím Enter/Escape và click, refresh thay đồng bộ `publication` + `views` không cướp focus khỏi `#draft-search`, 503 giữ snapshot stale, 403 xóa dữ kiện riêng. OS IME thật: **NOT_RUN**. |

## 12. Đối soát độc lập PR #519

Rebase `07a3aa8d` lên `origin/develop@4af0733f`. Xung đột ở bản đồ tính năng được giải bằng
cách giữ các mục Tài khoản #523/#524 và mục Bàn viết r3. Các bản vá review hủy lượt ngay khi
nhận tín hiệu đổi phiên, giữ nhãn và số liệu trong hàng ở 320 px/chữ 200%, tìm đúng tác phẩm
mẫu khi Desk đã tạo 250 bản thảo và bỏ kết quả hồ sơ khi vùng Tài khoản bị hủy. Phép thử
standalone dùng lối Bàn viết trên điều hướng chính của phiên bản tích hợp.

| Phép kiểm | Revision và kết quả |
| --- | --- |
| `python3 ./tools/local-budget run -- cargo xtask verify postgres app --filter work_view` | **PASS 22/22** trên `0eed558c9692726efd2abdb68c1296d95ed0d374`, macOS arm64, PostgreSQL 17.11 tạm trên loopback. Bao gồm phiên đình chỉ, chống trùng, quyền tác giả, rollback, phiên bản công khai và ETag. DB, HTTP intake và fixture PostgreSQL giữ nguyên qua các bản vá Web. |
| `cargo fmt --all -- --check`, `cargo xtask verify static tokens`, `cargo xtask verify static repo` | **PASS** tại `96fbf0fb`, 16 file sinh khớp. Marker r1 active và signoff r1 giữ nguyên. |
| `cargo xtask verify web --scenario desk --scenario reader --scenario standalone --capture checks` | Run `1795603257900796000-47616` tại `6057b8f1`: **PASS** `desk`, `desk-guest`, `reader`, `reader-views` và ca 10 đọc lại 3 lượt từ PostgreSQL trên Bàn viết sau kịch bản 250 nháp. `standalone` **FAIL** khi fixture còn trỏ vào nút Bàn viết cũ. |
| `cargo xtask verify web --scenario reader --capture checks` | Run `1795604497850188000-51942` tại `96fbf0fbc7ce08dce4160495bbd1150eb98aeec9`, Chrome 158.0.8127.89 và PostgreSQL tạm. **PASS** `reader` và pha `reader-views`, gồm ca 10 đọc 3 lượt thật lên Bàn viết. |
| `cargo xtask verify web --scenario standalone --capture checks` | Run `1795604387699622000-51303` trên cùng `96fbf0fb`. **PASS** CSR với App/Identity thật, E-41 và kiểm tra console sạch. |
| PNG Bàn viết, `--capture full` | Run `1795602810495643000-45926` tại `1c0dd1cd`. Đã mở ảnh Bàn viết 1440 sáng/100%, 390 tối/100%, 320 và 390 tối/200%, sheet 1440 sáng và 390 tối. Các thay đổi sau run nằm ở fixture E2E và callback Hồ sơ Tài khoản. |

Lượt gộp `reader` và `standalone` đầu tiên trên `96fbf0fb` mất phản hồi DevTools trong 30 giây tại `reader-views`. Hai lượt chạy riêng ở trên hoàn tất trên cùng SHA.

| Chiều UX | Kết quả và giới hạn |
| --- | --- |
| Functional | **PASS** trên Bàn viết SSR/hydration. Ca 10 đọc số thật sau Reader. 4 trạng thái và số lịch sử được kiểm qua các phản hồi mẫu trên bề mặt Bàn viết thật. |
| Geometry | **PASS 30/30** ở 320/390/834/1024/1440 px × sáng/tối × cỡ chữ 100/150/200%. Phép đo kiểm tràn của trang và từng thẻ, trạng thái và hàng số liệu. |
| Visual | **PASS** đối chiếu [bộ ảnh r3](https://drive.google.com/drive/folders/1QtovsyJfBO-98GJXGb_2ysR4Wa6trd_3). Bố cục hàng, thứ tự số liệu, nhãn chờ duyệt secondary và nút tonal khớp LayoutSpec. Mốc tham chiếu `fidelity: NO_APPROVED_REFERENCE`. |
| Accessibility/Responsive | **PASS** reflow chữ lớn, cây DOM có nhãn ẩn cho `never_public`, focus hiện và tương phản Desk qua runner. Trình đọc màn hình, chạm trên thiết bị thật và zoom hệ điều hành **NOT_RUN**. |
| Interaction | **PASS** Enter mở sheet, Escape/Đóng/Back trả focus, xác nhận cả sau khung hình Back. E-41 dùng phiên thật bị thu hồi và giữ phản hồi đọc lại để kiểm tra hủy lượt trước khi phiên xác nhận lại. |

`VDR-WORK-VIEWS@r1` vẫn active và `PRODUCT_CONFIRMED`. `VDR-WORK-VIEWS@r2`
`PENDING`, CHOICE và LayoutSpec r3 `DESIGN_PROPOSED`, `DQ-05` và `DQ-10` `OPEN`.
Phạm vi triển khai còn lại lúc đó gồm các route xóa của OQ-2 và gỡ cả bài của OQ-4. §13 ghi
hiện trạng sau #550.

## 13. Vòng đời và xóa tài khoản (OQ-1/OQ-2/OQ-4)

Nguồn: [Epic #323](https://github.com/loveoverflowcom/vink/issues/323), `#340`–`#342`, liên quan
[#297](https://github.com/loveoverflowcom/vink/issues/297) /
[PR #550](https://github.com/loveoverflowcom/vink/pull/550) (xóa tài khoản) và
[PR #519](https://github.com/loveoverflowcom/vink/pull/519) (`#343` + `#514`). Base
`develop@91191186`. Đặc tả áp dụng là `VDR-WORK-VIEWS@r2` (`PENDING`); `@r1` vẫn active. Lượt này
không đổi luật sản phẩm, schema, HTTP, Web, chính sách 5 giây hay 24 giờ.

### 13.1 Kết quả audit vòng đời

`record_view` chỉ đếm khi có pointer `publications` tại lúc quyết định, khóa pointer `FOR SHARE`,
rồi khóa principal của chủ `FOR KEY SHARE` và đọc `account_deletions`. Luật đọc (§9.2) không nhìn
lifecycle. Bảng dưới đối chiếu từng đường với code và test hiện có.

| Đường | Kết quả mong đợi | Code | Bằng chứng PostgreSQL |
| --- | --- | --- | --- |
| Công khai → riêng tư (`set-private`, L-07) | Giữ số cũ (`OQ-1 = A`), +0 khi riêng tư | `PointerEffect::Clear` → `Skipped(not_public)` | `shared_timeline_oracle_on_postgres`, `a_withdrawal_committed_before_the_decision_skips_the_view`, `a_view_decided_before_the_withdrawal_commits_counts`, `http_view_intake_drives_guest_member_dedup_retry_privacy_and_author_readback` |
| Riêng tư → công khai (L-09) | Đếm tiếp từ số cũ; người có lượt trong 24 giờ vẫn bị chặn | Pointer đặt lại, cửa sổ theo `WorkId` | Các bước `set_private`/`restore` của `scenario.json`; bước 10 của test HTTP intake |
| Rút sau kiểm duyệt (`O-17@r2`, L-08) | Giữ số, +0 | Pointer bị xóa qua route ca duyệt | `views_survive_review_withdrawal_and_count_on_after_republication`; bước 10 HTTP intake |
| Đăng lại với revision mới (L-06) | Cộng dồn theo `WorkId`, không reset | Receipt ghi `basis_revision_id`, tổng theo `WorkId` | Bước 10–11 của `scenario.json` (r2 thay r1 qua evaluator thật) |
| Admin gỡ cả bài (`PUB-05 C`, L-14, `OQ-4`) | Giữ số riêng tư, +0 | Core có `apply_admin_remove`; **không có route** | `CONDITIONAL_DEFERRED`: owner Publication/Moderation. Route nào đưa vào runtime phải giữ hàng Lượt xem và thêm test đọc lại |
| Xóa tài khoản tác giả (L-11, `OQ-2`) | Xóa cả bốn bảng của mọi tác phẩm; không lượt nào tái tạo dữ liệu | `apply_account_deletion` → `retire_owned_work` → `erase_work`; `record_view` đọc fence | Test của #550 và năm test mới ở §13.2 |
| Xóa tài khoản người đọc | Không trừ tổng; cửa sổ băm tự hết hạn | Không hàng nào lưu theo người | `a_reader_account_deletion_keeps_the_counts_it_contributed` |
| Tác giả xóa tác phẩm (L-10, `PUB-04`) | Xóa số liệu trong cùng giao dịch | **Không có route** | `CONDITIONAL_DEFERRED`: owner Publication. Route đó phải gọi `erase_work` (hoặc xóa hàng `works`, cascade) kèm test |

Không thấy gap hành vi nào trong code. Gap có bằng chứng là thiếu test cho race giữa intake và
xóa tài khoản, cho rollback/retry khi xóa lỗi sau `erase_work` và cho tài khoản người đọc. Doc
comment của `erase_work` mô tả thiếu: pointer lock chỉ phủ bài công khai, bài riêng tư dựa vào
fence principal. Lượt này thêm test và sửa doc comment, không đổi lock hay transaction.

### 13.2 Thứ tự khóa khi xóa tài khoản

| Bước | Quyết định lượt xem (`record_view`) | Xóa tài khoản (`apply_account_deletion`) |
| --- | --- | --- |
| 1 | `publications` `FOR SHARE` | Advisory lock theo subject |
| 2 | Principal chủ `FOR KEY SHARE`, rồi đọc `account_deletions` | Mỗi tác phẩm: `publications` `FOR UPDATE`, `work_publications` `FOR UPDATE`, `works` `FOR SHARE` |
| 3 | Advisory lock theo sự kiện, receipt, cửa sổ `FOR UPDATE` | Principal `FOR UPDATE` (`locked_standing`) |
| 4 | Ghi theo quyết định, commit | `account_deletions`, Social fence, `retire_owned_work` (`erase_work`, xóa pointer), tombstone principal, commit |

Hai bên lấy `publications` rồi principal theo cùng thứ tự, nên không có deadlock. Bài công khai:
lượt giữ pointer trước thì xóa chờ ở bước 2 và xóa luôn hàng vừa ghi; xóa giữ pointer trước thì
lượt chờ, thấy pointer đã mất và fence đã commit, nên không ghi gì. Bài riêng tư không có hàng
pointer: `FOR KEY SHARE` xung đột với `FOR UPDATE` của `locked_standing`, nên receipt `not_public`
commit trước bước 3 của xóa và bị xóa theo. `lock_timeout = 2s` của xóa tài khoản giữ nguyên;
một lượt giữ khóa lâu hơn làm lần xóa đó trả `Unavailable` và retry.

| Test (`tests/postgres/work_view_lifecycle.rs`) | Thứ tự | Kết quả kiểm |
| --- | --- | --- |
| `a_public_view_decided_first_is_erased_by_the_author_deletion` | Lượt giữ pointer → xóa chờ | Lượt `Counted`; sau xóa bốn bảng 0; gửi lại cùng mã → `work_not_found`, vẫn 0 |
| `a_private_view_decided_first_leaves_no_receipt_after_the_author_deletion` | Bài riêng tư giữ số cũ (`OQ-1 = A`); lượt giữ fence → xóa chờ | Lượt `Skipped(not_public)`; sau xóa bốn bảng 0 |
| `a_view_waiting_behind_the_author_deletion_writes_nothing` | Xóa dừng sau `erase_work` → lượt mới và lượt gửi lại chờ | Cả hai `work_not_found`; bốn bảng 0; HTTP trả `204` + `no-store` như mọi `Ok` và không ghi gì |
| `a_failed_author_deletion_keeps_views_and_its_retry_erases_them` | Lỗi chèn sau `erase_work` | Rollback giữ fence và số liệu (vẫn đếm được); retry xóa một lần; replay không đổi gì |
| `a_reader_account_deletion_keeps_the_counts_it_contributed` | Xóa tài khoản người đọc | Tổng, tổng ngày và số tác giả đọc giữ nguyên; cửa sổ và receipt chỉ rời khi dọn hạn |

### 13.3 Cách đọc trạng thái cho tác giả

| Trạng thái | Khi nào | Bảo đảm |
| --- | --- | --- |
| `counted` 0 | Mốc đếm đã có, bài từng công khai, chưa có tổng | Chỉ trạng thái này hiện “0” |
| `counted` > 0 khi bài riêng tư/rút | Số lịch sử (`OQ-1 = A`) | Không tăng khi không có pointer |
| `not_started` | Chưa có sự kiện nào được quyết định trên hệ thống | Không phải 0 |
| `never_public` | Bài chưa từng công khai | Không phải 0 |
| `unavailable` | Lỗi nguồn số liệu hoặc dữ liệu không nhất quán | Tên, trạng thái, Thấm/Nhận xét còn |

Chỉ phiên chủ tác phẩm đọc qua `GET /api/v1/author/works` (`no-store`); khách `401`, người khác
không thấy tác phẩm. Ghi một lượt không đổi byte hay `ETag` của Reader, danh sách công khai và
`/publication` (`only_the_owner_reads_views_and_no_public_surface_depends_on_them`). Lượt này không
đổi đường đọc.

### 13.4 Bằng chứng

Chạy trên nhánh `fix/work-views-lifecycle` (base `develop@91191186`), Linux x86_64 (không phải
Mac mini như prompt ghi), PostgreSQL `17.11-alpine` tạm trên loopback, build trong scope giới hạn
RAM 12 GB. Regression cuối chạy trên commit `e9b4bbfc`; commit sau chỉ thêm dòng kết quả này.

| Lệnh | Ranh giới | Kết quả |
| --- | --- | --- |
| `cargo test -p vink-app --test postgres -- work_view account_deletion --test-threads=4` | postgres + router Axum | **PASS 29/29** (5 test mới, 24 cũ của `#341`–`#343` và `#550`) |
| Đối chứng âm 1: tạm bỏ nhánh `account_deletions` trong `record_view` | postgres | **FAIL đúng** 4 test: 3 test mới và test của `#550` |
| Đối chứng âm 2: tạm bỏ `FOR KEY SHARE OF p` | postgres | **FAIL đúng** `a_private_view_decided_first_leaves_no_receipt_after_the_author_deletion` (xóa không còn chờ lượt) |
| `cargo fmt --all -- --check`, `cargo clippy -p vink-app --test postgres -- -D warnings` | format, lint | PASS |
| `cargo xtask decision validate` | registry | PASS, 156 revision, snapshot `eb01b0eb95e1d0247f37215b62f7273388ffbfd063977d675bd540e1b858e541` |
| `cargo xtask decision query VDR-WORK-VIEWS --history` | registry | `r1` `PRODUCT_CONFIRMED` `cfb789cc…`; `r2` `PENDING` `f6a3bcb7b8d27b188d18878e68efa1c04bb776ccaf0a06c3f27c57aded904a60` |
| `cargo xtask verify changed --base 91191186 --head e9b4bbfc --expect 7675449f62ed26eb` (`RUST_TEST_THREADS=4`) | static docs, check app ×3, unit app (446) + `document_budget` + `ssr_boundary`, unit notifications, unit web, postgres app (319 PASS, 7 ignored) | **PASS** 10/10 check đã chọn; không phải PASS toàn lane hay merge gate |

Không chạy Kani/Quint: không đổi luật thuần `decide_view`/`read_view_count` hay mô hình nào.
Không chạy browser: không đổi Web.

### 13.5 Bản đồ AC-07/AC-08 của #323

| AC | Đã có | Còn thiếu |
| --- | --- | --- |
| AC-07 | Luật và delta ở PRD §17, đặc tả §4/§7, `VDR-WORK-VIEWS@r2` (`PENDING`, `f6a3bcb7…`); `r1` và signoff giữ nguyên; ngưỡng một owner (`vink-work-view-policy`, §10); fixture và hợp đồng (§9.3, §10.2); lượt này đồng bộ L-10/L-11/L-14, §5, §9, §11 và hợp đồng intake với #550 | **Signoff `r2`** đúng revision/SHA-256 theo `VDR-DECISION-SIGNOFF@r1` (§13.6) |
| AC-08 | Chrome + PostgreSQL thật ở PR #519 (§12): dưới ngưỡng, đúng ngưỡng, ẩn/tạm dừng, hủy điều hướng, prefetch/SSR/hydration, khách/thành viên/tự xem, 24 giờ, quyền, `503` retry cùng `event_id`, CSR standalone E-41. Mất phản hồi sau commit ở server: `lost_answer_and_restart_replay_without_effect` và replay HTTP | **Mất phản hồi sau commit ở ranh giới trình duyệt** (`NOT_RUN`: smoke chỉ có `503` trước commit). Cần một pha `work-views-smoke.mjs` chặn phản hồi `204` sau khi server đã ghi |

### 13.6 Chuẩn bị signoff `VDR-WORK-VIEWS@r2`

Lượt này không sửa payload `r2` nên hash giữ nguyên. Các câu chốt 25/11 trên #323/#340 là lời
chốt sản phẩm, chưa phải signoff payload. Câu hỏi cần gửi chủ sản phẩm, nguyên văn:

> Bạn duyệt `VDR-WORK-VIEWS@r2` (SHA-256 `f6a3bcb7b8d27b188d18878e68efa1c04bb776ccaf0a06c3f27c57aded904a60`,
> `decisions/proposals/VDR-WORK-VIEWS.r2.toml`) để tôi ghi signoff không?

Chỉ khi có câu trả lời của chính chủ sản phẩm cho đúng câu này mới chạy
`cargo xtask decision signoff VDR-WORK-VIEWS@r2 f6a3bcb7… <INPUT>` và trích nguyên văn câu đó.
Tới lúc đó `r2` giữ `PENDING`, marker `active VDR-WORK-VIEWS@r1` giữ nguyên.

### 13.7 Phần còn mở tại PR #611

| Mục | Trạng thái | Owner |
| --- | --- | --- |
| Route tác giả xóa tác phẩm (L-10, `PUB-04`) | `CONDITIONAL_DEFERRED`: PR đầu tiên đưa route vào phải gọi `erase_work` (hoặc xóa `works`) trong giao dịch và có test như §13.2 | Publication |
| Admin gỡ cả bài (L-14, `PUB-05 C`, `OQ-4`) | `CONDITIONAL_DEFERRED`: route phải giữ hàng Lượt xem, +0, và có test đọc lại | Publication / Moderation |
| Signoff `r2` | `PENDING` | Chủ sản phẩm |
| Mất phản hồi sau commit ở trình duyệt (AC-08) | `NOT_RUN` | Web (`#343` owner) |
| `O-08` purge vật lý | `OPEN`, tách khỏi OQ-2 | Publication |

## 14. Nghiệm thu Web của Epic #323 trên Mac mini

Bản đối chứng là `develop@95dcca81` (đã có [PR #611](https://github.com/loveoverflowcom/vink/pull/611)).
Cùng kịch bản Chrome với PostgreSQL 17.11 trên loopback đã chỉ ra hai lỗi. Ở CSR, timer đầu tiên
được đặt khi thân bài còn chưa gắn vào DOM. Ở Bàn viết, sau tín hiệu đổi cookie và trước khi
phiên mới được xác nhận, tác phẩm của tài khoản trước vẫn hiện. Bản sửa `e07f5cdd` bắt đầu tính
giờ trong effect sau render và khóa danh sách theo phạm vi phiên đã xác minh. Ca kiểm ngưỡng
trong trình duyệt ở `acfead02`.

### Kịch bản và kết quả

- **Thời gian đọc và điều hướng.** Chrome đi từ Khám phá bằng chuột, Enter và sự kiện chạm CDP,
  mở URL trực tiếp và URL chia sẻ trong tab thứ hai, tải lại, Back và Forward. Trong cùng cookie
  khách, 5 lần mở đủ ngưỡng tạo 5 mã khác nhau và tổng giữ nguyên trong cửa sổ 24 giờ. Với
  đồng hồ monotonic và cờ prerender điều khiển ở fixture, 4.999 ms sau kích hoạt có 0 request,
  5.000 ms có 1 ứng viên. Các ca ẩn/hiện, đổi revision, ẩn → công khai nền và đổi phiên chạy
  qua cùng tracker. CSR kiểm đổi WorkId bằng link fixture trong cùng document sang trang ẩn thật.
  [Điều hướng](../../apps/vink-web/e2e/work-view-navigation.mjs) · [Vòng đời](../../apps/vink-web/e2e/work-views-smoke.mjs)
- **Mất ACK sau commit.** CDP giữ phản hồi thật ở pha Response. Trước khi làm rớt `204`,
  API tác giả đọc được +1. Lần gửi lại dùng đúng UUIDv7. Với hai tài khoản Identity độc lập,
  tác giả đọc 1 trên API và Bàn viết, PostgreSQL có đúng 1 lượt và 1 receipt sau retry.
  Bốn lần làm rớt liên tiếp cho đúng 4 POST cùng mã, sau đó không còn POST trong 9,5 giây quan sát.
  Một ca riêng trả `503` trước ghi để kiểm timer backoff đã được đặt và được gỡ khi rời Reader.
  [HTTP intake](../../services/vink-app/src/http/work_views.rs) · [Hai phiên thật](../../apps/vink-web/e2e/work-view-author.mjs)
- **Quyền và số liệu.** Chỉ tác giả thấy số. Sau thao tác chuyển riêng tư qua API thật,
  hàng trên Bàn viết giữ số cũ. Ca đổi tài khoản giữ phản hồi thành công của danh sách cũ và
  trì hoãn xác nhận phiên mới. Hàng cũ biến mất ngay khi nhận tín hiệu và không hiện lại sau
  khi giải phóng phản hồi. Khách nhận `401`, tài khoản khác nhận danh sách của chính mình.
  Validator công khai tiếp tục cho `304`. Scenario Desk bao phủ 0, chưa bắt đầu, chưa từng công
  khai, lịch sử và lỗi số liệu bằng fixture có nhãn. Sheet giải thích 5 giây, 24 giờ, khách,
  tự xem và no-JS có thao tác chạm CDP, Enter, Escape và trả focus.
  [Bàn viết](../../apps/vink-web/src/pages/desk.rs) · [Ma trận trạng thái](../../apps/vink-web/e2e/author-works-smoke.mjs)

### Kiểm chứng cục bộ

Mac mini `Mac16,10` (arm64), Chrome `158.0.8127.89`, PostgreSQL `17.11`. App và Identity dùng hai DB riêng trên loopback. Runtime ở `e07f5cdd`, probe cuối ở `acfead02`. Run đối chứng `1795784525688374000-28014` ghi được hai lỗi nêu trên. Run tích hợp sau sửa `1795785124568746000-31560` đạt cả năm pha. Các lệnh Cargo ở bảng giữ nguyên bộ lọc dùng khi chạy.

| Lệnh | Boundary và kết quả |
| --- | --- |
| `cargo xtask verify web --scenario reader --scenario standalone --scenario desk --capture checks` | **PASS** cho `reader`, `reader-views`, `desk`, `desk-guest`, `standalone`. SSR/hydration, CSR standalone, App, Identity và PostgreSQL thật. |
| `cargo xtask verify unit web --filter work_view` | **8/8** gồm ngưỡng, retry, thay quyền và DTO số liệu. |
| `cargo test --locked -p vink-work-view-policy` | **3/3**. Giá trị override đổi ngưỡng và thời lượng cùng lúc; câu giải thích kiểm ở unit `vink-web` (`work_view`). |
| `cargo xtask verify postgres app --filter work_view` | **27/27** giao dịch, HTTP, quyền đọc, dedup, retention và vòng đời. Xóa tài khoản xem phép chạy riêng ở §13.4. |
| `cargo xtask decision query VDR-WORK-VIEWS --history` | `r1` đang áp dụng, `r2` vẫn `PENDING`. |

| UX | Kết quả trên Web |
| --- | --- |
| Functional | **PASS** từ Reader đến số trên Bàn viết và sau khi rút công khai, kể cả retry sau commit. |
| Geometry | **PASS** kiểm tràn của từng hàng và thứ tự ba số trong scenario Desk. Ma trận 30/30 và viewport xem §12. |
| Visual | **PASS** khi mở hai ảnh CSR 1280 × 813 (tác phẩm riêng tư còn 1 lượt; Bàn viết của tài khoản kế tiếp) chụp trong run tích hợp. |
| Accessibility/Responsive | **PASS** các assertion tên control, hit target, focus và reflow tới 200% trong Desk. Đọc bằng trình đọc màn hình vật lý `NOT_RUN`. |
| Interaction | **PASS** Enter/Escape, trả focus, chạm qua CDP, Back/Forward. Thiết bị cảm ứng vật lý `NOT_RUN`. |

### Độ trễ và tài nguyên

So sánh bộ dev trên cùng host, cùng bài công khai và cấu hình. Chạy pha `reader-startup` của `browser-smoke.mjs` ba lần cho mỗi renderer/build. Mỗi lần đo mở lạnh, tải lại, mở tab mới ở CPU 1× và throttle 4×, tổng cộng 72 mẫu. Bảng ghi trung vị thời điểm frame đầu có văn bản, tính bằng mili giây từ khi bắt đầu điều hướng. Mẫu gốc và request waterfall theo từng lần mở

| Renderer | CPU | Mở lạnh, trước → sau | Tải lại | Tab mới |
| --- | --- | --- | --- | --- |
| SSR | 1× | 50,1 → 46,8 | 20,6 → 20,0 | 26,9 → 30,2 |
| SSR | 4× | 51,9 → 49,9 | 44,5 → 43,1 | 84,7 → 84,9 |
| CSR | 1× | 647,0 → 636,0 | 581,7 → 567,2 | 685,3 → 661,3 |
| CSR | 4× | 917,6 → 922,0 | 948,4 → 950,1 | 1268,0 → 1273,8 |

Ở CSR 4×, trung vị tăng 1,7–5,8 ms. Ba mẫu mỗi ô chỉ mô tả lần đo trên host này. Trong ca độc lập có phiên đăng nhập thật, thời điểm thấy chữ CSR là 553 → 661 ms. Waterfall có cùng các route API, timer mới bắt đầu sau khi thân bài đã lên DOM. Ở ca SSR mở liên kết chia sẻ, frame chữ tới ở 44 → 31 ms, rồi POST duy nhất bắt đầu ở 5.612 → 5.748 ms. Không có request thu nhận lượt trên đường hiển thị văn bản.

Cả trước và sau sửa, mỗi lần mở đủ ngưỡng trong ca điều hướng có một POST. Trong ba chu kỳ mở/rời, mỗi lần mount thêm một timer 5 giây, một listener `prerenderingchange` và một `pagehide`. Sau khi rời, timer theo dõi về 0, hai nhóm listener về mức nền 0 và 2. Ca lỗi riêng xác nhận timer retry đang chờ cũng về 0 ngay sau khi điều hướng. Tracker tiếp tục dùng timer một lần, không thêm heartbeat hay polling.

### Đối chiếu acceptance

| #323 | Nguồn và phép đối chiếu | Kết quả và phần còn mở |
| --- | --- | --- |
| AC-01 | Đặc tả §3–§6, P-01..05, bảng ví dụ và signoff r1 tại §2 registry này | Đã ghi nguồn quyết định. Ngưỡng mới theo AC-07. |
| AC-02 | P-06..08, ma trận vòng đời §13, test đọc sau rút, tái xuất bản và xóa tài khoản | Quyền đọc và WorkId có bằng chứng PostgreSQL. Route xóa tác phẩm và admin gỡ cả bài còn thuộc owner ở §13.7. |
| AC-03 | P-09..11, §7.6/§9.5, test mốc đếm, dọn hạn và lỗi đọc riêng từng metric | Đã đối chiếu. Không có dữ liệu để suy người đọc duy nhất hay lịch sử trước mốc. |
| AC-04 | §7.1/§9.1/§10, đối chiếu CHOICE-01..06 với tracker, HTTP, giao dịch PG và DTO tác giả | Sáu lựa chọn có owner, lý do và mức `DESIGN_PROPOSED`. |
| AC-05 | Ma trận ở §5, test race, replay và cửa sổ ở §7.6, Chrome ở §12 và §14 | Phân biệt fixture thời gian/visibility với commit thật. Mất ACK đã có đối chứng sau ghi. |
| AC-06 | §4, dependency #340 → #341/#342 → #343/#514, lịch sử PR #519 | Đối chiếu bàn giao và phạm vi của từng phần. |
| AC-07 | PRD §17, §2/§13.5, crate chính sách chung và ca override | **Mở.** `VDR-WORK-VIEWS@r2` còn thiếu signoff của chủ sản phẩm cho hash ở §13.6. |
| AC-08 | #519 §12, #550/#611 §13.2–§13.4 và các kiểm chứng §14 | **Đạt trong phạm vi Web và PG đã chạy.** 4.999/5.000 ms, quyền, 24 giờ, CSR/SSR, mất ACK sau commit, đọc lại bằng hai phiên thật và waterfall. Dữ liệu thời gian theo host ở trên. |
| AC-09 | LayoutSpec r3, #514, §11–§12 và hai phiên ở §14 | Số riêng, 0/dấu gạch, số cũ, giải thích và refresh. Đã kiểm geometry 30/30 tại `6057b8f1`. |
| AC-10 | D-01/D-02 trong PRD và LayoutSpec, Chrome Desk đọc số liệu rồi mở editor | Thấm → Nhận xét → Lượt xem. Hàng mở editor, đã kiểm bỏ đích Đọc. |

Phạm vi host cho #343 AC-02/06 gồm Chrome desktop, sự kiện touch qua CDP, đồng hồ/cờ
prerender và visibility điều khiển được. Không có lần chạy trên màn hình cảm ứng vật lý.
AC-07 của Epic vẫn cần chủ sản phẩm. Route xóa tác phẩm do Publication sở hữu, route admin
gỡ cả bài thuộc Publication/Moderation. Xóa **tài khoản** đã có bằng chứng ở
[PR #550](https://github.com/loveoverflowcom/vink/pull/550) và [PR #611](https://github.com/loveoverflowcom/vink/pull/611).
