# Kiểm chứng Web cho tìm và gợi ý tác giả #354

Khám phá → Tác giả (`/?mode=authors`). Baseline `c8db4862e6e00aa6b062e8b311774a21fa9dace1` gồm #370 và #371. [Issue #354](https://github.com/loveoverflowcom/vink/issues/354) · [LayoutSpec Social r2](../../../design/social-289.md#author-search) · [Lavender R3](../../../design/lavender-r3-336/README.md)

## Cách đo

- Dùng Chrome headless, DPR 1, locale `vi-VN`, root font-size 100/200%, đặt `prefers-reduced-motion: reduce`. Các bản dựng dùng chung cấu hình `verify web`. Chờ hydrate, `document.fonts.ready`, animation hữu hạn và hai `requestAnimationFrame` trước lượt đo. Xem theme, font đã tải và bounds thực trong số đo.
- Ma trận cơ sở gồm 320/390 × sáng/tối × chữ 100/200%, mỗi ô có phiên khách và phiên Identity thực. Kiểm rail ở 1440 CSS px. Viewport cơ sở cao 900 CSS px. Hạ viewport xuống 460 CSS px trong ca composition để đo khoảng trống từ heading đến thanh điều hướng dưới. Đây là mô phỏng viewport và composition qua CDP.
- PostgreSQL scratch của `author-search` chứa hai hồ sơ cùng bút danh “Đặng Hoàng Bảo Trâm Ánh Dương Thương Lê Nguyễn Hoàng Vũ”, Handle ASCII liền dài `zzztacgiathunghiemkhongcodaugachngang01` và `zzztacgiathunghiemkhongcodaugachngang02`, cùng 16 hàng “Đặng Minh Hòa” từ smoke. Phiên Identity có Handle scratch cố định `doc-gia-evidence-354`. Truy vấn `Đặng Hoàng Bảo Trâm` dành cho hai tên dài. Truy vấn exact `@…01` dành cho phần tô Handle. Dùng `không có bút danh này` cho kết quả rỗng và giữ request rồi trả 503 cho cảnh chờ/lỗi đầu. Ca lỗi tải thêm trả 503 cho `cursor` và giải phóng chặn để đi đến cuối danh sách. Ca Back ghi hàng rào thu hồi trong scratch trước khi quay từ hồ sơ.
- Đọc DOM rect và `Range.getClientRects()` theo CSS px. Đối chiếu `scrollWidth` với `clientWidth` ở root, lấy hit test tại tâm đích sau cuộn. Đo màu computed qua Canvas sRGB, ghép alpha trên nền các ancestor rồi tính tỷ lệ sáng tương đối. Đọc AX tree qua CDP để kiểm tên, role, trạng thái live và hai đích riêng. Vị trí focus lấy sau phím Tab, Enter hoặc Space, trừ lúc đặt điểm xuất phát cho từng chuỗi.

## Mốc thiết kế

