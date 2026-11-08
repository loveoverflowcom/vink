# Bộ chọn thẻ và Thông tin tác phẩm — TAG-PR-05

Triển khai có phạm vi cho [#99](https://github.com/loveoverflowcom/vink/issues/99) (một phần của epic
[#94](https://github.com/loveoverflowcom/vink/issues/94)), base `develop@f23df33` (sau #95–#97). Đây là
lựa chọn của agent trong phạm vi được giao, chưa có usability test hay quyết định user-approved. VinkDesign
(Drive) không được mở trong phiên này: không có ảnh thẻ đã duyệt nào được dùng làm tham chiếu
(NOT_INSPECTED). Hợp đồng là [HTTP v1 Work tags](../../services/vink-app/contracts/http/v1/README.md#work-tags-tag-pr-03);
lựa chọn API ở [taxonomy-api](../development/taxonomy-api.md). Luồng và bằng chứng triển khai ở
[taxonomy-editor](../development/taxonomy-editor.md).

## Brief

- **Người dùng / tác vụ:** tác giả đang ở Bàn viết `/ban-viet/{id}` muốn gắn một dạng bài (tối đa 1) và
  tối đa 3 chủ đề từ danh mục curated cho **bản nháp** của mình, rồi lưu, mà không mất chữ và không hiểu
  “đã lưu thẻ” thành “đã công khai”.
- **Nội dung / hành động chính:** bản thảo vẫn là chính; thẻ là thông tin phụ. Hành động chính vẫn là
  **Lưu** trên toolbar (lưu cả chữ lẫn thẻ khi có thay đổi), không thêm CTA thứ hai cạnh tranh.
- **Giới hạn có thật từ backend:** một `form` hoặc `null`; ≤3 `themes` khác nhau; chỉ thẻ `active` được
  thêm; thẻ `retired` đã có trên nháp được giữ; không route tạo/đề xuất thẻ. Nhóm/cardinality hiển thị vì
  #95/#97 thật sự có hai facet đó.
- **Trạng thái khó dùng để so phương án:** 390 px, chữ 200 %, đã chọn *Tản văn* + *Ký ức* + *Đô thị*
  (đã ngừng dùng, gợi ý *Mùa*), đang gõ `hồi` có 2 gợi ý, thẻ chưa lưu.

## CHOICE-01 — bố cục sửa metadata

| Hướng | Cây ngắn | Màn hẹp / chữ 200 % ở trạng thái khó | Đánh đổi |
| --- | --- | --- | --- |
| **A. Mục “Thông tin tác phẩm” inline sau trang viết** (chọn) | `#editor → #toolbar → #recovery → #paper(title, body) → #work-info(heading, privacy, form chips, theme chips, query, suggestions, tag notices)` | Một cột; chip tự xuống dòng; gợi ý là danh sách trong luồng, không lớp phủ; bàn phím ảo không che vì trang cuộn bình thường | Chiếm chiều cao cuối trang; bản thảo dài phải cuộn xuống. Không cần focus trap, Escape/Back hay safe-area riêng |
| B. Panel bên (desktop) + bottom sheet (mobile) mở từ tóm tắt thẻ trên toolbar | `#toolbar(… #tags-summary) → Overlay #tag-panel(heading, chips, query, suggestions, Đóng)` | Sheet cao ~80 %, ô tìm ở trên cùng; bàn phím ảo chiếm nửa sheet ở chữ 200 % → danh sách gợi ý còn 1–2 dòng | Trang viết gọn hơn nhưng cần owner overlay, trả focus, Escape/Back, IME khi đóng sheet giữa composition, safe-area. Catalog nhỏ (≤200 thẻ, 1+3 lựa chọn) không cần thêm không gian |
| C. Disclosure gọn giữa tiêu đề và nội dung | `#paper(title, #tags-disclosure[summary → chips, query, suggestions], body)` | Mở ra đẩy nội dung xuống; đóng lại chỉ còn một dòng tóm tắt | Thấy ngay gần tiêu đề nhưng chen vào luồng tiêu đề → nội dung (đường viết chính, focus mặc định) và thêm trạng thái mở/đóng |

**Chọn A.** Lý do: tác vụ nhỏ, một ngữ cảnh, không lớp phủ nên không đụng IME/focus của CodeMirror và
không tạo owner overlay thứ hai; reflow ở 320 px/200 % chỉ là xuống dòng. Xem lại B khi danh mục cần nhóm,
mô tả dài hoặc nhiều facet hơn; khi đó dùng `OverlayLayer` hiện có, không tạo overlay manager mới.

## CHOICE-02 — ranh giới composition dùng chung

| Hướng | Đánh giá |
| --- | --- |
| **A. Ghép primitive hiện có trong `vink-patterns`** (chọn) | `FilterChip` (dạng bài, chọn một), `SearchField` (ô tìm: Enter khi đang gõ dấu đã bị chặn, Enter không gửi form bản nháp), `Button` (gợi ý có nhãn + mô tả), `IconButton` (bỏ từng thẻ, tên riêng), `SupportingDetails` (xem chủ đề có sẵn). Composition `TagSelector` + `TagChip` nhận facts/callback, không DTO, không fetch |
| B. Thêm primitive chọn có kiểm soát vào `ui-system` | Combobox `aria-activedescendant` cần API keydown/ARIA mới ở `text_control`, cặp CMP, vectors và qualification. Mẫu danh sách nút + phím mũi tên trong composition diễn đạt đủ ngữ nghĩa mà không đổi primitive |

Không thêm token: role màu/khoảng cách/bo góc hiện có đủ. `TagChip` dùng lại hình dạng chip; trạng thái
ngừng dùng/lỗi có chữ + icon, không chỉ màu.

## LayoutSpec — hướng A (selected, agent, revision 1)

```text
LayoutSpec: A inline Thông tin tác phẩm, rev 1; selected bởi agent thực hiện #99
Context: tác giả, Bàn viết /ban-viet/{id}, bản nháp riêng; dữ liệu thật qua API, mẫu chỉ trong gallery
Sources: develop@f23df33; apps/vink-web/src/pages/desk.rs, desk_state.rs; vink-patterns; vink-ui field/action;
         tokens.json 0.5.x; HTTP v1 Work tags. Gap: VinkDesign NOT_INSPECTED

Screen #editor-page — viết và lưu một bản nháp
  Column #editor (form hiện có)
    Region #toolbar — EditorToolbar hiện có: trạng thái gộp (chữ + thẻ) · Xem trước · Tải .txt · Lưu
    Region #recovery — phục hồi chữ hiện có (save-failed / conflict của văn bản), không đổi
    Region #paper — tiêu đề + CodeMirror, không đổi, không remount khi thẻ đổi
    Region #work-info — <section aria-labelledby> “Thông tin tác phẩm” (component app)
      Row #work-info-heading — H2 Title + trạng thái thẻ (Đã lưu thẻ / Chưa lưu thẻ / Đang lưu thẻ… /
                               Chưa lưu được thẻ / Thẻ đã đổi ở nơi khác)
      Text #work-info-privacy — “Chỉ bạn thấy. Lưu thẻ không công khai tác phẩm.”
      Region #tag-selector — TagSelector (vink-patterns)
        Group #tag-form — legend “Dạng bài” + “chọn 1”; FilterChip mỗi dạng (aria-pressed);
                          dạng đang chọn đã ngừng dùng → TagChip riêng phía trước nhóm
        Group #tag-themes — legend “Chủ đề” + “n/3”
          List #tag-themes-selected — TagChip: nhãn · [Đã ngừng dùng] · [Thay bằng …] · IconButton
                                      “Bỏ chủ đề {nhãn}”; lỗi từ máy chủ dưới chip (icon + chữ)
          Field #tag-query — SearchField “Tìm chủ đề”, help; ẩn khi đủ 3 (thay bằng câu giải thích)
          Status #tag-suggestions-status — live polite: “2 gợi ý cho “hồi”” / không có / tạm không tải được
          List #tag-suggestions — Button gợi ý: nhãn · “qua tên {alias}” · mô tả (aria-describedby)
          Details #tag-browse — “Xem các chủ đề có sẵn (n)” khi ô tìm trống; cùng Button gợi ý
      Region #tag-notices — InlineNotice của thẻ: xung đột (Dùng thẻ đang lưu / Giữ lựa chọn của tôi),
                           từ chối (thẻ cụ thể), lỗi/không chắc chắn (Thử lại), không tải được danh mục
    Dialog #leave-editor — hiện có; thông điệp nói rõ phần nào chưa lưu (chữ, thẻ hay cả hai)
```

- **Hierarchy:** bản thảo → toolbar → thông tin tác phẩm. H2 Title cho mục; chip dùng Label; mô tả gợi ý
  Caption muted. Không thêm nút Filled thứ hai: **Lưu** trên toolbar lưu cả hai phần. Hành động trong
  notice là Text/Tonal.
- **Sizing:** mục rộng bằng `--vk-measure` như trang viết; khoảng cách `--vk-space-3/4`; chip cao ≥ 44 px
  (`density.control-height` gọn), nút bỏ 48 px target; nhãn chip không cắt (`overflow-wrap: anywhere`).
- **Responsive:** một cột ở mọi bề rộng; chip `flex-wrap`; gợi ý xếp dọc, mô tả xuống dòng. Không có phần
  ẩn chỉ ở màn hẹp. 320 px/200 %: mỗi chip một dòng nếu cần, không cuộn ngang.
- **Scroll:** chỉ cuộn trang. Toolbar sticky hiện có (≥44 rem) không che ô tìm vì ô tìm ở cuối trang.
- **Overlays:** không có. Rời trang dùng Dialog hiện có.
- **Themes:** cấu trúc chung sáng/tối. Mục: nền `surface-container-low`, bo `radius-lg`. Chip đã chọn:
  `secondary-container`/`on-secondary-container`; chip ngừng dùng: `warning-container`/`on-warning-container`
  + chữ “Đã ngừng dùng”; lỗi: `error` + icon. Gợi ý: nút Text trên `surface-container-low`, hover
  `surface-container`. Focus 2 px `--vk-focus-ring`.
- **States:** catalog/thẻ: loading (“Đang tải thẻ…”), ready, empty (danh mục chưa có dạng bài/chủ đề),
  unavailable (thử lại; chữ vẫn sửa/lưu được). Gợi ý: idle, loading, ready, no-results, unavailable,
  invalid (query không có chữ/số). Lưu thẻ: saved, dirty, saving, conflict, refused (unknown/retired/
  wrong facet/too many), failed/uncertain. Thẻ đã chọn: active, retired (+ kế nhiệm), có lỗi máy chủ.
- **Accessibility:** section có heading; hai `fieldset/legend` (Dạng bài, Chủ đề); chip dạng bài
  `aria-pressed`; mỗi nút bỏ có tên “Bỏ chủ đề {nhãn}”; ô tìm có nhãn thật, Enter khi đang gõ dấu không
  chọn/không gửi; Enter ngoài composition chỉ thêm gợi ý **trùng khớp** (exact) của đúng query hiện tại;
  ↓ từ ô tìm vào gợi ý đầu, ↑/↓ giữa gợi ý, Escape về ô tìm; số gợi ý báo bằng live polite; sau khi bỏ
  chip, focus sang nút bỏ của chip kế tiếp hoặc về ô tìm; sau khi thêm, focus về ô tìm.
- **Compare:** cùng vùng/state ở 320/390/834/1024/1440, sáng/tối, chữ 100/200 %: chip không cắt, không
  cuộn ngang, CodeMirror giữ caret/undo khi thẻ đổi; keyboard/IME kiểm riêng với ảnh.

## Bàn giao cho #100

`TagChip` (chip có nút bỏ, trạng thái ngừng dùng/lỗi bằng chữ) và `TagSelector` (facts + callback,
không DTO/fetch) là owner dùng chung. Explore/tag-detail nên dùng `TagChip` cho bộ lọc đang áp dụng và
`TagSelector`/gợi ý cho chọn bộ lọc; app giữ URL, request generation và kết quả `GET /api/v1/works`.
