# LayoutSpec R4.3 · Bàn viết, Khám phá và điều hướng Web

**DESIGN_PROPOSED · `fidelity: NO_APPROVED_REFERENCE`**

Source đối chiếu `develop@2712145b08a6a4c7a12c1d57033ecd2a5bd33b26`. Đặc tả này tổng hợp phần R4.3
truy được từ repo và issue #414. [Bản audit R4](../redesign-r4-ban-viet-kham-pha.md) giữ lịch sử R4.1/R4.2
và lời chốt từng DQ. [Review và bàn giao](review-414.md) ghi nguồn, owner và phần cần kiểm tiếp.

## 1. Intent và References

- Người đọc tìm, lọc và mở đúng tác phẩm công khai. Người viết mở và phục hồi danh sách riêng, hoặc
  tạo bản thảo đầu tiên. Rail và thanh dưới phản ánh vị trí hiện tại mà không lấn át nội dung.
- [Epic #401 và delta R4.3](https://github.com/loveoverflowcom/vink/issues/401#issuecomment-7102228596)
  ghi hướng đã chọn. [Kết quả cũ #414](https://github.com/loveoverflowcom/vink/issues/414#issuecomment-7102229465)
  dẫn commit local `2c4f5fb` chưa truy được trong lượt này.
- [Drive R4](https://drive.google.com/drive/folders/1aJ-i-Y6I4zP2ia0JqmTicUf8juKaWRO8),
  [Canvas R4.3](https://claude.ai/artifact/KnQb9QuEfQKjxpyxzALCiN) và
  [Stitch ZIP đính kèm #414](https://github.com/loveoverflowcom/vink/issues/414#issuecomment-7099440249)
  đều `NOT_INSPECTED`. Canvas và Stitch là concept. Khi dùng để nghiệm thu thị giác, cần đúng file
  hoặc revision, quyền đọc và phân loại theo [sổ mốc](../visual-anchors.md). Folder Drive là đầu mối
  do issue chỉ tới, chưa xác minh nội dung revision.

## 2. Khung và Invariants

| Phần | Đặc tả |
| --- | --- |
| Nền, cột, thẻ | Nền trang và điều hướng dùng `bg-page` / `bg-nav`. Thẻ riêng dùng `bg-content`, bo đều `radius.card` 20 px, khe 8 px. Trục chính `size.page-column` 48rem, gutter hẹp 16 px. |
| Chữ và control | H1 `type.page-title` Be Vietnam Pro 600, 24 px (22 ở mobile). Nhan đề tác phẩm dùng Literata. Dùng `radius.control-56/48/40` (16/14/12 px) theo chiều cao. Giá trị thuộc [token SSOT](../../../packages/ui-system/shared/tokens.json). |
| R4-INV-01 · Nội dung | Giữ quyền riêng tư, chữ chưa lưu, Unicode/IME, cấu trúc dòng và focus. Phân biệt nháp với bản công khai, trạng thái duyệt với đối tượng đọc, số 0 với số chưa đọc được. |
| R4-INV-02 · Phục hồi | Bỏ Tải lại thường trực, giữ thao tác theo trạng thái: thử lại, cursor không còn hợp lệ, đối soát lần tạo chưa xác nhận, về phần đầu. Dữ liệu tải thành công còn hữu ích trong cùng phiên khi refresh tạm lỗi. |
| R4-INV-03 · Điều hướng | Duy trì URL/link thật, lịch sử Back/Forward, tên truy cập và tín hiệu selected. Kiểm khu vực ẩn khỏi Tab và cây accessibility. |
| R4-INV-04 · Tương tác | Ổn định node nhập trong cùng ngữ cảnh, tránh submit/đóng do phím của IME. Tôn trọng reduced motion và kiểm cả forced colors. |

## 3. Shell, rail và thanh dưới

Thứ tự Khám phá → Bàn viết, mục Tài khoản ở cuối. Dưới 760 px dùng thanh dưới và safe area. Từ
760–1099 px giữ rail 96 px, mở điều hướng đầy đủ bằng hộp thoại. Từ 1100 px theo chế độ rail và
preference (mở rộng 220 px, thu gọn 96 px). Điều hướng không đổi route mặc định `/`.

[DQ-11](../redesign-r4-ban-viet-kham-pha.md#8-sửa-theo-thiết-kế-hiện-hành-vs-cần-quyết-định)
giữ hai lựa chọn: bác thẻ tài khoản ở chân rail và dùng nền trang cho mục Tài khoản khi mục này có chữ.
[#413](https://github.com/loveoverflowcom/vink/issues/413) thay riêng phần “giữ Giao diện”. Web #413
bỏ nút ở rail mở rộng, thu gọn và hộp thoại. Trên Web, khách và người đã đăng nhập chỉnh
Giao diện tại Tài khoản (`/toi`). Mặc định 100%, Lớn 150%; bootstrap chuyển preference cũ 200 → Lớn 150%
trước CSS và hydration. Stress 200% có phương pháp và metadata riêng. Source đã pin ở đầu tài
liệu ghi lại baseline trước #413.

[#427 AC-04](https://github.com/loveoverflowcom/vink/issues/427#acceptance) giữ tên đích và
`aria-current="page"`, dùng icon outlined cho đích chưa chọn và filled cho đích đang chọn.
Áp dụng cho ba đích Khám phá (`explore`), Bàn viết (`edit`) và Tài khoản (`person` khi chưa có avatar). Phối hợp #413 để
thêm biến thể outlined một lần vào [catalog icon](../../../packages/ui-system/shared/icons.json),
ghi nguồn Material Icons Outlined cùng commit upstream và kiểm generated output. Việc đổi
Giao diện qua Tài khoản trong khi đang viết đi qua guard rời editor. Thiết kế lối tắt và canvas editor thuộc [#428](https://github.com/loveoverflowcom/vink/issues/428).

[#520](https://github.com/loveoverflowcom/vink/issues/520) cập nhật mục Tài khoản khi đã đăng nhập.
Rail mở rộng và hộp thoại tablet hiện avatar tròn 32 px cạnh nhãn “Tài khoản”. Rail thu gọn
chỉ hiện avatar, giữ tên truy cập “Tài khoản”, trạng thái chọn và focus. Thanh dưới hiện avatar
32 px trong pill, nhãn bên dưới. Khách giữ nhãn và biểu tượng theo khả năng đăng nhập.

### Tài khoản · #521

[#521](https://github.com/loveoverflowcom/vink/issues/521) ghi hướng bố cục chủ sản phẩm đã chọn.
Trên `/toi` khi phiên hoạt động, avatar chữ 64 px và bút danh làm H1 trong khối đầu. Chuông mobile
nằm ở mép phải theo quyền phiên. Bên dưới là “Xem hồ sơ” và “Chỉnh hồ sơ”, hai link có icon và
xuống dòng khi thiếu chỗ. Khách giữ tiêu đề form hiện có và H1 trong cây trợ năng.

- CHOICE-02 dùng một lần đọc hồ sơ chính chủ theo phạm vi phiên và đọc lại khi nhận tín hiệu đổi hồ sơ.
  “Xem hồ sơ” chỉ hiện khi địa chỉ tác giả đã thiết lập và hợp lệ. Khi thiếu địa chỉ, dẫn bằng lời
  tới “Chỉnh hồ sơ”. Khi lỗi, hiện trạng thái và cho thử lại.
- CHOICE-01 gom “Quyền riêng tư”, “Tên đăng nhập và mật khẩu”, email xác minh, “Giao diện” và
  “Đăng xuất” dưới H2 “Cài đặt” theo quyền hiện có. Quyền riêng tư trỏ thẳng tới vùng policy
  trong Chỉnh hồ sơ. Khối mật khẩu mở tại chỗ và giữ form đã mở trong cùng phiên. Email hiện
  trạng thái hoặc tác vụ xác minh có thật. Giao diện dùng vùng điều khiển hiện có và mục nhảy
  từ form khách. Đăng xuất dùng luồng đọc lại phiên hiện có.
- Khối nền tonal bo theo token R4 và dùng font UI. Hàng tương tác có vùng bấm tối thiểu 44 CSS px.
  Ưu tiên một cột ở màn hẹp, giữ thứ tự bàn phím và nhãn tiếng Việt đầy đủ. Kiểm Chrome ở hai
  theme, 320/390 px và desktop, 100%/150% cùng stress 200% riêng.

Tham chiếu là hướng bố cục đã chọn trong #521. LayoutSpec R4.3 giữ `DESIGN_PROPOSED` và
[`VDR-SOCIAL-GRAPH@r2`](../../../decisions/proposals/VDR-SOCIAL-GRAPH.r2.toml) vẫn `PENDING`.

## 4. Khám phá

- Đầu trang gồm H1 Khám phá, nút tìm, nút Lọc thẻ có số đã áp dụng, logo ở phải. Chế độ Tất cả /
  Đang theo dõi / Tác giả dùng link, giữ feature flag của Đang theo dõi. Nhóm nối liền có selected
  primary + dấu chọn. Khi không vừa một hàng, dùng menu vẫn giữ đích link và `aria-current`.
- Một hàng/thẻ dùng thể loại và ngày công khai đầu (Asia/Ho_Chi_Minh khi xác định được), nhan đề,
  trích đoạn và byline với avatar tròn. Dùng tertiary cho dòng thông tin và tertiary-container cho
  nút Lọc thẻ. Hàng phải đọc được ở màn hẹp kể cả khi nhan đề dài.
- Sheet dùng chip theo bề rộng chữ, bo 12 px, trạng thái chọn có dấu. “Bỏ chọn” chỉ xóa draft của
  sheet. “Áp dụng” submit GET, tối đa 4 thẻ, kết hợp AND. Link xóa các thẻ đã áp dụng giữ q, bỏ
  cursor cũ. Giữ fallback GET trước JS.
- **R4-INV-06.** Ở chế độ Tất cả, SSR `/?q=…` mở dock và hiện q. Đóng bằng nút hoặc Esc bỏ q
  và cursor, giữ bộ lọc thẻ đã áp dụng và trả focus theo ngữ cảnh. Trang chi tiết `/the/:slug`
  dùng trường “Tìm trong thẻ” riêng, ngoài hợp đồng mở/đóng dock này.
- **#427 AC-01/02.** Ở 320/390 px, Mặc định 100% và Lớn 150%, nhóm chế độ vừa một hàng nối
  liền hoặc gập thành menu. Khi dock đóng, logo, nút Tìm và nút Lọc thẻ cùng một hàng, trang
  không cuộn ngang. Ở mỗi bề rộng chỉ một bộ link chế độ nhận Tab và xuất hiện trong cây trợ
  năng. Kiểm stress 200% riêng.

## 5. Bàn viết

### 5.1. Header, tìm và phục hồi

H1 Bàn viết, kính lúp mở dock và ô “Tìm theo tên”. Tách chữ đang nhập khỏi query đã gửi, giữ
node qua refresh cùng phiên. Query riêng không nằm trên URL. Sắp xếp thuộc danh sách. Bỏ Tải lại
thường trực, giữ `#retry-drafts`, `#reconcile-drafts`, `#restart-drafts` khi thích hợp và
`#first-draft-page` từ phần sau. Ở nhánh phục hồi, bảo đảm focus tới đích còn điều khiển được và
có tên.

[Delta R4.3 của #408](https://github.com/loveoverflowcom/vink/issues/408#issuecomment-7102205637) nêu
bỏ q khi đóng, sắp xếp còn thấy khi tìm và focus đích heading nếu dock đóng. #426 hiện giữ q khi
đóng, ẩn sắp xếp khi dock mở và dùng launcher cho nhiều đường phục hồi. Ba điểm này cần đối soát
ở owner Desk trước khi sửa runtime. Xem [HOW Bàn viết](how-ban-viet.md) và [disposition](review-414.md).

### 5.2. Khách, lần đầu, đang tải và lỗi

Khách và các biến thể `DeskAccess` dùng EmptyState trên nền trang, CTA theo khả năng đăng nhập.
Phiên chưa đủ quyền không đọc danh sách riêng. Lần đầu có minh họa và một CTA “Viết bản thảo đầu
tiên”, kèm Select “Định dạng bản thảo” theo DQ-06. DQ-04 ẩn kính lúp/sắp xếp ở kho rỗng vẫn
`OPEN`; đề xuất ẩn khi ô không có chữ phải giữ node và tái kiểm khi có query hoặc lỗi.
Màn chờ hiện skeleton UIS-03 sau 300 ms. Lỗi ở vùng kết quả, có Thử lại khi thích hợp, giữ hàng
đã đọc và query nếu vẫn cùng phiên. Lỗi quyền có trạng thái riêng và gỡ dữ kiện riêng tư.

### 5.3. Thẻ tác phẩm

Thẻ rời (`bg-content`, bo 20 px, khe 8 px), nhan đề Literata, nhan đề trống dùng chữ nghiêng. Status
hiển thị số tác phẩm trong kết quả cùng nút chữ “Cách tính lượt xem” mở sheet giải thích (`#514`).
Mỗi sự thật có một nhãn ngắn kèm icon (tối đa ba; r3 bổ sung tone tonal chọn lọc cho “Đã đăng”,
nhóm chờ duyệt bằng `secondary-container` và nhóm không được duyệt/ẩn/gỡ bằng `error-container`),
giữ nghĩa của “Chỉ mình tôi”, “Chưa có bản công khai”, bản mới và tên bản công khai nếu khác. Không
tách chuỗi đã render để lấy state. Cụm số liệu giữ thứ tự cố định **Thấm → Nhận xét → Lượt xem**
(delta 25/11 `D-01`, #323/#514): dùng `0` chỉ cho số 0 đo được, dấu “—” kèm lý do/tên chỉ số khi
chưa bắt đầu hoặc chưa tải được; bài riêng tư/ẩn giữ số Lượt xem lịch sử (`OQ-1 = A`) trong khi
Thấm/Nhận xét chỉ hiển thị khi có bản công khai. Theo delta `D-02` (#323/#514), **bỏ hẳn** liên kết
“Đọc” / “Đọc bản công khai” khỏi thẻ tác phẩm Bàn viết (không có nút, vùng giữ chỗ hay tab stop
thay thế); giữ liên kết mở bản thảo của hàng. DQ-05 (giờ) và DQ-10 (dùng tertiary cho “Chờ duyệt”)
vẫn `OPEN`: giữ giờ theo #213 và dùng `secondary-container` (không dùng tertiary) cho nhóm chờ duyệt.
Xem [LayoutSpec Lượt xem r3](../work-views.md) và [delta R4.4](desk-moderation.md) cho chi tiết
kiểm duyệt mở từ thẻ.

## 6. Interaction và kiểm lại theo owner

| Luồng | Lỗi/phục hồi cần kiểm | Owner |
| --- | --- | --- |
| Bàn viết | Đổi phiên/401/403, lỗi trước và sau khi tải, lần tạo mất phản hồi, cursor hết hiệu lực, clear/retry làm gì với focus và query | #408/#411, cùng [registry tác giả](../../development/desk-author-feedback.md) |
| Khám phá | SSR `/?q=…` ở Tất cả rồi đóng bằng nút/Esc, Back khỏi Reader, chỉ một bộ link có trong Tab/AX, logo + hai nút một hàng ở 320/390 px tại 100%/150%, GET filter và cursor lỗi | #404/#406 và phần còn lại #427 |
| Shell/cỡ chữ | 320/390/834/1440, cả hai theme, 100/150 theo #413 và stress 200 riêng. Kiểm keyboard, focus và AX đúng bề mặt | #403/#413/#427 |

Ghi kết quả Functional, Geometry, Visual, Accessibility–Responsive và Interaction theo từng
scenario, viewport, theme, app scale, browser zoom và SHA. Source và ảnh lịch sử chưa chứng minh
các mục này tại SHA mới. Chi tiết lý do và câu hỏi còn mở ở [WHY R4](why-r4.md).