Đọc cả bốn PNG Drive của ngày 20/11/2026. `DESIGN_PROPOSED / NO_APPROVED_REFERENCE`. So sánh theo cấu trúc và trạng thái của LayoutSpec, với ảnh Chrome của baseline làm mốc hồi quy cùng môi trường. [295-01 sáng](https://drive.google.com/file/d/1MpKMZiTBoKdm3u7BK-3EJ38SvdUDuXA9/view) · [295-02 tối](https://drive.google.com/file/d/1M99xpJCzdhcTtpiA5sEflB-vpk2CTVzK/view) · [295-03 gợi ý](https://drive.google.com/file/d/1FQ2Qu_ZRfZXoLxRmegiKGu4Wr1RpOJok/view) · [295-04 trạng thái](https://drive.google.com/file/d/1DF4GUL72WWIG0W7zzS4--5vPk1MYXyDL/view)

- 295-01/02 đặt truy vấn trong tiêu đề và phân biệt bút danh trùng bằng Handle. 295-03 đặt “Bạn” cùng vùng hành động theo phiên. Dòng chú thích trên Web diễn đạt rằng danh sách dùng chung. Hợp đồng sắp theo Handle ASCII.
- 295-04 giữ phản hồi tại vùng kết quả trong khi chờ, lỗi lần đầu, lỗi tải thêm và khi Back mất hàng. Cảnh exact Handle tô `@handle`. Đích tìm tác phẩm và xóa tìm kiếm tiếp tục dùng điều hướng Khám phá hiện có.

## Vòng sửa tràn ngang

Chrome trên `8870f3f1fc630343bee7ce6389ee20ec313b2a87` tái hiện tràn khi tiêu đề chứa
Handle liền 39 ký tự. Ở 390 px/sáng/100%, `scrollWidth` là 544 px trên `clientWidth`
375 px. Ở 320 px/tối/200%, số đo tương ứng là 1.065 và 305 px. Cột lưới kết quả
nở theo min-content của tiêu đề và đẩy cả nút Theo dõi ra khỏi khung. [390 px](first-candidate/exact-handle-390-900-light-100.png) · [320 px sau cuộn](first-candidate/exact-handle-detail-320-900-dark-200.png) · [Số đo vòng đầu](first-candidate/overflow.json)

Vòng sửa giới hạn bề rộng cột trong vùng Tác giả và áp dụng `overflow-wrap: anywhere`
trực tiếp lên tiêu đề kết quả.

## Bản dựng và phép kiểm

Chrome 158.0.8127.36 headless trên macOS 15.6 arm64, Node 22.15.0 và PostgreSQL 17.11.
Font Be Vietnam Pro và Literata tải từ ứng dụng. Lượt trước dựng `956ec3ad92b2285b4d77f2c6f7d41fb21e483fa6`
với mã sản phẩm ở `c8db4862` và chỉ thay browser smoke. Lượt sau dựng
`f547bccf2bddafe77e1a85733a294a950990960b`. Hai lượt dùng cùng hook CDP tạm ở đầu
scenario. [Chỉ mục lượt chụp, ảnh và SHA-256](capture-index.json) · [Số đo và AX trước](before/measurements.json) · [Số đo và AX sau](after/measurements.json)

`cargo xtask verify web --scenario author-search` trên baseline trả `FAIL` đúng dự kiến
ở 11 assertion mới thuộc S1–S7. Các phép kiểm cũ về IME mô phỏng, response cũ, phân
trang, hai phiên, Back, 410 và console chạy qua. Bốn unit test SSR về heading/chờ,
rỗng, lỗi và exact Handle đạt trên `8870f3f1`.

`cargo xtask verify web --scenario author-search --scenario discovery` trên `f547bccf`
trả `PASS` với App và Identity thật trên PostgreSQL scratch. Smoke kiểm S1–S7, hai
ca exact Handle liền 38 ký tự và console. Mỗi lượt CDP có 65 phép đo và 59 PNG
viewport đã mở để rà hình. Các lượt cuộn chi tiết giữ cùng viewport, DPR và theme.

## Kết quả AC-04

| Chiều | Kết quả |
| --- | --- |
| Functional | `PASS`. Gợi ý có chú thích và hàng của phiên hiện hành hiện “Bạn”. Kết quả rỗng giữ từ khóa khi sang tìm tác phẩm. Xóa trở về gợi ý và đưa focus về ô tìm. Lỗi đầu có đường thử lại. |
| Geometry | `PASS`. Tràn ngang lớn nhất 0 CSS px trên 65 cảnh sau sửa. Cả 8 cấu hình hẹp chứa trọn exact Handle dài 39 ký tự. CTA xuống dưới hàng tác giả ở cả 4 ô 200% trên mobile. Rail có mặt ở 1440 px. |
| Visual | `PASS` theo LayoutSpec. Hai bút danh trùng phân biệt qua Handle, dấu tiếng Việt đọc được và vùng tô xuống dòng theo chữ. Tìm trong tác phẩm nổi bật hơn Xóa tìm kiếm. Ở 320 px/200%, cần cuộn mới thấy hết hàng và hành động. |
| Accessibility/Responsive | `PASS` cho đích chạm, tương phản resolve, vòng focus và AX tree đã kiểm. Tên link đọc kèm Handle. “Theo dõi” là button riêng có mô tả gắn với tác giả. `status` của kết quả rỗng, chờ và Back đều có `live=polite`. `alert` của lỗi đầu có `live=assertive`. |
| Interaction | `PASS` với Tab, Enter và Space trong Chrome. Tab đi qua nút Tìm kiếm, ô tìm rồi đến link tác giả đầu tiên khi ô rỗng. Từ heading, Tab qua link tác giả → Theo dõi → link tác giả tiếp. Space thực hiện Theo dõi và Enter mở hồ sơ. Retry append giữ đủ 12 hàng và giữ focus. Sau khi phục hồi, link và nút của hàng 16 nằm trên thanh dưới và nhận hit test. Back khi hàng mất đặt focus ở heading và cho thấy thông báo. Lặp lại truy vấn xóa thông báo. |

`NOT_RUN` với IME hệ điều hành, bàn phím ảo trên thiết bị thật, trình đọc màn hình và
BFCache hit thực. AX tree xác nhận role và thuộc tính live. Lượt đo viewport thấp dùng
composition qua CDP và phím Enter có `isComposing`.

### Tương phản thấp nhất trong ma trận

| Theme | Chữ trước | Vòng focus trước | Chữ sau | Vòng focus sau |
| --- | --- | --- | --- | --- |
| Sáng | 5,40:1 | 4,64:1 | 5,40:1 | 4,64:1 |
| Tối | 7,68:1 | 7,57:1 | 7,68:1 | 7,57:1 |

### Đích tương tác tại 390 px, sáng, 100%

Mọi đích chính đo được trên bản sửa đạt tối thiểu 44 × 44 CSS px. Nút tìm và xóa trong
ô tìm đạt 48 × 48 CSS px. Kích thước dưới đây là rộng × cao theo CSS px.

| Đích | Trước | Sau |
| --- | --- | --- |
| Link ở hàng tên dài | 215,62 × 131,75 | 215,62 × 131,75 |
| Nút Theo dõi | 110,38 × 48,00 | 110,38 × 48,00 |
| Thử lại lần đầu | 51,62 × 20,00 | 51,62 × 44,00 |
| Thử tải thêm | 97,95 × 25,59 | 97,95 × 44,00 |
| Tìm trong tác phẩm | — | 187,05 × 44,00 |
| Xóa tìm kiếm trong trạng thái rỗng | — | 101,56 × 44,00 |

### Chiều cao vùng kết quả ở viewport thấp

Viewport cao 460 CSS px. Số đo lấy giao của vùng kết quả và phần viewport phía trên
thanh điều hướng dưới ở vị trí Chrome tự cuộn để giữ focus. Cả tám ca giữ truy vấn đã
gửi và giá trị đang composition. Ở 200%, cần cuộn để thấy các hàng sau heading.

| Rộng (CSS px) | Theme | Cỡ chữ | Trước (CSS px) | Sau (CSS px) |
| --- | --- | --- | --- | --- |
| 320 | Sáng | 100% | 112,61 | 171,61 |
| 320 | Sáng | 200% | 91,86 | 91,86 |
| 320 | Tối | 100% | 112,61 | 171,61 |
| 320 | Tối | 200% | 91,86 | 91,86 |
| 390 | Sáng | 100% | 158,61 | 158,61 |
| 390 | Sáng | 200% | 199,05 | 92,05 |
| 390 | Tối | 100% | 158,61 | 158,61 |
| 390 | Tối | 200% | 199,05 | 92,05 |

## Ảnh đối chiếu

Toàn bộ ma trận có trong [chỉ mục ảnh](capture-index.json). Bảng ghim các cặp cần đọc để
đối chiếu một trạng thái hoặc hành động. Tên có `detail` ghi riêng vị trí sau khi cuộn.

| Cảnh | Trước | Sau |
| --- | --- | --- |
| Khách, tên dài tô phần khớp · 390 sáng 100% | [PNG](before/guest-390-900-light-100.png) | [PNG](after/guest-390-900-light-100.png) |
| Theo dõi và vòng focus · 320 tối 200% | [PNG](before/member-follow-focus-320-900-dark-200.png) | [PNG](after/member-follow-focus-320-900-dark-200.png) |
| Handle 39 ký tự sau cuộn · 320 tối 200% | [PNG](before/exact-handle-detail-320-900-dark-200.png) | [PNG](after/exact-handle-detail-320-900-dark-200.png) |
| Gợi ý với phiên thành viên · 390 sáng 100% | [PNG](before/suggested-390-900-light-100.png) | [PNG](after/suggested-390-900-light-100.png) |
| Rỗng sau khi tìm thành công · 390 sáng 100% | [PNG](before/empty-390-900-light-100.png) | [PNG](after/empty-390-900-light-100.png) |
| Chờ truy vấn · 390 sáng 100% | [PNG](before/loading-390-900-light-100.png) | [PNG](after/loading-390-900-light-100.png) |
| Lỗi lần đầu · 390 sáng 100% | [PNG](before/initial-error-390-900-light-100.png) | [PNG](after/initial-error-390-900-light-100.png) |
| Lỗi tải thêm · 390 sáng 100% | [PNG](before/append-error-390-900-light-100.png) | [PNG](after/append-error-390-900-light-100.png) |
| Back mất hồ sơ và focus heading · 320 tối 200% | [PNG](before/back-lost-profile-320-900-dark-200.png) | [PNG](after/back-lost-profile-320-900-dark-200.png) |
| Focus hàng cuối sau cuộn · 320 tối 200% | [PNG](before/last-result-320-900-dark-200.png) | [PNG](after/last-result-320-900-dark-200.png) |
| Composition trong viewport 460 CSS px · 390 tối 200% | [PNG](before/composition-short-390-460-dark-200.png) | [PNG](after/composition-short-390-460-dark-200.png) |
| Rail desktop · 1440 tối 200% | [PNG](before/member-1440-900-dark-200.png) | [PNG](after/member-1440-900-dark-200.png) |
