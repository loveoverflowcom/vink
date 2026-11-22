# Lượt xem tác phẩm — đặc tả nghiệp vụ

Đặc tả này đi theo chuỗi: độc giả mở bài → khi nào thành một lượt → tác giả thấy số nào, ở
đâu, trong từng trạng thái của bài. Backend, Web và người thiết kế dùng chung một nghĩa từ đây.
Nguồn: [#340](https://github.com/loveoverflowcom/vink/issues/340), thuộc
[Epic #323](https://github.com/loveoverflowcom/vink/issues/323). Yêu cầu có ID nằm ở
[PRD §17](../../prd/requirements.md#17-lượt-xem-tác-phẩm-work-views). Mức triển khai, bản đồ
sang các issue tiếp theo và kịch bản nghiệm thu xuyên suốt nằm ở
[registry phát triển](../development/work-views.md).

## 1. Mục tiêu, nguồn và trạng thái

Tác giả biết bài của mình được mở đọc ở mức nào, qua một con số tham khảo đếm đúng nghĩa đã
chốt. Con số không cản người đọc, không lộ ai đã đọc và không lưu nhiều hơn mức cần để chống
trùng.

<!-- vdr: active VDR-WORK-VIEWS@r1 -->

**Lời chốt.** Ngày 21/11/2026, trong phiên làm #340, chủ sản phẩm trả lời “Theo khuyến nghị”
cho đúng bộ khuyến nghị P-01..P-11 và luật UX ở [§3](#3-bảng-quyết-định-p-01p-11). Brief do chủ
sản phẩm gửi. Các phương án gốc, câu hỏi kỹ thuật CHOICE-01..06 và phép kiểm đề xuất nằm trong
Epic #323.

| Chiều | Trạng thái |
| --- | --- |
| Quyết định sản phẩm | P-01..P-11 và luật UX: `PRODUCT_CONFIRMED` trong brief ngày 21/11/2026 (VIEW-01..12). [VDR-WORK-VIEWS@r1](../../decisions/proposals/VDR-WORK-VIEWS.r1.toml) ghi cùng nội dung; chủ sản phẩm duyệt đúng revision này ngày 21/11/2026 ([signoff](../../decisions/signoffs/VDR-WORK-VIEWS.r1.toml), `PRODUCT_CONFIRMED`). OQ-1..OQ-4 ở [§4](#4-câu-hỏi-còn-mở) là `OPEN`. |
| Quyết định kiến trúc | #341 đề xuất CHOICE-02 A, CHOICE-03 và CHOICE-06 A (`DESIGN_PROPOSED`, [registry §7](../development/work-views.md#7-đường-ghi-341)); #342 đề xuất CHOICE-05 A ([registry §9.1](../development/work-views.md#91-choice-05-design_proposed)). CHOICE-01, 04 `OPEN`, giao cho #343. [§10](#10-ràng-buộc-cho-lựa-chọn-kỹ-thuật-choice-0106) chỉ ghi ràng buộc nghiệp vụ mà lựa chọn phải thỏa. |
| Mức triển khai | Đường ghi (#341): `core` + adapter PostgreSQL + dọn hạn trong `vink-worker`. Đọc của tác giả (#342): `persistence-api`, trường `views` của `GET /api/v1/author/works`. Chưa có HTTP thu nhận, tín hiệu Web hay UI ([registry](../development/work-views.md)). |
| Bằng chứng | Đường ghi: unit và PostgreSQL thật ([registry §7.6](../development/work-views.md#76-bằng-chứng-và-nghiệm-thu)). Đọc: unit/contract và PostgreSQL qua router HTTP ([registry §9.5](../development/work-views.md#95-bằng-chứng-và-nghiệm-thu)). Chưa có browser. |

Các luật đã có và đặc tả này giữ nguyên: WR-01 (nháp riêng), UX-01 (Reader có HTML từ server),
AUD-01/02 (hai audience, audience không phải trạng thái xuất bản), PUB-04/05/07 (xóa, riêng tư,
trang ẩn), MOD-05 và INV-08 (nháp riêng không lọt ra ngoài phạm vi),
[`O-17@r2`](../../decisions/proposals/O-17.r2.toml) (trang rút sau kiểm duyệt trả HTTP 200).
Mã HTTP không phải bằng chứng người đọc đã xem thân bài.

## 2. Định nghĩa và thuật ngữ

> **Một lượt xem** là một lần người đọc chủ động mở trang đọc công khai `/tac-pham/{id}` của
> một tác phẩm và thân bài công khai đã hiện trên trình duyệt có JavaScript. Người đọc không
> phải tác giả đang đăng nhập của bài. Mỗi người đọc được tính tối đa một lượt cho mỗi tác phẩm
> trong 24 giờ trượt. Số lượt cộng dồn theo tác phẩm.

“Lượt xem” **không** phải số người duy nhất: một khách dùng hai trình duyệt có thể thành hai lượt.
Nó cũng không phải số người đọc hết bài, thời gian đọc, hay số request tới server.

**Cửa sổ 24 giờ trượt.** Lần mở được tính khi người đọc chưa có lượt nào cho tác phẩm, hoặc lượt
được tính gần nhất của người đọc đó với tác phẩm đó đã quá 24 giờ. Lần mở bị chặn trong 24 giờ
ấy không cộng và không kéo dài cửa sổ. Ví dụ: 08:00 thứ Hai +1, 20:00 thứ Hai +0, 07:59 thứ Ba
+0, 08:01 thứ Ba +1. Cách đọc này khớp với mã chống trùng có thời hạn bằng cửa sổ (P-04) và tự
xóa khi hết cửa sổ (P-11). Đúng mốc 24 giờ (08:00:00 thứ Ba) vẫn +0; từ sau mốc đó +1 (#341).

| Tên trong sản phẩm | Tên trong code/API hiện có | Trạng thái | Ranh giới |
| --- | --- | --- | --- |
| Lượt xem | `views` trong `GET /api/v1/author/works` | có (API tác giả, #342) | Chỉ tác giả thấy (P-07). |
| Tác phẩm | `WorkId` | có sẵn | Khóa cộng dồn của Lượt xem (P-06). |
| Bản công khai | `RevisionId`, `Publication` | có sẵn | Không phải khóa của Lượt xem. Thấm dùng khóa này ([`LVE-PR-01-CHOICE-02@r1`](literary-endorsement.md)). |
| Thân bài công khai đã hiện | `ReaderResponse::Published` → `PublicationView` trong [`pages/mod.rs`](../../apps/vink-web/src/pages/mod.rs) | có sẵn (điểm hiển thị) | Điểm duy nhất có thể sinh lượt. |
| Đang mở | `Suspense` fallback “Đang mở tác phẩm…” trong `ReaderScope` | có sẵn | Rời trang ở đây không sinh lượt. |
| Tải lỗi | `LoadError::NotFound` (404), lỗi khác (503, `Unavailable`) | có sẵn | Không sinh lượt. |
| Trang thay thế | `ReaderResponse::Hidden` (trang ẩn, PUB-07) và `ReaderResponse::ReviewWithdrawn` (`O-17@r2`) → `ReplacementView` | có sẵn | Trả HTTP 200 nhưng không có thân bài, nên không sinh lượt. |
| Người đọc (định danh đo lường) | `ViewReader`: thành viên `PrincipalId` từ phiên, khách `GuestToken` | có (core, #341) | Một định danh đo lường, không phải một người thật. Hai loại không gộp (E-30). |
| Mã khách | `GuestToken` (UUID v4) | có (core); cookie do #343 cấp | Ngẫu nhiên, first-party, thời hạn 24 giờ cố định, chỉ dùng chống trùng Lượt xem. |
| Mã chống trùng đã băm | `ReaderDigest` (HMAC theo khóa server), bảng `work_view_windows` | có (core + PostgreSQL) | Dạng băm của người đọc gắn với tác phẩm; tự xóa khi hết cửa sổ. |
| Mã sự kiện | `ViewEventId` (UUIDv7), receipt `work_view_receipts` | có (core + PostgreSQL) | Gửi lại cùng mã không bao giờ cộng thêm (P-03, P-10). |
| Tổng theo tác phẩm, tổng theo ngày | `work_view_totals`, `work_view_days` | có (PostgreSQL); tổng đọc qua API tác giả (#342), tổng ngày chưa đọc | Không theo người (P-11). |
| Mốc bật | `work_view_counting.started_at` | có (PostgreSQL); `views.counted_since` (#342); nhãn hiển thị thuộc lượt thiết kế | Ngày bắt đầu đếm; nhãn “Tính từ dd/mm/yyyy” (P-09). |
| Bàn viết | `/ban-viet`, `GET /api/v1/author/works` ([read-model](../development/desk-author-feedback.md)) | có sẵn | Nơi tác giả sẽ thấy số; vị trí trong hàng thuộc lượt thiết kế sau. |

## 3. Bảng quyết định P-01..P-11

Mọi dòng: chủ sản phẩm, 21/11/2026, “Theo khuyến nghị” cho brief #340. Câu hỏi và phương án gốc
ở Epic #323.

| PRD | #323 | Lựa chọn | Luật áp dụng | Ranh giới, khác biệt |
| --- | --- | --- | --- | --- |
| VIEW-01 | P-01 Khi nào tính một lượt? | **B** | +1 khi thân bài công khai đã hiện sau thao tác mở. Không yêu cầu thời gian ở lại. Bấm rồi hủy, tải lỗi, trang thay thế (kể cả HTTP 200) = +0. Tên hiển thị “Lượt xem”. | Không phải “lượt mở” (A). HTML server trả về chưa chứng minh thân bài đã hiện. |
| VIEW-02 | P-02 Đường vào nào được tính? | **B** | Link nội bộ, URL trực tiếp, link chia sẻ, tab mới. Chuột, chạm, Enter như nhau. Hover, prefetch, bot tạo bản xem trước link = +0. Trình duyệt tắt JavaScript không được đếm. | Giới hạn no-JS phải hiện cho tác giả (VIEW-12). |
| VIEW-03 | P-03 Mở lại đếm thế nào? | **C** | Tối đa 1 lượt/người đọc/tác phẩm trong 24 giờ trượt ([§2](#2-định-nghĩa-và-thuật-ngữ)). Refresh, Back/Forward, mở lại trong cửa sổ = +0. Gửi trùng kỹ thuật không bao giờ thành lượt mới. | Hai phép kiểm riêng: gộp lần mở trong cửa sổ, và idempotency của cùng một sự kiện. |
| VIEW-04 | P-04 Khách có được tính? | **A** | Có. Chống trùng bằng mã ngẫu nhiên first-party, thời hạn = cửa sổ P-03. Không IP thô, không fingerprint. | Chấp nhận đếm lại khi xóa cookie, đổi trình duyệt/máy, hoặc khách đăng nhập giữa cửa sổ. |
| VIEW-05 | P-05 Tác giả tự xem, lượt phục vụ duyệt? | **B** | Không tính khi phiên là tác giả của bài. Màn kiểm duyệt, preview, nháp không bao giờ tính. Admin mở Reader công khai như độc giả thì tính. | Tác giả đã đăng xuất được tính như khách; không hứa nhận diện. Luật theo bề mặt: moderator ở Reader công khai cũng như độc giả. |
| VIEW-06 | P-06 Thuộc tác phẩm hay bản công khai? | **A** | Cộng dồn theo `WorkId` qua sửa, đăng lại, revision mới. | Khác Thấm (gắn `RevisionId`): revision mới không reset Lượt xem. |
| VIEW-07 | P-07 Ai thấy, ở đâu? | **A** | Bản đầu chỉ tác giả thấy, ở Bàn viết. | Không số công khai, không danh sách người xem. Vị trí trong Bàn viết thuộc lượt thiết kế sau. |
| VIEW-08 | P-08 Ẩn/rút/riêng tư thì số cũ thế nào? | **A** | Bài bị ẩn, chuyển “Chỉ mình tôi” hay bị rút sau kiểm duyệt: tác giả vẫn thấy số cũ, không tăng thêm. Công khai lại thì đếm tiếp. Mở trang thay thế không tính. | Khác luật hiện hành của Thấm/Nhận xét khi bài riêng tư: OQ-1. Admin gỡ cả bài: OQ-4. |
| VIEW-09 | P-09 Có tính lại lịch sử? | **A** | Bắt đầu từ ngày bật. Bài đăng trước mốc có nhãn “Tính từ dd/mm/yyyy”. Không backfill. | Chưa từng có nguồn ([#214](https://github.com/loveoverflowcom/vink/issues/214) loại tracking). Không suy từ Thấm/Nhận xét hay log truy cập. |
| VIEW-10 | P-10 Trễ và mất lượt? | **A** | Số tham khảo; kỳ vọng tác giả thấy trong khoảng 1 phút, không phải SLA. Retry hữu hạn trong tab cùng mã sự kiện. | Đóng tab hoặc offline có thể mất lượt; không bao giờ cộng trùng. Lỗi thống kê không bao giờ cản người đọc. |
| VIEW-11 | P-11 Lưu gì, bao lâu? | **B** | Tổng/tác phẩm, tổng theo ngày/tác phẩm (không theo người, chưa có biểu đồ), mã chống trùng đã băm tự xóa khi hết cửa sổ. Xóa tác phẩm hoặc tài khoản thì xóa số liệu. | Không áp tuổi 180 ngày của Notifications. Route xóa chưa có runtime: OQ-2. |
| VIEW-12 | UX | — | Chưa có hoặc lỗi = “—” kèm lý do, khác “0” thật (mẫu [VDR-AUTHOR-WORK-FEEDBACK@r1](../../decisions/proposals/VDR-AUTHOR-WORK-FEEDBACK.r1.toml)). Định dạng 1.234. Không realtime, không thông báo. | Nhãn diễn đạt đúng độ phủ ([§8](#8-trải-nghiệm-người-dùng-nghĩa-không-phải-thiết-kế-thị-giác)). |

## 4. Câu hỏi còn mở

Các câu dưới đây là `OPEN` ở mức sản phẩm. Không câu nào chặn PR tài liệu hay #341. Khuyến nghị
chỉ là đề xuất để chủ sản phẩm chọn, chưa phải luật.

| ID | Câu hỏi | Ca bị ảnh hưởng | Phương án, khuyến nghị | Khi chưa có câu trả lời |
| --- | --- | --- | --- | --- |
| **OQ-1** | P-08 A cho tác giả thấy Lượt xem khi bài riêng tư. [VDR-AUTHOR-WORK-FEEDBACK@r1](../../decisions/proposals/VDR-AUTHOR-WORK-FEEDBACK.r1.toml) (kiến trúc, `PENDING`) ẩn Thấm/Nhận xét khi bài riêng tư hoặc không rõ trạng thái. Có đồng bộ hai luật không? | L-07, L-08 ([§7](#7-vòng-đời-theo-trạng-thái-ac-03)); hàng Bàn viết của bài riêng tư; contract đọc #342; lượt thiết kế. | **A** giữ hai luật khác nhau: hàng chỉ hiện Lượt xem. **B** ẩn cả Lượt xem (trái P-08 A, cần revision mới). **C** hiện lại cả Thấm/Nhận xét cũ (đổi read-model tác giả). | Áp P-08 A: hàng Bàn viết của bài riêng tư có thể chỉ hiện Lượt xem (số cũ, không tăng). |
| **OQ-2** | Route xóa tác phẩm (PUB-04/05, mới ở core) và xóa tài khoản (SOC-LIFE-02) chưa có runtime. Luật xóa số liệu của P-11 ghi ở mức target. | L-10, L-11. | Điều kiện kích hoạt: PR đầu tiên đưa route xóa tác phẩm hoặc xóa tài khoản vào runtime phải xóa tổng, tổng theo ngày và mã chống trùng của tác phẩm liên quan, kèm phép kiểm. `O-08` (purge vật lý của chính tác phẩm) vẫn `OPEN` và tách khỏi luật này. | Không có ca runtime nào chạm luật xóa. #341 thiết kế dữ liệu sao cho xóa theo `WorkId` làm được. |
| **OQ-3** | Thân bài hiện mà không có thao tác mở mới, hoặc chưa ở trước mắt người đọc. (a) Trang đang mở ở trang ẩn/trang rút chuyển sang thân bài nhờ cập nhật nền của `ReaderLoaded` (bài được công khai lại). (b) Tab mở nền đã hiện thân bài nhưng người đọc chưa chuyển tới. | E-07, E-36 ([§6](#6-bảng-ví-dụ-ac-02)); điểm phát của #343. | Khuyến nghị: (a) tính khi thân bài hiện lần đầu trong trang đó, vẫn qua cửa sổ P-03; (b) tính khi thân bài đã hiện trong tab, không đòi tab được nhìn thấy, vì P-01 B không đo thời gian hay sự chú ý. | #343 không đặt oracle cho hai ca này; ghi OPEN trong kịch bản browser. Luật ghi của #341 không đổi. |
| **OQ-4** | Admin gỡ cả bài vì vi phạm (PUB-05 C, chưa có route runtime). Độc giả nhận “không tìm thấy” nên +0. Tác giả còn thấy số cũ không? | L-14; contract đọc #342 khi route có. | Khuyến nghị: như P-08 A, giữ số cũ cho tác giả và không tăng, cho tới khi tác phẩm bị xóa. | Không có ca runtime. Không tự áp P-08 cho ca này. |

## 5. Quyền

| Actor | Hành động | Đối tượng và trạng thái | Điều kiện | Kiểm tra bền vững tại commit |
| --- | --- | --- | --- | --- |
| Khách, thành viên không phải tác giả | Sinh một lượt (gián tiếp, bằng việc mở bài) | Tác phẩm đang có bản công khai | Thân bài đã hiện; ngoài cửa sổ chống trùng | Tác phẩm còn công khai tại lúc ghi; danh tính lấy từ phiên hoặc mã khách; chống trùng theo cửa sổ. Không tin client về quyền hay trạng thái bài. |
| Tác giả đang đăng nhập | Sinh lượt trên bài của mình | — | Không bao giờ | So principal của phiên với chủ tác phẩm tại lúc ghi. |
| Admin, moderator ở màn kiểm duyệt, ca duyệt, bản gửi duyệt | Sinh lượt | — | Không bao giờ | Các bề mặt này không phát tín hiệu lượt xem. |
| Admin, moderator ở Reader công khai | Sinh lượt | Như người đọc | Như người đọc | Như người đọc. Nếu chính họ là tác giả của bài thì luật tác giả thắng. |
| Tác giả | Đọc số của tác phẩm mình | Mọi trạng thái còn tác phẩm (VIEW-08) | Chủ tác phẩm lấy từ phiên | Quyền hiện tại tại lúc đọc; phản hồi `no-store`; không vào HTML SSR, cache công khai hay JSON Reader công khai. |
| Người khác, khách | Đọc số | — | Không bao giờ (VIEW-07) | Không có API công khai cho số này. |
| Hệ thống | Xóa mã chống trùng hết hạn; xóa số liệu khi xóa tác phẩm/tài khoản (target, OQ-2) | — | Hết cửa sổ; sự kiện xóa | Xóa theo `WorkId`; không cần dữ liệu theo người. |

## 6. Bảng ví dụ (AC-02)

Giả định chung, trừ khi dòng ghi khác: tác phẩm W đang công khai và tính năng đã bật; T là tác
giả; người đọc chưa có lượt nào cho W trong 24 giờ; trình duyệt có JavaScript. Cột “Ai thấy gì”:
chỉ T thấy số, ở Bàn viết, kỳ vọng trong khoảng 1 phút. Người đọc không thấy số và Reader không
có gì mới.

| # | Đầu vào | Người dùng trải qua | Số đổi | Ai thấy gì | Luật |
| --- | --- | --- | --- | --- | --- |
| E-01 | Thành viên A bấm chuột vào thẻ W ở Khám phá | “Đang mở tác phẩm…” rồi thân bài | +1 | T thấy +1 | VIEW-01, 02 |
| E-02 | A chạm thẻ W trên điện thoại | Như E-01 | +1 | T thấy +1 | VIEW-02 |
| E-03 | A dùng Tab tới link W rồi nhấn Enter | Như E-01 | +1 | T thấy +1 | VIEW-02 |
| E-04 | Khách G dán URL `/tac-pham/W` vào thanh địa chỉ | HTML từ server đã có thân bài, rồi hydration | +1 khi trang chạy JavaScript với thân bài đã hiện; HTML server tự nó không là lượt | T thấy +1 | VIEW-01, 02 |
| E-05 | G mở link chia sẻ W từ một ứng dụng chat | Như E-04 | +1 | T thấy +1 | VIEW-02, 04 |
| E-06 | Ứng dụng chat tự tải link để tạo bản xem trước | Không có ai mở bài | +0 | Không đổi | VIEW-02 |
| E-07 | A Ctrl/Cmd+click hoặc chọn “Mở trong tab mới” | Tab mới hiện thân bài | +1. Tab mở nền chưa được chuyển tới: OQ-3 | T thấy +1 | VIEW-02, OQ-3 |
| E-08 | A rê chuột qua link W; trình duyệt hoặc router prefetch | Không mở bài | +0 | Không đổi | VIEW-02 |
| E-09 | A bấm W rồi Back khi còn “Đang mở tác phẩm…” | Không thấy thân bài | +0 | Không đổi | VIEW-01 |
| E-10 | A bấm W và tải lỗi: 503 “Chưa tải được tác phẩm”, mất mạng, hoặc 404 | Trang lỗi | +0 | Không đổi | VIEW-01 |
| E-11 | Sau E-10, A tải lại và thân bài hiện | Thân bài | +1 (lượt đầu của A trong cửa sổ) | T thấy +1 | VIEW-01, 03 |
| E-12 | A mở URL của W khi W đã chuyển “Chỉ mình tôi” (HTTP 200) | “Nội dung đã được ẩn” | +0 | T thấy số cũ | VIEW-01, 08 |
| E-13 | A mở URL của W đã bị rút sau kiểm duyệt (`O-17@r2`, HTTP 200) | “Bản này đã bị gỡ sau khi kiểm duyệt.” | +0 | T thấy số cũ | VIEW-01, 08 |
| E-14 | A có lượt lúc 08:00, refresh lúc 08:05 | Thân bài | +0 | Không đổi | VIEW-03 |
| E-15 | A Back khỏi W lúc 08:10 rồi Forward lại | Thân bài | +0 | Không đổi | VIEW-03 |
| E-16 | A mở lại W lúc 20:00 cùng ngày và lúc 07:59 hôm sau | Thân bài | +0, +0 | Không đổi | VIEW-03 (cửa sổ không kéo dài) |
| E-17 | A mở lại W lúc 08:01 hôm sau | Thân bài | +1 | T thấy tổng +2 so với trước 08:00 hôm trước | VIEW-03 |
| E-18 | Bot tìm kiếm hoặc crawler không chạy JavaScript tải W | — | +0 | Không đổi | VIEW-02. Trình duyệt tự động có chạy JavaScript có thể lọt; không hứa loại mọi bot |
| E-19 | Khách G mở W | Thân bài | +1 theo mã khách | T thấy +1 | VIEW-04 |
| E-20 | G xóa cookie, hoặc mở bằng trình duyệt/máy khác, rồi mở lại W sau 1 giờ | Thân bài | +1 (chấp nhận đếm lại) | T thấy +1 | VIEW-04 |
| E-21 | Thành viên A mở W trên điện thoại, 1 giờ sau mở trên máy tính cùng tài khoản | Thân bài | +1 rồi +0 | T thấy +1 | VIEW-03; định danh thành viên là `PrincipalId` |
| E-22 | T đang đăng nhập mở W ở Reader | Thân bài | +0 | T thấy số không đổi | VIEW-05 |
| E-23 | T mở nháp hoặc xem trước của W ở Bàn viết | Nháp, bản xem trước | +0 | Không đổi | VIEW-05 |
| E-24 | T đăng xuất rồi mở W | Thân bài | +1, tính như khách | T (khi đăng nhập lại) thấy +1 | VIEW-05, 04 |
| E-25 | Admin hoặc moderator mở ca duyệt, bản gửi duyệt của W | Màn kiểm duyệt | +0 | Không đổi | VIEW-05 |
| E-26 | Admin mở W ở Reader công khai như độc giả | Thân bài | +1 (một lần trong cửa sổ, theo `PrincipalId`) | T thấy +1 | VIEW-05 |
| E-27 | A bấm đúp link W | Thân bài hiện một lần | +1 tổng | T thấy +1 | VIEW-03 |
| E-28 | Lần gửi đầu đã ghi nhưng mất phản hồi; tab gửi lại cùng mã sự kiện | Đọc bình thường | +1 tổng | T thấy +1 | VIEW-03, 10 |
| E-29 | A mở W ở hai tab gần như cùng lúc | Hai tab hiện thân bài | +1 tổng, kể cả khi hai yêu cầu tới server đồng thời | T thấy +1 | VIEW-03 |
| E-30 | Khách G mở W (+1), đăng nhập thành A rồi mở lại W trong cửa sổ | Thân bài | Lần hai +1: #341 không gộp mã khách với tài khoản | T thấy +2 | VIEW-04 |
| E-31 | Trình duyệt tắt JavaScript mở W | HTML server có thân bài, đọc bình thường | +0 | T không thấy lượt này; nhãn nói rõ giới hạn | VIEW-02, 12 |
| E-32 | A mở W, thân bài hiện, mất mạng trước khi gửi xong; online lại trong cùng tab | Đọc bình thường | +1 nếu thử lại thành công trong giới hạn; +0 nếu hết lượt thử hoặc đã đóng tab; không bao giờ +2 | T thấy +1 hoặc không đổi | VIEW-10 |
| E-33 | Ghi lượt lỗi phía server | Đọc bình thường, không thấy lỗi nào | +0 (lượt có thể mất) | T thấy số không đổi, hoặc “—” nếu đọc số cũng lỗi | VIEW-10, 12 |
| E-34 | Thân bài hiện khi W còn công khai, nhưng sự kiện tới server sau khi T đã chuyển W sang “Chỉ mình tôi” | Đọc bình thường | +0 | T thấy số cũ | VIEW-08 (kiểm trạng thái tại lúc ghi), VIEW-10 |
| E-35 | A có lượt cho W khi r1 công khai; 2 giờ sau r2 thay r1, A mở W | Thân bài r2 | +0 (cùng `WorkId`, còn trong cửa sổ) | Tổng của W không reset | VIEW-03, 06 |
| E-36 | A đang ở trang ẩn của W; T công khai lại và trang tự chuyển sang thân bài | Thân bài | OQ-3 (khuyến nghị +1 nếu ngoài cửa sổ) | — | OQ-3 |
| E-37 | Tab của A đã mở W từ hôm trước; trang nhận bản mới và A chọn áp dụng bản mới | Thân bài bản mới | +0: không có thao tác mở mới, thân bài đã hiện trong trang này | Không đổi | VIEW-01, 02, 06 |

## 7. Vòng đời theo trạng thái (AC-03)

Độc giả luôn đọc được bài khi thống kê lỗi, dù lỗi ghi hay lỗi đọc số.

| # | Trạng thái tác phẩm | Độc giả mở URL | Lượt có thể tăng? | Tác giả thấy ở Bàn viết |
| --- | --- | --- | --- | --- |
| L-01 | Nháp chưa từng công khai, kể cả đang chờ duyệt lần đầu | Không tìm thấy (404, PUB-05 D) | Không | Không có số. Nếu ô Lượt xem được hiện (OQ-1, lượt thiết kế), hiện “—” kèm lý do chưa công khai, không hiện “0”. |
| L-02 | Công khai sau mốc bật, chưa ai xem | Thân bài | Có | “0” thật, chỉ khi đọc số thành công. |
| L-03 | Công khai, có số | Thân bài | Có | Số định dạng kiểu Việt, ví dụ “1.234”, nhãn “Lượt xem”. |
| L-04 | Số chưa cập nhật | Thân bài | Có | Số cũ hơn khoảng tối đa 1 phút; không phải lỗi; không tự cập nhật trực tiếp; lần tải sau thấy số mới. |
| L-05 | Thống kê lỗi | Thân bài, đọc bình thường | Lượt có thể mất | “—” kèm lý do không tải được. Tên, trạng thái bài và link sửa vẫn dùng được. |
| L-06 | Sửa sang revision mới: r2 chờ duyệt khi r1 công khai, rồi r2 thay r1 | r1, rồi r2 | Có | Tổng cộng dồn, không reset khi đổi revision (khác Thấm). |
| L-07 | Chuyển “Chỉ mình tôi” (trang ẩn, PUB-07) | “Nội dung đã được ẩn” | Không | Số cũ, không tăng. OQ-1: hàng có thể chỉ hiện Lượt xem. |
| L-08 | Rút sau duyệt (`O-17@r2`), kể cả khi tác giả chuyển riêng tư sau đó | “Bản này đã bị gỡ sau khi kiểm duyệt.” hoặc trang đóng | Không | Số cũ, không tăng. |
| L-09 | Công khai lại, cùng revision hay revision mới | Thân bài | Có, đếm tiếp | Số cũ cộng lượt mới. Người đã có lượt trong 24 giờ trước khi ẩn vẫn bị chặn tới hết cửa sổ. |
| L-10 | Tác giả xóa tác phẩm (target, PUB-04; OQ-2) | Trang thay thế “Nội dung đã được tác giả xoá” | Không | Số liệu bị xóa; không còn số để hiện. |
| L-11 | Xóa tài khoản tác giả (target, SOC-LIFE-02; OQ-2) | Trang thay thế theo SOC-LIFE-02 | Không | Số liệu mọi tác phẩm của tài khoản bị xóa. Tổng ở tác phẩm người khác không bị trừ vì không lưu theo người; mã chống trùng của tài khoản đó tự hết hạn trong tối đa 24 giờ. |
| L-12 | Bài đã công khai trước mốc bật | Thân bài | Có, từ mốc | Số kèm nhãn “Tính từ dd/mm/yyyy”. |
| L-13 | Bài công khai lần đầu sau mốc | Thân bài | Có | Không cần nhãn “Tính từ”. |
| L-14 | Admin gỡ cả bài vì vi phạm (PUB-05 C, chưa có runtime) | Không tìm thấy | Không | OQ-4. |

## 8. Trải nghiệm người dùng (nghĩa, không phải thiết kế thị giác)

Vị trí con số, bố cục, kiểu chữ, icon và mockup thuộc lượt thiết kế sau (LayoutSpec trong
`docs/design/`, chưa tạo). Phần dưới đây là nghĩa mà thiết kế phải giữ.

- **Tên:** “Lượt xem”. Số định dạng đầy đủ kiểu Việt, dấu chấm phân cách hàng nghìn (1.234,
  12.345). Có rút gọn số lớn hay không do lượt thiết kế quyết và phải giữ nghĩa.
- **“0” và “—”:** “0” chỉ khi đọc số thành công và tổng thật bằng 0. “—” luôn kèm lý do đọc
  được, ví dụ chưa công khai hoặc chưa tải được số liệu. Không hiện “0” thay cho lỗi.
- **Nhãn “Tính từ dd/mm/yyyy”** cho bài công khai trước mốc bật. Ngày hiển thị theo
  `Asia/Ho_Chi_Minh`, như quy ước hiển thị ngày hiện có của Web (`discovery.rs::first_public_date`).
- **Độ phủ:** phần giải thích của con số phải nói được ba điều: mỗi người đọc tính tối đa một lần
  trong 24 giờ; khách được tính theo trình duyệt; lượt của chính tác giả và trình duyệt tắt
  JavaScript không được tính. Câu đề xuất cho lượt thiết kế, chưa phải copy cuối: “Số lần bài
  được mở đọc, mỗi người tối đa một lần trong 24 giờ. Khách được tính theo trình duyệt; lượt bạn
  tự xem và trình duyệt tắt JavaScript không được tính.”
- **Không** cập nhật trực tiếp, không thông báo, không số ở Reader/Khám phá/hồ sơ, không danh
  sách người xem.
- **Reader không đổi gì** về giao diện. Lỗi ghi lượt không hiện cho độc giả và không chặn mở,
  đọc, cuộn, bình luận hay quay lại.

**Ranh giới thiết kế trước runtime.** Đường ghi (#341) và đọc theo quyền (#342) đã có, nhưng chưa có
tín hiệu Reader (#343) nên runtime chưa đếm lượt nào. Màn hình mẫu dùng dữ liệu mẫu có nhãn, ví dụ
hai fixture ở [registry §9.3](../development/work-views.md#93-hợp-đồng-và-ví-dụ). Nghiệm thu
đầu-cuối cần #343.

## 9. Dữ liệu, quyền riêng tư và lưu giữ

- **Được lưu (P-11 B), và chỉ ba loại:** tổng theo tác phẩm; tổng theo ngày theo tác phẩm, không
  theo người và chưa có biểu đồ; mã chống trùng đã băm, tự xóa khi hết cửa sổ 24 giờ. Nếu kỹ thuật
  cần giữ mã sự kiện để chống gửi lại, mã đó thuộc loại thứ ba và hết hạn cùng cửa sổ.
- **Không lưu:** IP thô, fingerprint, danh sách người đã xem, lịch sử đọc theo người. Tín hiệu
  dùng để lọc bot hay prefetch (CHOICE-06) không được lưu thành dữ liệu theo người.
- **Mã khách:** ngẫu nhiên, first-party, thời hạn 24 giờ, chỉ dùng chống trùng Lượt xem.
- **Số liệu là dữ liệu riêng của tác giả:** không vào HTML SSR, cache công khai, IndexedDB/bộ nhớ
  đọc công khai hay JSON Reader công khai; đọc bằng phiên của tác giả với `no-store`.
- **Xóa:** xóa tác phẩm hay tài khoản thì xóa số liệu tương ứng (target, OQ-2).
- **Mốc ngày của tổng theo ngày:** dùng `Asia/Ho_Chi_Minh` như quy ước hiển thị ngày của Web.
  Đây là ràng buộc thiết kế, không phải lời chốt riêng; lượt có biểu đồ có thể hỏi lại.
- **Log:** đường ghi Lượt xem không đưa IP thô hay mã khách chưa băm vào dữ liệu hoặc log của
  riêng nó. Log truy cập của hạ tầng hiện có nằm ngoài phạm vi và không được dùng để đếm hay
  backfill.
- **AI:** Lượt xem không gửi gì tới AI provider và không chạm nội dung nháp (MOD-05, INV-08).

## 10. Ràng buộc cho lựa chọn kỹ thuật (CHOICE-01..06)

Các lựa chọn do issue tương ứng chọn bằng bằng chứng. #341 đề xuất CHOICE-02 A, CHOICE-03 và
luật CHOICE-06 A (`DESIGN_PROPOSED`, lý do và giới hạn ở
[registry §7](../development/work-views.md#7-đường-ghi-341)); #342 đề xuất CHOICE-05 A
([registry §9.1](../development/work-views.md#91-choice-05-design_proposed)); các lựa chọn còn lại
`OPEN`. Lựa chọn
nào làm đổi nghĩa số, quyền riêng tư, độ trễ hay khả năng mất lượt thì quay lại chủ sản phẩm.

| CHOICE | Issue | Ràng buộc nghiệp vụ phải thỏa |
| --- | --- | --- |
| CHOICE-01 Điểm phát | #343 | Phát tại điểm thân bài công khai đã hiện (`ReaderResponse::Published` đã render), không tại trang thay thế, trang lỗi hay fallback “Đang mở…”. Không cộng mọi GET, SSR, render, hydration, prefetch hay lần xác thực lại nền. P-02 chấp nhận không đếm no-JS nên không cần tín hiệu server cho no-JS. |
| CHOICE-02 Lưu và cộng | #341 | Một hiệu ứng cho mỗi người đọc, tác phẩm và cửa sổ, kể cả khi yêu cầu đến đồng thời, mất phản hồi sau commit, rollback hay khởi động lại. Kiểm tại lúc ghi: tác phẩm còn công khai, người đọc không phải tác giả. Tổng và tổng theo ngày khớp nhau. Nếu tổng hợp trễ, hội tụ trong kỳ vọng khoảng 1 phút. |
| CHOICE-03 Định danh | #341 | Thành viên: `PrincipalId` từ phiên. Khách: mã ngẫu nhiên first-party, thời hạn 24 giờ. Không IP thô, không fingerprint. Chọn và ghi rõ: mã khách hết hạn cố định hay được làm mới khi dùng (nếu làm mới khiến mã sống liên tục quá 24 giờ thì hỏi lại chủ sản phẩm); hành vi khách → đăng nhập (E-30); phiên không hợp lệ hay bị đình chỉ, với điều kiện không cản đọc. |
| CHOICE-04 Gửi lỗi và retry | #343 | Retry hữu hạn trong tab với cùng mã sự kiện. Không bắt buộc hàng đợi bền qua lần đóng tab. Retry không bao giờ cộng trùng; retry tới sau khi bài hết công khai thì +0. |
| CHOICE-05 Đọc và cache | #342 | Phân biệt “0” với chưa có/lỗi. Lỗi số liệu không phá tên, trạng thái bài và link sửa. Không N+1. Số liệu tác giả không vào SSR hay cache công khai; việc tăng số không buộc tải lại toàn văn công khai; `ETag`/`304` vẫn đúng với quyền hiện tại. |
| CHOICE-06 Chống tăng số giả | #341 (luật), #343 (áp tại HTTP) | Kiểm input, chống gửi lặp, giới hạn tần suất, lọc bot và prefetch nhận diện được. Không hứa loại mọi bot. Không thêm loại định danh ngoài CHOICE-03 nếu chưa hỏi chủ sản phẩm. |

## 11. Ranh giới phạm vi

**Trong v1:** luật đếm VIEW-01..06, số chỉ tác giả thấy ở Bàn viết (VIEW-07..09, 12), độ trễ và
lưu giữ (VIEW-10, 11), tín hiệu trên Reader Web.

**Ngoài phạm vi:** code, migration và API trong PR tài liệu này; vị trí, visual và mockup của
con số (lượt thiết kế sau); số công khai ở Reader, Khám phá hay hồ sơ; danh sách người xem;
dashboard và biểu đồ; xếp hạng; Notifications; CMP/native; export; văn bản chính sách quyền riêng
tư hay cookie nói chung. Các mục này cần quyết định riêng trước khi làm.

## 12. Owner và bằng chứng

| Loại | Owner |
| --- | --- |
| Yêu cầu có ID | [PRD §17](../../prd/requirements.md#17-lượt-xem-tác-phẩm-work-views) |
| Luật, ví dụ, vòng đời | Tài liệu này |
| Bản ghi quyết định | [VDR-WORK-VIEWS@r1](../../decisions/proposals/VDR-WORK-VIEWS.r1.toml), [signoff](../../decisions/signoffs/VDR-WORK-VIEWS.r1.toml) `PRODUCT_CONFIRMED` |
| Mức triển khai, bản đồ issue, kịch bản xuyên suốt | [Registry phát triển](../development/work-views.md) |
| Code tương lai | `services/vink-app` (domain, application, PostgreSQL, HTTP), `apps/vink-web` (tín hiệu Reader, Bàn viết), `services/vink-app/contracts/http/v1` |

Bằng chứng (kiểm tài liệu, unit, PostgreSQL) ghi ở registry.
