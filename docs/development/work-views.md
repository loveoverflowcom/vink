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
| Quyết định sản phẩm | VIEW-01..12 `PRODUCT_CONFIRMED` trong brief #340 ngày 21/11/2026. [VDR-WORK-VIEWS@r1](../../decisions/proposals/VDR-WORK-VIEWS.r1.toml) có [signoff](../../decisions/signoffs/VDR-WORK-VIEWS.r1.toml) `PRODUCT_CONFIRMED` ngày 21/11/2026, là revision hiện hành. OQ-1..OQ-4 `OPEN`. |
| Quyết định kiến trúc | CHOICE-02 A, CHOICE-03 và CHOICE-06 A `DESIGN_PROPOSED` bởi #341 (§7.1); CHOICE-05 A `DESIGN_PROPOSED` bởi #342 (§9.1). Chưa lựa chọn nào `Accepted`. CHOICE-01, 04 `OPEN`. |
| Mức triển khai | Đường ghi #341: `core` + adapter PostgreSQL + dọn hạn trong `vink-worker`, chưa có HTTP thu nhận. Đọc #342: `persistence-api` (luật đọc thuần, câu lệnh batch trong read-model tác giả, trường `views` của `GET /api/v1/author/works`). Chưa có consumer Web (bảng ở §3). |
| Bằng chứng | #340: kiểm tài liệu (§6). #341: unit và PostgreSQL thật (§7.6). #342: unit/contract và PostgreSQL thật qua router HTTP (§9.5). Chưa có browser. |

## 2. Snapshot quyết định

Tra bằng `cargo xtask decision` tại nhánh `docs/340-dac-ta-luot-xem`, base
`develop@9f68c513b5dae224b6a9f896d04bc9d1d035e1b8`, sau khi ghi signoff:
snapshot `8a322aed0446142317f78323e9bcb1748deae7e50e6cfbc892c360f2bc68aaaf`
(137 revision). Trước signoff, ngay sau khi tạo proposal, snapshot là
`176154f6ec916e58eb3e4d5db8419e7e256d8c2dfcc8c3b8a643e67708fa8330`. Snapshot đổi khi kho có
revision hay signoff mới; người tiếp tục tra lại.

| ID@rN | Trạng thái | SHA-256 payload | Vai trò |
| --- | --- | --- | --- |
| `VDR-WORK-VIEWS@r1` | `PRODUCT_CONFIRMED` (`current`, signoff 21/11/2026) | `cfb789cc2c5f7a99a2f8c8e24f0057815d736d7b02cdacde50d327c8a525e008` | Luật Lượt xem của PR này. |
| `O-17@r2` | `PRODUCT_CONFIRMED` | `cb30cb43040a3fad50bd4930096e537057a2f3c6037b387d8c332be497fa15aa` | Trang rút sau kiểm duyệt trả HTTP 200; không phải lượt. |
| `USER-02@r1` | `PRODUCT_CONFIRMED` | `2293bf58194809957b892b74bc3891d56a05998e7b97008ad1c8fbfd98fdc97b` | Hai audience. |
| `LVE-PR-01-CHOICE-02@r1` | `DESIGN_PROPOSED` | `766485bf6d2c6eabc3292921fc478d01a13376b46bed36bee9a00150a6ad6a03` | Thấm gắn `RevisionId`; Lượt xem cố ý khác (P-06). |
| `VDR-DECISION-SIGNOFF@r1` | `PRODUCT_CONFIRMED` | `add9c28677a66d759cc4c352eada0ebf92cc64dcc0849d489f2d89f16a022dc2` | Brief chưa tự là signoff của payload TOML. |
| `VDR-AUTHOR-WORK-FEEDBACK@r1` | `PENDING` | `4616ced97f723fac18cc31b1c11639d1eb7649b83340bb2d702d92999a3a14a7` | Mẫu “—” khác “0”; ẩn Thấm/Nhận xét khi riêng tư (OQ-1). |
| `O-08@r1` | `OPEN` | `410cb3637a3d013c49b9bd746ec8b5ddb292761fed6b987c3134b281c95b4375` | Purge vật lý của tác phẩm, tách khỏi luật xóa số liệu (OQ-2). |

