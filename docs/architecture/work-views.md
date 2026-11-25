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
cho bộ khuyến nghị r1 (`P-01..P-11`, [VDR-WORK-VIEWS@r1](../../decisions/proposals/VDR-WORK-VIEWS.r1.toml),
đã [signoff](../../decisions/signoffs/VDR-WORK-VIEWS.r1.toml) cùng ngày). Ngày 25/11/2026, chủ
sản phẩm chốt bổ sung trên [#323](https://github.com/loveoverflowcom/vink/issues/323) và `#340`:
yêu cầu thân bài công khai hiển thị tối thiểu theo ngưỡng cấu hình chung mặc định **5 giây (`5.000 ms`)**,
chốt `OQ-1..OQ-4`, bộ luật `UX-01..10`, giữ riêng tư chỉ tác giả thấy, lộ trình 2 PR (`#340+#343` rồi
`#514`), và hai quyết định Bàn viết `D-01` (thứ tự cố định **Thấm → Nhận xét → Lượt xem**) và
`D-02` (bỏ hẳn nút/link “Đọc” / “Đọc bản công khai”). Delta ngày 25/11/2026 nằm ở
[VDR-WORK-VIEWS@r2](../../decisions/proposals/VDR-WORK-VIEWS.r2.toml) (`PENDING` chờ signoff đúng
revision/SHA-256 theo `VDR-DECISION-SIGNOFF@r1`); `@r1` vẫn là marker active cho tới khi `@r2`
được signoff.

| Chiều | Trạng thái |
| --- | --- |
| Quyết định sản phẩm | `r1` (`P-01..P-11`, `VIEW-01..12`): `PRODUCT_CONFIRMED` ngày 21/11/2026 ([signoff](../../decisions/signoffs/VDR-WORK-VIEWS.r1.toml)). Delta 25/11/2026 (`#323`/`#340`: ngưỡng 5s cấu hình chung, `OQ-1=A`, `OQ-2`, `OQ-3A/B`, `OQ-4`, `UX-01..10`, `D-01`/`D-02`, 2 PR) được chủ sản phẩm chốt trên issue; proposal [VDR-WORK-VIEWS@r2](../../decisions/proposals/VDR-WORK-VIEWS.r2.toml) `PENDING` chờ signoff đúng payload/SHA-256. |
| Quyết định kiến trúc | #341 đề xuất CHOICE-02 A, CHOICE-03 và CHOICE-06 A (`DESIGN_PROPOSED`, [registry §7](../development/work-views.md#7-đường-ghi-341)); #342 đề xuất CHOICE-05 A ([registry §9.1](../development/work-views.md#91-choice-05-design_proposed)); #343 (PR1) đề xuất CHOICE-01, CHOICE-04 và CHOICE-POLICY ([registry §10](../development/work-views.md#10-thu-nhận-http-và-tín-hiệu-reader-343)). Chưa lựa chọn nào `Accepted`. |
| Mức triển khai | Xem [registry phát triển](../development/work-views.md) cho mức hiện tại của `#341`, `#342`, `#343` (PR1) và phần giao diện Bàn viết `#514` (PR2). |
| Bằng chứng | Xem [registry phát triển](../development/work-views.md). |

Các luật đã có và đặc tả này giữ nguyên: WR-01 (nháp riêng), UX-01 (Reader có HTML từ server),
AUD-01/02 (hai audience, audience không phải trạng thái xuất bản), PUB-04/05/07 (xóa, riêng tư,
trang ẩn), MOD-05 và INV-08 (nháp riêng không lọt ra ngoài phạm vi),
[`O-17@r2`](../../decisions/proposals/O-17.r2.toml) (trang rút sau kiểm duyệt trả HTTP 200).
Mã HTTP không phải bằng chứng người đọc đã xem thân bài.

## 2. Định nghĩa và thuật ngữ

> **Một lượt xem** là một lần người đọc chủ động mở trang đọc công khai `/tac-pham/{id}` của
> một tác phẩm, thân bài công khai đã hiện trên trình duyệt có JavaScript **và** hiển thị ở tab
> foreground/visible đủ tối thiểu ngưỡng cấu hình dùng chung mặc định **5 giây (`5.000 ms`)**
> trong cùng lần mở đó (tính bằng đồng hồ monotonic; tạm dừng khi tab ẩn, tiếp tục khi hiện; hủy
> khi rời trang, thay bài/revision, mất công khai hay đổi authority phiên). Người đọc không phải
> tác giả đang đăng nhập của bài. Mỗi người đọc được tính tối đa một lượt cho mỗi tác phẩm trong
> 24 giờ trượt. Số lượt cộng dồn theo tác phẩm.

“Lượt xem” **không** phải số người duy nhất: một khách dùng hai trình duyệt có thể thành hai lượt.
Nó cũng không phải số người đọc hết bài, thời gian đọc thực sự của mắt người, hay số request tới
server. Ngưỡng 5 giây là điều kiện cần ở phía trình duyệt để lọc lướt chạm vô tình; không phải
bằng chứng xác thực chú ý phía server và không thay thế luật chống trùng 24 giờ, tự xem hay kiểm
tra công khai tại lúc commit.

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
| Ngưỡng hiển thị tối thiểu | `WORK_VIEW_MIN_VISIBLE_MS` (`5_000 ms`) trong `vink-work-view-policy` | có (shared crate, PR1 #343) | Một nguồn cấu hình duy nhất cho timer Reader, giải thích UI (`UX-07`) và test. |
| Thân bài công khai đã hiện | `ReaderResponse::Published` → `PublicationView` trong [`pages/mod.rs`](../../apps/vink-web/src/pages/mod.rs) | có sẵn (điểm hiển thị) | Điều kiện khởi tạo đồng hồ foreground trong cùng lần mở. |
| Đang mở | `Suspense` fallback “Đang mở tác phẩm…” trong `ReaderScope` | có sẵn | Rời trang ở đây không sinh lượt và không cộng vào thời gian hiển thị. |
| Tải lỗi | `LoadError::NotFound` (404), lỗi khác (503, `Unavailable`) | có sẵn | Không sinh lượt. |
| Trang thay thế | `ReaderResponse::Hidden` (trang ẩn, PUB-07) và `ReaderResponse::ReviewWithdrawn` (`O-17@r2`) → `ReplacementView` | có sẵn | Trả HTTP 200 nhưng không có thân bài, nên không sinh lượt (và hủy đồng hồ đang chạy nếu có). |
| Người đọc (định danh đo lường) | `ViewReader`: thành viên `PrincipalId` từ phiên, khách `GuestToken` | có (core, #341) | Một định danh đo lường, không phải một người thật. Hai loại không gộp (E-30). |
| Mã khách | `GuestToken` (UUID v4), cookie `__Host-vink_guest` | có (core #341; cookie #343) | Ngẫu nhiên, first-party, thời hạn 24 giờ cố định, chỉ dùng chống trùng Lượt xem. |
| Mã chống trùng đã băm | `ReaderDigest` (HMAC theo khóa server `VINK_WORK_VIEW_DIGEST_KEY`), bảng `work_view_windows` | có (core + PostgreSQL) | Dạng băm của người đọc gắn với tác phẩm; tự xóa khi hết cửa sổ. |
| Mã sự kiện | `ViewEventId` (UUIDv7), receipt `work_view_receipts` | có (core + PostgreSQL) | Phát một lần tại đúng ngưỡng; gửi lại cùng mã không bao giờ cộng thêm (P-03, P-10). |
| Tổng theo tác phẩm, tổng theo ngày | `work_view_totals`, `work_view_days` | có (PostgreSQL); tổng đọc qua API tác giả (#342), tổng ngày chưa đọc | Không theo người (P-11). |
| Mốc bật | `work_view_counting.started_at` | có (PostgreSQL); `views.counted_since` (#342); nhãn hiển thị thuộc #514 | Ngày bắt đầu đếm; nhãn “Tính từ dd/mm/yyyy” (P-09). |
| Bàn viết | `/ban-viet`, `GET /api/v1/author/works` ([read-model](../development/desk-author-feedback.md)) | có sẵn (API); UI Lượt xem + `D-01/D-02` ở #514 | Nơi tác giả thấy số; thứ tự cố định Thấm → Nhận xét → Lượt xem, bỏ nút “Đọc”. |

## 3. Bảng quyết định P-01..P-11

`r1`: chủ sản phẩm, 21/11/2026, “Theo khuyến nghị” cho brief #340. Delta 25/11/2026 (`#323`, `#340`):
cập nhật VIEW-01 (ngưỡng 5 giây foreground cấu hình tập trung), VIEW-07/12 (`D-01`/`D-02` và `UX-01..10`)
và `OQ-1..4`.

| PRD | #323 | Lựa chọn | Luật áp dụng | Ranh giới, khác biệt |
| --- | --- | --- | --- | --- |
| VIEW-01 | P-01 B + delta 25/11 | **B + ngưỡng 5s** | +1 khi thân bài công khai đã hiện sau thao tác mở hợp lệ **và** hiển thị ở tab foreground/visible tối thiểu theo ngưỡng cấu hình chung (mặc định `5.000 ms`, đồng hồ monotonic, tạm dừng khi ẩn, tiếp tục khi hiện; hủy/reset khi rời/unmount, đổi bài/revision thực sự, mất công khai hoặc đổi authority phiên). Tại đúng ngưỡng phát một ứng viên; dưới ngưỡng không gửi. Không tính tải trang, SSR HTML, hydration, prefetch, re-render hay refresh nền; không yêu cầu đọc hết hay cuộn. Bấm rồi hủy, tải lỗi, trang thay thế (kể cả HTTP 200) = +0. Tên hiển thị “Lượt xem”. | Ngưỡng là điều kiện cần phía trình duyệt, tách một nguồn cấu hình dùng chung; không đo đọc hết. |
| VIEW-02 | P-02 B | **B** | Link nội bộ, URL trực tiếp, link chia sẻ, tab mới khi thỏa VIEW-01. Chuột, chạm, Enter như nhau. Hover, prefetch, prerender, bot tạo bản xem trước link = +0. Trình duyệt tắt JavaScript không được đếm. | Giới hạn no-JS phải hiện cho tác giả (VIEW-12). Tab mở nền chờ foreground (OQ-3B). |
| VIEW-03 | P-03 C | **C** | Tối đa 1 lượt/người đọc/tác phẩm trong 24 giờ trượt ([§2](#2-định-nghĩa-và-thuật-ngữ)). Refresh (là lần mở mới, vẫn dedup 24h), Back/Forward, mở lại trong cửa sổ = +0. Không cộng dồn thời gian giữa nhiều lần mở hay nhiều tab. Gửi trùng kỹ thuật không bao giờ thành lượt mới. | Hai phép kiểm riêng: gộp lần mở trong cửa sổ, và idempotency của cùng một sự kiện. |
| VIEW-04 | P-04 A | **A** | Có. Chống trùng bằng mã ngẫu nhiên first-party, thời hạn cố định = cửa sổ P-03 (không làm mới). Không IP thô, không fingerprint. | Chấp nhận đếm lại khi xóa cookie, đổi trình duyệt/máy, hoặc khách đăng nhập giữa cửa sổ. |
| VIEW-05 | P-05 B | **B** | Không tính khi phiên là tác giả của bài. Màn kiểm duyệt, preview, nháp không bao giờ tính. Admin mở Reader công khai như độc giả thì tính. | Tác giả đã đăng xuất được tính như khách; không hứa nhận diện. Luật theo bề mặt: moderator ở Reader công khai cũng như độc giả. |
| VIEW-06 | P-06 A | **A** | Cộng dồn theo `WorkId` qua sửa, đăng lại, revision mới. | Khác Thấm (gắn `RevisionId`): revision mới không reset Lượt xem. Đổi revision thực sự giữa chừng khi chưa đạt 5s thì hủy bộ đếm hiện tại (VIEW-01). |
| VIEW-07 | P-07 A + D-01/D-02 | **A + D-01/D-02** | Chỉ tác giả thấy, riêng tư tại Bàn viết. Không số công khai, không danh sách người xem. Hàng Bàn viết cố định thứ tự **Thấm → Nhận xét → Lượt xem** (`D-01`) và **bỏ hẳn nút/link “Đọc” / “Đọc bản công khai”** (`D-02`, không nút/icon/menu/placeholder/click số liệu; giữ link mở bản thảo/editor). | Thiết kế thị giác và triển khai UI Bàn viết nằm ở `#514` (PR2). |
| VIEW-08 | P-08 A + OQ-1 A + OQ-4 | **A** | Bài bị ẩn, chuyển “Chỉ mình tôi” (`PUB-07`), bị rút sau kiểm duyệt (`O-17@r2`), hay bị admin gỡ cả bài (`PUB-05 C`, OQ-4): tác giả vẫn thấy số Lượt xem cũ nếu còn quyền, số không tăng thêm (dù Thấm/Nhận xét ẩn khi riêng tư, OQ-1 A). Công khai lại thì đếm tiếp. Mở trang thay thế không tính. | Xóa tác phẩm thì xóa số liệu theo OQ-2 / VIEW-11. |
| VIEW-09 | P-09 A | **A** | Bắt đầu từ ngày bật. Bài đăng trước mốc (`counted_since != null`) có nhãn “Tính từ dd/mm/yyyy” theo `Asia/Ho_Chi_Minh`. Không backfill. | Chưa từng có nguồn ([#214](https://github.com/loveoverflowcom/vink/issues/214) loại tracking). Không suy từ Thấm/Nhận xét hay log truy cập. |
| VIEW-10 | P-10 A | **A** | Số tham khảo; kỳ vọng tác giả thấy trong khoảng 1 phút, không phải SLA. Khi đủ ngưỡng, gửi một mã UUIDv7 và retry hữu hạn trong tab cùng mã sự kiện. | Đóng tab hoặc offline có thể mất lượt; không bao giờ cộng trùng. Lỗi thống kê không bao giờ cản người đọc. |
| VIEW-11 | P-11 B + OQ-2 | **B** | Tổng/tác phẩm, tổng theo ngày/tác phẩm (không theo người, chưa có biểu đồ), mã chống trùng và biên nhận đã băm tự xóa khi hết cửa sổ. Xóa tác phẩm hoặc tài khoản thì xóa số liệu (kích hoạt ở PR đầu tiên có route xóa, OQ-2). | Không áp tuổi 180 ngày của Notifications. Tách khỏi `O-08` purge vật lý. |
| VIEW-12 | UX-01..10 + D-01/D-02 | **Chốt 25/11** | Chưa có hoặc lỗi = “—” kèm lý do, khác “0” thật (`counted=0`, mẫu [VDR-AUTHOR-WORK-FEEDBACK@r1](../../decisions/proposals/VDR-AUTHOR-WORK-FEEDBACK.r1.toml)). Định dạng đầy đủ `1.234`, icon mắt Material Symbols (`visibility`), đặt cuối trong **Thấm → Nhận xét → Lượt xem**, trung tính R4/Tailwind; giải thích chạm/bàn phím/screen reader dùng đúng ngưỡng từ nguồn cấu hình chung + 24h + khách + tự xem/no-JS; cập nhật khi làm mới trang, không realtime/poll. | Chi tiết `UX-01..10` ở [§8](#8-trải-nghiệm-người-dùng-nghĩa-và-quy-ước-ux-0110). |

## 4. Quyết định bổ sung ngày 25/11/2026 (`OQ-1..OQ-4`, `D-01`/`D-02`)

Ngày 25/11/2026, chủ sản phẩm chốt toàn bộ `OQ-1..OQ-4`, giữ số liệu riêng tư, duyệt lộ trình
2 PR và bổ sung `D-01`/`D-02` cho hàng tác phẩm Bàn viết (ghi vào `VDR-WORK-VIEWS@r2` `PENDING`):

| ID | Kết luận đã chốt (25/11/2026) | Hệ quả kỹ thuật & phạm vi |
| --- | --- | --- |
| **OQ-1** | **Chọn A:** giữ số **Lượt xem** cũ cho tác giả khi bài riêng tư (`Chỉ mình tôi`) hoặc bị rút sau kiểm duyệt nếu còn quyền tác giả; giữ nguyên luật ẩn Thấm và Nhận xét hiện hành của [VDR-AUTHOR-WORK-FEEDBACK@r1](../../decisions/proposals/VDR-AUTHOR-WORK-FEEDBACK.r1.toml). | Hàng Bàn viết của bài riêng tư có thể chỉ hiện Lượt xem (vẫn giữ vị trí metric hợp lệ, không thêm slot giả cho Thấm/Nhận xét ẩn). |
| **OQ-2** | PR đầu tiên triển khai route/luồng **xóa tác phẩm** (`PUB-04/05`) hoặc **xóa tài khoản** (`SOC-LIFE-02`) vào runtime **bắt buộc** gọi xóa số liệu lượt xem liên quan (`erase_work` / xóa theo `WorkId` trong cùng transaction) kèm test kiểm chứng. | Không chặn Epic #323 khi route xóa chưa tồn tại; không tự mở scope thêm route xóa trong Epic và không quyết định `O-08` (purge vật lý của tác phẩm) thay owner. |
| **OQ-3** | **OQ-3A:** Trang đang ở màn thay thế (`Hidden` / `ReviewWithdrawn`) được công khai lại do cập nhật nền của `ReaderLoaded` thì **không** tự đếm; cần thao tác mở mới. **OQ-3B:** Tab mở nền (`visibilityState = hidden`) **không** đếm cho tới khi người dùng đưa tab ra foreground, thân bài công khai hiện và hiển thị đủ ngưỡng; thời gian ẩn không tính. | Tracker Reader khóa cờ mở hợp lệ khi trang khởi đầu ở `ReplacementView` hoặc rời khỏi `Published`, và chỉ chạy đồng hồ monotonic khi `document.visibilityState !== "hidden"` & `!document.prerendering`. |
| **OQ-4** | Khi admin gỡ cả bài (`PUB-05 C`), tác giả vẫn giữ số Lượt xem cũ ở chế độ riêng tư khi còn quyền xem và số không tăng thêm (độc giả nhận 404 nên +0); chỉ khi tác phẩm bị xóa thì xóa dữ liệu theo OQ-2. | Giữ nguyên luật đọc `AUTHOR_VIEWS_SQL` theo `WorkId` / `owner_id` (#342); route admin remove là prerequisite tương lai không nằm trong PR này. |
| **D-01** | Thứ tự cố định trên hàng tác phẩm Bàn viết cho mọi metric đang được phép hiển thị: **Thấm → Nhận xét → Lượt xem** (Lượt xem luôn đứng cuối). | PR1 chốt SSOT; PR2 (#514) sửa UI Bàn viết và đồng bộ tài liệu thiết kế. |
| **D-02** | Bỏ hẳn nút/link **“Đọc” / “Đọc bản công khai”** trong hàng tác phẩm Bàn viết (không chỉ bỏ icon mắt); không thay bằng nút, icon, menu hay click vào số liệu; bỏ vùng giữ chỗ và tab stop cũ. | Giữ nguyên liên kết mở bản thảo/editor của hàng/nhan đề và giữ nguyên Reader công khai cũng như mọi đường vào Reader khác. Triển khai UI và bỏ wiring thừa ở `#514`. |

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
giả; người đọc chưa có lượt nào cho W trong 24 giờ; trình duyệt có JavaScript; ngưỡng tối thiểu
đang cấu hình là `WORK_VIEW_MIN_VISIBLE_MS = 5.000 ms` và khi nói “đạt ngưỡng” tức thân bài công
khai đã hiện liên tục/cộng dồn ở tab foreground/visible đủ `≥ 5.000 ms` trong cùng lần mở. Cột
“Ai thấy gì”: chỉ T thấy số, ở Bàn viết, kỳ vọng trong khoảng 1 phút. Người đọc không thấy số và
Reader không có gì mới.

| # | Đầu vào | Người dùng trải qua | Số đổi | Ai thấy gì | Luật |
| --- | --- | --- | --- | --- | --- |
| E-01 | Thành viên A bấm chuột vào thẻ W ở Khám phá và đọc đủ 5s | “Đang mở tác phẩm…” rồi thân bài hiện ở tab foreground `≥ 5.000 ms` | +1 | T thấy +1 | VIEW-01, 02 |
| E-02 | A chạm thẻ W trên điện thoại và ở lại đủ 5s | Như E-01 | +1 | T thấy +1 | VIEW-02 |
| E-03 | A dùng Tab tới link W rồi nhấn Enter và ở lại đủ 5s | Như E-01 | +1 | T thấy +1 | VIEW-02 |
| E-04 | Khách G dán URL `/tac-pham/W` vào thanh địa chỉ và ở lại đủ 5s | HTML từ server đã có thân bài, hydration, rồi ở lại `≥ 5.000 ms` foreground | +1 sau khi hydration chạy bộ đếm foreground đủ ngưỡng; HTML server tự nó không là lượt | T thấy +1 | VIEW-01, 02 |
| E-05 | G mở link chia sẻ W từ một ứng dụng chat và ở lại đủ 5s | Như E-04 | +1 | T thấy +1 | VIEW-02, 04 |
| E-06 | Ứng dụng chat tự tải link để tạo bản xem trước | Không có ai mở bài | +0 | Không đổi | VIEW-02 |
| E-07 | A Ctrl/Cmd+click hoặc chọn “Mở trong tab mới” (mở nền) | Tab mở ở nền (`hidden`), 10s sau A mới chuyển sang tab và ở lại thêm 5s | +0 lúc còn ẩn; +1 sau khi lên foreground đủ 5s (OQ-3B) | T thấy +1 | VIEW-02, OQ-3B |
| E-08 | A rê chuột qua link W; trình duyệt hoặc router prefetch / prerender | Không mở bài ở foreground; `document.prerendering = true` hoặc `Sec-Purpose: prefetch` | +0 | Không đổi | VIEW-02 |
| E-09 | A bấm W rồi Back khi còn “Đang mở tác phẩm…” | Không thấy thân bài | +0 | Không đổi | VIEW-01 |
| E-10 | A bấm W và tải lỗi: 503 “Chưa tải được tác phẩm”, mất mạng, hoặc 404 | Trang lỗi | +0 | Không đổi | VIEW-01 |
| E-11 | Sau E-10, A tải lại và thân bài hiện đủ 5s | Thân bài foreground `≥ 5.000 ms` | +1 (lượt đầu của A trong cửa sổ) | T thấy +1 | VIEW-01, 03 |
| E-12 | A mở URL của W khi W đã chuyển “Chỉ mình tôi” (HTTP 200) | “Nội dung đã được ẩn” | +0 | T thấy số cũ | VIEW-01, 08 |
| E-13 | A mở URL của W đã bị rút sau kiểm duyệt (`O-17@r2`, HTTP 200) | “Bản này đã bị gỡ sau khi kiểm duyệt.” | +0 | T thấy số cũ | VIEW-01, 08 |
| E-14 | A có lượt lúc 08:00, refresh lúc 08:05 và ở lại đủ 5s | Thân bài | +0 (đã tính trong 24h) | Không đổi | VIEW-03 |
| E-15 | A Back khỏi W lúc 08:10 rồi Forward lại đủ 5s | Thân bài | +0 | Không đổi | VIEW-03 |
| E-16 | A mở lại W lúc 20:00 cùng ngày và lúc 07:59 hôm sau (mỗi lần đủ 5s) | Thân bài | +0, +0 | Không đổi | VIEW-03 (cửa sổ không kéo dài) |
| E-17 | A mở lại W lúc 08:01 hôm sau và ở lại đủ 5s | Thân bài foreground `≥ 5.000 ms` | +1 | T thấy tổng +2 so với trước 08:00 hôm trước | VIEW-03 |
| E-18 | Bot tìm kiếm hoặc crawler không chạy JavaScript tải W | — | +0 | Không đổi | VIEW-02. Trình duyệt tự động có chạy JavaScript có thể lọt; không hứa loại mọi bot |
| E-19 | Khách G mở W và ở lại đủ 5s | Thân bài foreground `≥ 5.000 ms` | +1 theo mã khách | T thấy +1 | VIEW-04 |
| E-20 | G xóa cookie, hoặc mở bằng trình duyệt/máy khác, rồi mở lại W sau 1 giờ đủ 5s | Thân bài | +1 (chấp nhận đếm lại) | T thấy +1 | VIEW-04 |
| E-21 | Thành viên A mở W đủ 5s trên điện thoại, 1 giờ sau mở đủ 5s trên máy tính cùng tài khoản | Thân bài | +1 rồi +0 | T thấy +1 | VIEW-03; định danh thành viên là `PrincipalId` |
| E-22 | T đang đăng nhập mở W ở Reader đủ 5s | Thân bài | +0 | T thấy số không đổi | VIEW-05 |
| E-23 | T mở nháp hoặc xem trước của W ở Bàn viết | Nháp, bản xem trước | +0 | Không đổi | VIEW-05 |
| E-24 | T đăng xuất rồi mở W đủ 5s | Thân bài | +1, tính như khách | T (khi đăng nhập lại) thấy +1 | VIEW-05, 04 |
| E-25 | Admin hoặc moderator mở ca duyệt, bản gửi duyệt của W | Màn kiểm duyệt | +0 | Không đổi | VIEW-05 |
| E-26 | Admin mở W ở Reader công khai như độc giả đủ 5s | Thân bài | +1 (một lần trong cửa sổ, theo `PrincipalId`) | T thấy +1 | VIEW-05 |
| E-27 | A bấm đúp link W và đọc đủ 5s | Thân bài hiện một lần | +1 tổng | T thấy +1 | VIEW-03 |
| E-28 | Lần gửi đầu đã ghi nhưng mất phản hồi; tab gửi lại cùng mã sự kiện | Đọc bình thường | +1 tổng | T thấy +1 | VIEW-03, 10 |
| E-29 | A mở W ở hai tab gần như cùng lúc, cả hai đều qua 5s | Hai tab hiện thân bài | +1 tổng, kể cả khi hai yêu cầu tới server đồng thời | T thấy +1 | VIEW-03 |
| E-30 | Khách G mở W đủ 5s (+1), đăng nhập thành A rồi mở lại W đủ 5s trong cửa sổ | Thân bài | Lần hai +1: #341 không gộp mã khách với tài khoản | T thấy +2 | VIEW-04 |
| E-31 | Trình duyệt tắt JavaScript mở W và ở lại 10s | HTML server có thân bài, đọc bình thường | +0 | T không thấy lượt này; nhãn nói rõ giới hạn | VIEW-02, 12 |
| E-32 | A mở W đủ 5s, mất mạng khi gửi sự kiện; online lại trước khi hết retry trong cùng tab | Đọc bình thường | +1 nếu thử lại thành công trong giới hạn (cùng `event_id`); +0 nếu hết lượt thử hoặc đã đóng tab; không bao giờ +2 | T thấy +1 hoặc không đổi | VIEW-10 |
| E-33 | Ghi lượt lỗi phía server | Đọc bình thường, không thấy lỗi nào | +0 (lượt có thể mất) | T thấy số không đổi, hoặc “—” nếu đọc số cũng lỗi | VIEW-10, 12 |
| E-34 | Thân bài hiện đủ 5s khi W còn công khai, nhưng sự kiện tới server sau khi T đã chuyển W sang “Chỉ mình tôi” | Đọc bình thường | +0 | T thấy số cũ | VIEW-08 (kiểm trạng thái tại lúc ghi), VIEW-10 |
| E-35 | A có lượt cho W khi r1 công khai; 2 giờ sau r2 thay r1, A mở W đủ 5s | Thân bài r2 | +0 (cùng `WorkId`, còn trong cửa sổ) | Tổng của W không reset | VIEW-03, 06 |
| E-36 | A đang ở trang ẩn/rút của W; T công khai lại và cập nhật nền của `ReaderLoaded` chuyển sang thân bài | Thân bài tự hiện không có thao tác mở mới | +0 dù ở lại 5s (OQ-3A: phải mở mới) | Không đổi | OQ-3A, VIEW-01 |
| E-37 | Tab của A đã mở W từ hôm trước; trang nhận bản mới và A chọn “Xem bản mới” | Thân bài bản mới | +0: không có thao tác mở mới (nếu đổi revision khi bộ đếm trước 5s đang chạy, bộ đếm cũ bị hủy) | Không đổi | VIEW-01, 02, 06 |
| E-38 | A mở W, thân bài hiện 4.999 ms (hoặc rời sau 3s) rồi Back/đóng tab | Rời trang trước ngưỡng | +0 (client không phát request nào) | Không đổi | VIEW-01 (delta 25/11) |
| E-39 | A mở W, thân bài hiện ở foreground đúng 5.000 ms (đồng hồ monotonic) | Đủ ngưỡng đúng một lần | +1 (phát đúng một ứng viên tại ngưỡng; không heartbeat lặp) | T thấy +1 | VIEW-01 (delta 25/11) |
| E-40 | A mở W 3s foreground → chuyển sang tab khác 10s (`hidden`) → quay lại W thêm 2s foreground | Tạm dừng khi ẩn, cộng dồn phần thấy | +1 tại giây thứ 2 sau khi quay lại (`3 + 2 = 5s`, không tính 10s ẩn) | T thấy +1 | VIEW-01, OQ-3B |
| E-41 | A mở W mới 2s (chưa tới 5s) rồi đổi ý sang bài khác / chuyển revision / logout / bài bị ẩn | Đang đếm dở thì đổi đích hoặc authority | +0; bộ đếm bị hủy ngay, không chạy ngầm hay bù sau | Không đổi | VIEW-01 |
| E-42 | A mở W ở 2 tab độc lập, mỗi tab chỉ xem 3s rồi đóng | Hai lần mở rời rạc < 5s | +0 (không cộng dồn thời gian giữa nhiều lần mở hay nhiều tab) | Không đổi | VIEW-01, 03 |
| E-43 | Mạng/test override ngưỡng cấu hình dùng chung `WORK_VIEW_MIN_VISIBLE_MS` sang `1.200 ms` | Ở lại `1.199 ms` → +0; `1.200 ms` → +1 | +0 rồi +1 theo ngưỡng mới; giải thích UX cập nhật tương ứng | Phản ánh đúng nguồn cấu hình | VIEW-01, 12 |

## 7. Vòng đời theo trạng thái (AC-03)

Độc giả luôn đọc được bài khi thống kê lỗi, dù lỗi ghi hay lỗi đọc số.

| # | Trạng thái tác phẩm | Độc giả mở URL | Lượt có thể tăng? | Tác giả thấy ở Bàn viết (khi #514 lên UI; D-01/D-02) |
| --- | --- | --- | --- | --- |
| L-01 | Nháp chưa từng công khai, kể cả đang chờ duyệt lần đầu | Không tìm thấy (404, PUB-05 D) | Không | Ẩn metric hoặc “—” kèm lý do chưa công khai; không hiện “0”. Không có nút “Đọc”. |
| L-02 | Công khai sau mốc bật, chưa ai xem | Thân bài | Có (sau ≥ ngưỡng 5s) | “0” thật, chỉ khi đọc số thành công (`Thấm → Nhận xét → Lượt xem`). Không có nút “Đọc”. |
| L-03 | Công khai, có số | Thân bài | Có (sau ≥ ngưỡng 5s) | Số định dạng kiểu Việt, ví dụ “1.234”, icon mắt, đặt cuối `Thấm → Nhận xét → Lượt xem`. |
| L-04 | Số chưa cập nhật | Thân bài | Có | Số cũ hơn khoảng tối đa 1 phút; không tự cập nhật trực tiếp; lần tải sau thấy số mới. |
| L-05 | Thống kê lỗi | Thân bài, đọc bình thường | Lượt có thể mất | “—” kèm lý do không tải được. Tên, trạng thái bài và link mở bản thảo/editor vẫn dùng được. |
| L-06 | Sửa sang revision mới: r2 chờ duyệt khi r1 công khai, rồi r2 thay r1 | r1, rồi r2 | Có | Tổng cộng dồn, không reset khi đổi revision (khác Thấm). |
| L-07 | Chuyển “Chỉ mình tôi” (trang ẩn, PUB-07) | “Nội dung đã được ẩn” | Không | Số Lượt xem cũ, không tăng (`OQ-1 = A`, dù Thấm/Nhận xét ẩn theo `VDR-AUTHOR-WORK-FEEDBACK`). |
| L-08 | Rút sau duyệt (`O-17@r2`), kể cả khi tác giả chuyển riêng tư sau đó | “Bản này đã bị gỡ sau khi kiểm duyệt.” hoặc trang đóng | Không | Số Lượt xem cũ, không tăng (`OQ-1 = A`). |
| L-09 | Công khai lại, cùng revision hay revision mới | Thân bài | Có, đếm tiếp | Số cũ cộng lượt mới (nếu người xem mở mới và đủ 5s, không tự đếm qua cập nhật nền của tab đang mở trang ẩn theo `OQ-3A`). Người đã có lượt trong 24 giờ trước khi ẩn vẫn bị chặn tới hết cửa sổ. |
| L-10 | Tác giả xóa tác phẩm (target, PUB-04; OQ-2) | Trang thay thế “Nội dung đã được tác giả xoá” | Không | Số liệu bị xóa trong cùng PR triển khai route xóa; không còn số để hiện. |
| L-11 | Xóa tài khoản tác giả (target, SOC-LIFE-02; OQ-2) | Trang thay thế theo SOC-LIFE-02 | Không | Số liệu mọi tác phẩm của tài khoản bị xóa trong cùng PR triển khai route xóa. Tổng ở tác phẩm người khác không bị trừ vì không lưu theo người; mã chống trùng của tài khoản đó tự hết hạn trong tối đa 24 giờ. |
| L-12 | Bài đã công khai trước mốc bật (`counted_since != null`) | Thân bài | Có, từ mốc | Số kèm nhãn “Tính từ dd/mm/yyyy” (`Asia/Ho_Chi_Minh`). |
| L-13 | Bài công khai lần đầu sau mốc | Thân bài | Có | Không cần nhãn “Tính từ”. |
| L-14 | Admin gỡ cả bài vì vi phạm (PUB-05 C, chưa có runtime; OQ-4) | Không tìm thấy | Không | Tác giả vẫn giữ số Lượt xem cũ riêng tư khi còn quyền xem và số không tăng (`OQ-4`); xóa tác phẩm thì xóa dữ liệu (`OQ-2`). |

## 8. Trải nghiệm người dùng (nghĩa và quy ước UX-01..10)

Các ID `UX-01..UX-10` dưới đây là ID **cục bộ của Epic #323** (khác với `UX-01` nền tảng Reader
của PRD §4). Giao diện Bàn viết thuộc `#514` (PR2/2); thiết kế chi tiết (`LayoutSpec`, artboard)
được `#514` đồng bộ với delta 25/11 (`D-01`/`D-02`, không dùng PNG R1 cũ có nút “Đọc” hoặc thứ tự
khác làm chuẩn):

- **Thứ tự và bỏ nút Đọc (`D-01`, `D-02`, `UX-01..03`):** trong hàng tác phẩm Bàn viết, các
  metric được phép hiển thị giữ thứ tự cố định **Thấm → Nhận xét → Lượt xem** (Lượt xem đứng cuối),
  dùng icon mắt Material Symbols (`visibility`) kèm số cùng hàng và tone trung tính R4/Tailwind.
  **Bỏ hẳn nút/link “Đọc” / “Đọc bản công khai”** khỏi thẻ/hàng Bàn viết; không thay bằng nút,
  icon, menu, vùng giữ chỗ, tab stop hay click vào số liệu; giữ nguyên liên kết mở bản thảo/editor
  của hàng/nhan đề và giữ nguyên Reader công khai cùng các đường vào Reader khác.
- **Định dạng và phân biệt 0 / lỗi (`UX-04..06`, `UX-09`):** tên metric là “Lượt xem”. Số định
  dạng đầy đủ kiểu Việt (`1.234`, `12.345`). `counted = 0` hiện “0” thật; `not_started` /
  `never_public` / `unavailable` hiện “—” kèm lý do, không giả thành 0. Khi `counted_since != null`,
  hiện nhãn “Tính từ dd/mm/yyyy” theo `Asia/Ho_Chi_Minh`. Cập nhật khi tải lại/làm mới danh sách
  (không realtime, không polling tự động, không biểu đồ/dashboard, không thông báo).
- **Giải thích độ phủ (`UX-07`):** tooltip/popover mở được bằng chạm, bàn phím và screen reader
  giải thích rõ từ cùng nguồn cấu hình ngưỡng (`WORK_VIEW_MIN_VISIBLE_MS`): tối thiểu `X giây` ở
  chế độ hiển thị thực sự, tối đa 1 lượt / người / 24 giờ, khách tính theo trình duyệt, không tính
  lượt tác giả tự xem khi đăng nhập và không tính trình duyệt tắt JavaScript.
- **Không lộ công khai (`UX-08`, `UX-10`):** không số công khai ở Reader, Khám phá hay hồ sơ;
  không danh sách người xem; không đẩy sang Notifications, ranking hay CMP/native. Reader không thay
  đổi gì về giao diện hay trạng thái chờ: lỗi ghi lượt không hiện cho độc giả và không chặn mở, đọc,
  cuộn, bình luận hay quay lại.

## 9. Dữ liệu, quyền riêng tư và lưu giữ

- **Được lưu (P-11 B), và chỉ ba loại:** tổng theo tác phẩm; tổng theo ngày theo tác phẩm, không
  theo người và chưa có biểu đồ; mã chống trùng và biên nhận sự kiện đã băm, tự xóa khi hết cửa sổ
  24 giờ. Nếu kỹ thuật dùng mã sự kiện để chống gửi lại, mã đó cũng băm một chiều và hết hạn cùng
  cửa sổ.
- **Không lưu:** IP thô, fingerprint, danh sách người đã xem, lịch sử thời lượng đọc hay lịch sử
  đọc theo người. Tín hiệu dùng để lọc bot hay prefetch (CHOICE-06) không được lưu thành dữ liệu
  theo người.
- **Mã khách:** ngẫu nhiên (UUID v4), cookie first-party `__Host-vink_guest`, thời hạn 24 giờ cố
  định (không làm mới), chỉ dùng chống trùng Lượt xem.
- **Số liệu là dữ liệu riêng của tác giả:** không vào HTML SSR, cache công khai, IndexedDB/bộ nhớ
  đọc công khai hay JSON Reader công khai; đọc bằng phiên của tác giả với `no-store`.
- **Xóa:** xóa tác phẩm hay tài khoản thì xóa số liệu tương ứng (kích hoạt ở PR đầu tiên đưa route
  xóa vào runtime, OQ-2).
- **Mốc ngày của tổng theo ngày:** dùng `Asia/Ho_Chi_Minh` như quy ước hiển thị ngày của Web.
  Đây là ràng buộc thiết kế, không phải lời chốt riêng; lượt có biểu đồ có thể hỏi lại.
- **Log:** đường ghi Lượt xem không đưa IP thô hay mã khách chưa băm vào dữ liệu hoặc log của
  riêng nó. Log truy cập của hạ tầng hiện có nằm ngoài phạm vi và không được dùng để đếm hay
  backfill.
- **AI:** Lượt xem không gửi gì tới AI provider và không chạm nội dung nháp (MOD-05, INV-08).

## 10. Ràng buộc cho lựa chọn kỹ thuật (CHOICE-01..06, CHOICE-POLICY)

Các lựa chọn do issue tương ứng chọn bằng bằng chứng (`DESIGN_PROPOSED`, chưa `Accepted`).
#341 đề xuất `CHOICE-02 A`, `CHOICE-03` và `CHOICE-06 A` ([registry §7](../development/work-views.md#7-đường-ghi-341));
#342 đề xuất `CHOICE-05 A` ([registry §9.1](../development/work-views.md#91-choice-05-design_proposed));
#343 (PR1) đề xuất `CHOICE-01`, `CHOICE-04` và `CHOICE-POLICY` ([registry §10](../development/work-views.md#10-thu-nhận-http-và-tín-hiệu-reader-343)).
Lựa chọn nào làm đổi nghĩa số, quyền riêng tư, độ trễ hay khả năng mất lượt thì quay lại chủ
sản phẩm.

| CHOICE | Issue | Ràng buộc nghiệp vụ phải thỏa |
| --- | --- | --- |
| CHOICE-01 Điểm phát | #343 | Phát một `ViewEventId` (UUIDv7) duy nhất tại trình duyệt khi thân bài công khai đã hiện (`ReaderResponse::Published` → `PublicationView`) sau thao tác mở hợp lệ và tích lũy đủ ngưỡng foreground từ nguồn cấu hình chung (mặc định `5.000 ms`, đồng hồ monotonic, tạm dừng khi ẩn, hủy khi rời/unmount/đổi bài/đổi revision thực sự/mất công khai/đổi authority phiên, chờ foreground nếu mở nền theo OQ-3B, không tự đếm khi cập nhật nền từ `Hidden`/`ReviewWithdrawn` theo OQ-3A). Không cộng mọi GET, SSR HTML, hydration, prefetch/prerender, re-render hay refresh nền; không nhịp heartbeat định kỳ. |
| CHOICE-02 Lưu và cộng | #341 | Một hiệu ứng cho mỗi người đọc, tác phẩm và cửa sổ, kể cả khi yêu cầu đến đồng thời, mất phản hồi sau commit, rollback hay khởi động lại. Kiểm tại lúc ghi: tác phẩm còn công khai, người đọc không phải tác giả. Tổng và tổng theo ngày khớp nhau. Nếu tổng hợp trễ, hội tụ trong kỳ vọng khoảng 1 phút. |
| CHOICE-03 Định danh | #341, #343 | Thành viên: `PrincipalId` từ phiên (khi `IdentityResolver` trả `Unavailable`, fail-closed `503` thay vì hạ xuống khách để tránh đếm nhầm tự xem/trùng). Khách: mã ngẫu nhiên UUIDv4 first-party `__Host-vink_guest`, thời hạn 24 giờ cố định, không làm mới. Không IP thô, không fingerprint. |
| CHOICE-04 Gửi lỗi và retry | #343 | Retry hữu hạn trong tab với cùng mã sự kiện UUIDv7 cho lỗi mạng / `5xx`; dừng ngay ở `204` hoặc `4xx` hay khi hủy/rời/đổi bài/đổi phiên. Không hàng đợi bền qua đóng tab. Retry không bao giờ cộng trùng; retry tới sau khi bài hết công khai thì +0; thống kê lỗi không chặn đọc. |
| CHOICE-05 Đọc và cache | #342 | Phân biệt “0” với chưa có/lỗi. Lỗi số liệu không phá tên, trạng thái bài và link sửa. Không N+1. Số liệu tác giả không vào SSR hay cache công khai; việc tăng số không buộc tải lại toàn văn công khai; `ETag`/`304` vẫn đúng với quyền hiện tại. |
| CHOICE-06 Chống tăng số giả | #341 (luật), #343 (HTTP) | Kiểm input, chống gửi lặp, `INGEST_LIMIT` theo mạng trong bộ nhớ tiến trình, lọc `is_speculative` (`prefetch`/`prerender`) và `document.prerendering === false`. Không hứa loại mọi bot. Không thêm trường hay token phụ trên Reader công khai phá `ETag`. |
| CHOICE-POLICY Nguồn ngưỡng | #343 | Một nguồn cấu hình/constant typed dùng chung cho FE timer, giải thích UI (`UX-07`) và contract/test (mặc định `5.000 ms`), không trùng lặp magic number; chứng minh được bằng test override sang giá trị khác mặc định; không tạo service/DB riêng chỉ để cấu hình. |

## 11. Ranh giới phạm vi

**Trong Epic #323 (PR1 `#340+#343`, PR2 `#514`):** luật đếm VIEW-01..06, ngưỡng 5s cấu hình chung,
OQ-1..4, UX-01..10, D-01/D-02 trên Bàn viết, số chỉ tác giả thấy ở Bàn viết (VIEW-07..09, 12),
độ trễ và lưu giữ (VIEW-10, 11), HTTP intake và tín hiệu trên Reader Web, giao diện Bàn viết ở `#514`.

**Ngoài phạm vi:** số công khai ở Reader, Khám phá hay hồ sơ; danh sách người xem; dashboard và
biểu đồ; xếp hạng; Notifications; CMP/native; export; văn bản chính sách quyền riêng tư hay cookie
nói chung; route xóa tác phẩm/tài khoản hoặc admin gỡ cả bài (`OQ-2`/`OQ-4` gắn điều kiện vào PR
tương ứng). Các mục này cần quyết định/PR riêng trước khi làm.

## 12. Owner và bằng chứng

| Loại | Owner |
| --- | --- |
| Yêu cầu có ID | [PRD §17](../../prd/requirements.md#17-lượt-xem-tác-phẩm-work-views) |
| Luật, ví dụ, vòng đời | Tài liệu này |
| Bản ghi quyết định | [VDR-WORK-VIEWS@r1](../../decisions/proposals/VDR-WORK-VIEWS.r1.toml) (`PRODUCT_CONFIRMED`, active), [VDR-WORK-VIEWS@r2](../../decisions/proposals/VDR-WORK-VIEWS.r2.toml) (`PENDING`, delta 25/11) |
| Mức triển khai, bản đồ issue, kịch bản xuyên suốt | [Registry phát triển](../development/work-views.md) |
| Policy ngưỡng dùng chung | `packages/work-view-policy` (`vink-work-view-policy`) |
| Code triển khai | `services/vink-app` (domain, application, PostgreSQL, HTTP intake & author read), `apps/vink-web` (tín hiệu Reader; Bàn viết ở `#514`), `services/vink-app/contracts/http/v1` |

Bằng chứng (kiểm tài liệu, unit, PostgreSQL, browser) ghi ở registry.