## 3. Mức triển khai theo yêu cầu

| ID | Mức hiện tại | Issue sở hữu | Ghi chú |
| --- | --- | --- | --- |
| VIEW-01 | Kiểm tại lúc ghi: `core` + PostgreSQL (#341). Điểm phát: `NOT_IMPLEMENTED` | #343 (điểm phát), #341 (kiểm tại lúc ghi) | Điểm hiển thị có sẵn: `ReaderResponse::Published` → `PublicationView`. Ghi chỉ cộng khi pointer công khai tồn tại lúc commit. |
| VIEW-02 | `NOT_IMPLEMENTED` | #343 | No-JS không đếm theo luật. Luật nhận diện prefetch/prerender (`is_speculative`) có ở core, chưa áp tại HTTP. |
| VIEW-03 | `core` + PostgreSQL (#341) | #341 | Cửa sổ trượt neo tại lượt được tính; đúng 24 giờ vẫn chặn. Receipt theo sự kiện tách khỏi cửa sổ. |
| VIEW-04 | `core` + PostgreSQL (#341); cookie khách `NOT_IMPLEMENTED` | #341, #343 (cấp cookie) | `GuestToken` UUID v4, thời hạn 24 giờ cố định. |
| VIEW-05 | `core` + PostgreSQL (#341) | #341 | So principal với `works.owner_id` tại lúc ghi. |
| VIEW-06 | Khóa ghi: `core` + PostgreSQL (#341). Đọc: `persistence-api` (#342) | #341 (khóa ghi), #342 (đọc không reset) | Mọi bảng khóa theo `WorkId`; revision chỉ là căn cứ ghi trong receipt. Đọc không nhìn revision. |
| VIEW-07 | `persistence-api` (#342); Bàn viết `NOT_IMPLEMENTED` (lượt thiết kế) | #342 | Chỉ `GET /api/v1/author/works` của phiên chủ; không API công khai. |
| VIEW-08 | Phía ghi: `core` + PostgreSQL (#341). Đọc: `persistence-api` (#342) | #342 (đọc), #341 (không tăng khi hết công khai) | Tổng không bao giờ giảm khi ẩn/rút. Bài riêng tư vẫn đọc được số theo fallback OQ-1. |
| VIEW-09 | Mốc lưu ở `work_view_counting` (#341). Dữ liệu nhãn: `persistence-api` (`counted_since`, #342). Nhãn hiển thị: `NOT_IMPLEMENTED` | #342, lượt thiết kế | Không backfill; mốc là lần quyết định sự kiện đầu tiên. |
| VIEW-10 | Idempotency: `core` + PostgreSQL (#341). Retry: `NOT_IMPLEMENTED` | #343 (retry), #341 (idempotency) | Kỳ vọng khoảng 1 phút, không SLA. Ghi đồng bộ nên số có ngay sau commit. |
| VIEW-11 | `core` + PostgreSQL + `vink-worker` (#341) | #341 | Ba loại dữ liệu; cửa sổ và receipt xóa sau 24 giờ (chu kỳ dọn mỗi giờ). Xóa theo `WorkId` có hàm `erase_work`, chưa có route (OQ-2). |
| VIEW-12 | Trạng thái dữ liệu: `persistence-api` (`views.state`, #342). Hiển thị: `NOT_IMPLEMENTED` | #342 (trạng thái dữ liệu), lượt thiết kế (hiển thị) | Copy cuối thuộc lượt thiết kế. |

## 4. Bản đồ P/CHOICE sang issue

| Issue | Kết quả | P-ID | CHOICE | Câu còn mở chạm tới |
| --- | --- | --- | --- | --- |
| [#341](https://github.com/loveoverflowcom/vink/issues/341) Ghi và chống trùng | Use case ghi, migration, adapter PostgreSQL, purge mã hết hạn | P-03, P-04, P-05, P-11 (thêm P-06 cho khóa `WorkId`; P-01/P-08 cho kiểm tại lúc ghi) | CHOICE-02, CHOICE-03, CHOICE-06 | OQ-2 (thiết kế để xóa theo `WorkId` làm được) |
| [#342](https://github.com/loveoverflowcom/vink/issues/342) Đọc theo quyền | Query/read-model, DTO, contract đọc của tác giả, trạng thái “0”/“—”/“Tính từ” (§9) | P-07, P-08, P-09 (thêm P-06 cho đọc cộng dồn; VIEW-12 cho trạng thái) | CHOICE-05 (A, §9.1) | OQ-1, OQ-4 |
| [#343](https://github.com/loveoverflowcom/vink/issues/343) Reader và kiểm xuyên suốt | HTTP thu nhận, tín hiệu Web tại điểm thân bài đã hiện, kịch bản browser cuối | P-01, P-02, P-10 | CHOICE-01, CHOICE-04 | OQ-3 |
| Lượt thiết kế (chưa có issue) | LayoutSpec vị trí số, copy “—”/“Tính từ”/độ phủ trong Bàn viết | VIEW-07, VIEW-12 | — | OQ-1 |

Thứ tự: #341 sau PR này; #342 sau #341 (dùng dữ liệu do đường ghi tạo); #343 viết tín hiệu sau
#341 và nghiệm thu xuyên suốt sau #342.

## 5. Kịch bản nghiệm thu xuyên suốt

Dùng làm oracle chung cho #341–#343. Dữ liệu thử: tác phẩm W công khai lần đầu sau mốc bật;
T là tác giả; A là thành viên; G là khách; D là admin. Thời điểm tính theo đồng hồ thử. Tổng là
số tác giả đọc được qua API của #342 sau khi chờ hội tụ (kỳ vọng tối đa khoảng 1 phút).

| Bước | Thao tác | Tổng mong đợi | Ví dụ, vòng đời | Lane |
| --- | --- | --- | --- | --- |
| 1 | T mở Bàn viết | 0 (số thật, không phải “—”) | L-02 | #342 HTTP, #343 browser |
| 2 | 08:00 A bấm thẻ W ở Khám phá, thân bài hiện | 1 | E-01 | #343 browser |
| 3 | A refresh, Back rồi Forward | 1 | E-14, E-15 | #343 browser |
| 4 | Gửi lại cùng mã sự kiện sau khi mất phản hồi; bấm đúp | 1 | E-27, E-28 | #341 PostgreSQL, #343 |
| 5 | G mở link chia sẻ W ở tab mới, rồi mở thêm tab thứ hai | 2 | E-05, E-07, E-29 | #343 browser; #341 đồng thời |
| 6 | T đăng nhập mở W ở Reader; mở nháp ở Bàn viết | 2 | E-22, E-23 | #341, #343 |
| 7 | D mở ca duyệt của W; rồi mở W ở Reader công khai | 2, rồi 3 | E-25, E-26 | #341, #343 |
| 8 | Trình duyệt tắt JavaScript mở W; bot tải W để tạo bản xem trước | 3 | E-31, E-06, E-18 | #343 |
| 9 | r2 thay r1; A mở lại W lúc 10:00 | 3, không reset | E-35, L-06 | #341, #342 |
| 10 | T chuyển W sang “Chỉ mình tôi”; A mở URL cũ | 3, Bàn viết vẫn hiện 3 (OQ-1) | E-12, L-07 | #342, #343 |
| 11 | T công khai lại; A mở W lúc 08:01 hôm sau | 4 | E-17, L-09 | #341, #343 |
| 12 | Làm lỗi nguồn số liệu khi đọc | “—” kèm lý do; tên, trạng thái, link sửa còn | L-05 | #342 |
| 13 | Làm lỗi đường ghi khi A mở W | Reader đọc bình thường; tổng không tăng hoặc tăng sau retry; không bao giờ +2 | E-32, E-33 | #343 |
| 14 | Tác phẩm khác V công khai trước mốc bật | Số kèm “Tính từ dd/mm/yyyy” | L-12 | #342 |

Bước nào chạm OQ-1..OQ-4 thì ghi kết quả quan sát là `OPEN`, không tính là PASS hay FAIL.

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

### Kết quả ngày 21/11/2026

Chạy trên cây làm việc của nhánh `docs/340-dac-ta-luot-xem` (base
`develop@9f68c513b5dae224b6a9f896d04bc9d1d035e1b8`), Linux, trước commit.

| Phép kiểm | Kết quả |
| --- | --- |
| `git diff --check` | Không lỗi. |
| Kiểm link tương đối và anchor kiểu GitHub cho `prd/requirements.md`, hai file `work-views.md`, `docs/README.md`, `docs/development/README.md`, `decisions/README.md` | 0 link hỏng. Script tạm, có ca đối chứng âm (anchor sai, file thiếu) được bắt đúng. |
| `cargo xtask decision validate` | 137 revision, snapshot `176154f6ec916e58eb3e4d5db8419e7e256d8c2dfcc8c3b8a643e67708fa8330`, 98 tham chiếu khai báo; review ngữ nghĩa là việc riêng. |
| `cargo xtask decision query VDR-WORK-VIEWS` | `pending_only`, head r1, SHA-256 `cfb789cc2c5f7a99a2f8c8e24f0057815d736d7b02cdacde50d327c8a525e008`. |
| `wc -c AGENTS.md` | 18.876 byte; không sửa AGENTS.md. |
| Đối chiếu 11 câu của #323 | Đủ, bảng dưới. |

Sau signoff (cùng ngày, trên cây làm việc sau commit `8c9cbf38830042e8b761cf709a80f9a4f36f33da`):

| Phép kiểm | Kết quả |
| --- | --- |
| `cargo xtask decision signoff VDR-WORK-VIEWS@r1 cfb789cc… INPUT` | `current: true`, `PRODUCT_CONFIRMED`. |
| `cargo xtask decision validate` | 137 revision, snapshot `8a322aed0446142317f78323e9bcb1748deae7e50e6cfbc892c360f2bc68aaaf`, 99 tham chiếu khai báo. |
| `cargo xtask decision query VDR-WORK-VIEWS` | `current`, r1, `PRODUCT_CONFIRMED`, SHA-256 không đổi. |
| `cargo xtask decision query VIEW-03` | `related_only` trả `VDR-WORK-VIEWS@r1`. |
| `cargo xtask decision refs` | Marker `active VDR-WORK-VIEWS@r1` ở đặc tả được nhận. |
| `git diff --check`, kiểm link, `wc -c AGENTS.md` | Không lỗi; 0 link hỏng; 18.876 byte. |

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
- Route xóa tác phẩm/tài khoản chưa có (OQ-2); chỉ có `erase_work` và cascade, đã kiểm trên
  PostgreSQL.
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
- **OQ-2, OQ-4.** Chưa có route xóa tác phẩm/tài khoản hay admin gỡ cả bài, nên không có ca runtime.
  Khi route xóa có, `erase_work` (#341) xóa hàng và tác phẩm rời danh sách; quyết định cho admin
  gỡ cả bài vẫn `OPEN`, không tự áp P-08.

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

- Nghiệm thu xuyên suốt §5 đọc số qua `views` của `GET /api/v1/author/works` bằng phiên tác giả:
  bước 1 (`counted`/0 khi mốc đã có, ngược lại `not_started`), 9–11 (giữ qua r2, riêng tư,
  công khai lại), 12 (`unavailable`), 14 (`counted_since`). Ghi đồng bộ (CHOICE-02 A) nên số có
  ngay sau commit; không có trạng thái “chưa hội tụ”.
- Tải lại danh sách là cách duy nhất để thấy số mới; không validator, không `304`. `ETag` của
  Reader, danh sách công khai và `/publication` không phụ thuộc số (§9.5).
- Consumer Web (`apps/vink-web/src/contract/author_works.rs`) hiện bỏ qua `views` (decoder không
  từ chối field lạ). Lượt thiết kế thêm representation, “—” kèm lý do cho ba trạng thái không số,
  định dạng 1.234 và “Tính từ dd/mm/yyyy” theo `Asia/Ho_Chi_Minh` từ `counted_since`.

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
