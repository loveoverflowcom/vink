# Giao diện Editor yên tĩnh — #254 (Editor 1/7)

## Trạng thái trên bốn trục độc lập

- **Product decision:** `PRODUCT_CONFIRMED` theo yêu cầu chủ sản phẩm ngày 16/11/2026 tại [Issue #254](https://github.com/loveoverflowcom/vink/issues/254) (cập nhật delta `WR-01` trong [PRD](../../prd/requirements.md): bỏ `"Bản thử nghiệm"`, hướng dẫn ngắt dòng dài và toàn bộ nút tải bản thảo `.txt`, giữ cơ chế chống mất nội dung; làm lại nút quay về Bàn viết dạng Tonal + icon `ArrowBack`).
- **Architecture decision:** kế thừa `D-04@r1` (`DESIGN_PROPOSED`); thay đổi trình bày cục bộ ở shell/editor và loại bỏ client export `.txt`, không đổi `DraftSession`, `EditorSession`, `NavigationGuard`, `LeaveGuard` hay domain/API.
- **Implementation level:** `web` (`apps/vink-web`, cập nhật sample `screen.editor` trong `packages/ui-system/leptos/preview` và test slot trong `packages/vink-patterns/leptos/tests/editor_tools.rs`).
- **Reference class:** `NO_APPROVED_REFERENCE`. Đã đối chiếu `mockup.html` và ảnh phác thảo sáng/tối trên desktop/mobile chỉ ở mức tham khảo nhịp thị giác:
  - Giữ trong `#254`: nút `← Bàn viết` dạng Tonal pill bằng semantic tokens (`--vk-action-tonal-container` / `--vk-action-tonal-content`), căn trái thẳng hàng mép giấy viết, mặt viết phẳng tách với nền bằng bề mặt (`--vk-surface` trên `--vk-surface-container-low`, không thêm resting border hay `box-shadow` để giữ bất biến `#101`/`#218` và bề rộng viết `#193`), cùng phân cấp `#discard-back` (`Tonal`) / `#discard-confirm` (`Danger`).
  - Loại trừ khỏi `#254`: thanh định dạng Markdown (`#256`), mục lục (`#257`), bộ chọn thể loại trên Bàn viết (`#262`), hợp nhất H1 tiêu đề (`#263`), nhãn thời gian lưu giả định và footer đếm từ/thẻ.

## LayoutSpec và `CHOICE-01 A`

```text
Scaffold (không còn footer #build-info “Bản thử nghiệm”)
  └── .vk-desk--editor
        ├── Header (.vk-editor-heading, align-items: center)
        │     ├── Back link (a.vk-editor-heading__back[href="/ban-viet"], Tonal pill + Icon ArrowBack + "Bàn viết")
        │     └── Heading#editor-heading ("Bản nháp")
        ├── EditorToolbar (#editor-toolbar · paper_first=true, không utilities export trong #editor-save-details)
        ├── Recovery band có điều kiện (.vk-editor-recovery: #retry-save / #keep-mine + details, không nút tải .txt)
        ├── Paper (.vk-editor-paper: #draft-title + #draft-title-details + CodeMirror 6; không .vk-editor-help)
        ├── WorkInfo (TAG-PR-05: giữ #tags-use-stored / #tags-keep-mine / #tags-retry / #tags-reload, không #export-tags)
        └── Dialog#leave-editor (Choosing: #leave-stay, #leave-save, #leave-discard; Confirm: #discard-back, #discard-confirm)
```

### Lựa chọn `CHOICE-01 A` — giữ thẻ `<a href="/ban-viet">` và tạo hình Tonal + icon

- **Chọn A:** giữ `<a class="vk-editor-heading__back" href="/ban-viet">` bọc `<Icon name=ArrowBack size=Small />` và `<span>"Bàn viết"</span>`, là một click target duy nhất (`>= 48px` chiều cao qua `--vk-density-control-height`), hỗ trợ click phím bổ trợ / mở tab mới của trình duyệt, và kích hoạt `NavigationGuard` qua bộ chặn `a[href]` sẵn có.
- **Lý do bỏ B (thay bằng `<Button>` JS):** `<Button>` mất ngữ nghĩa điều hướng gốc của link, buộc điều hướng bằng tay và dễ tạo nested interactive target.
- **Responsive, theme, 200% text và forced-colors:** bỏ `margin-inline-start: calc(-1 * var(--vk-space-3))` để mép pill Tonal thẳng trục với `#editor-toolbar` và `.vk-editor-paper` ở mọi độ rộng (`320–1440px`); `max-inline-size: 100%` ngăn tràn ngang ở `320px / 200%`; hover/active dùng `::after` 8%/12% `currentColor`; focus giữ `outline: 2px solid var(--vk-focus-ring); outline-offset: 0;` khớp token nút Tonal; `forced-colors: active` dùng `border-color: LinkText; color: LinkText; outline-color: Highlight;`.

## Trạng thái chống mất nội dung

- Bỏ mọi nút và wording tải `.txt` (`#export`, `#export-failed`, `#export-conflict`, `#export-tags`, `#leave-export`, `export_text`, `export_file_name`, `download_text`).
- Notice phục hồi chỉ hướng dẫn giữ trang / `Thử lại` / `Lưu đè bằng nội dung đang viết` / xử lý xung đột thẻ trên trang; buffer đang viết, `NavigationGuard`, `beforeunload` và xác nhận hai bước `#leave-editor` giữ nguyên.

## Đánh giá 5 chiều và phạm vi kiểm chứng

| Chiều | Kết quả | Bằng chứng quan sát |
| --- | --- | --- |
| 1. Chức năng | Đạt | `verify web --scenario main --scenario restart` PASS: giữ nguyên buffer live/NFD, lưu/reload/restart, conflict (`409` / `#keep-mine`), tag conflict/recovery, `NavigationGuard`, `beforeunload`, xác nhận 2 bước `#leave-editor`; xóa `#build-info`, `.vk-editor-help`, toàn bộ `#export*` và `#leave-export` |
| 2. Hình học | Đạt | Đo thật tại `paper-clean-320-100-light`: `.vk-editor-heading__back` (`x: 8`, `height: 48`, `width: 115.3`), `#editor-toolbar` (`x: 8`), `.vk-editor-paper` (`x: 8`), `.cm-content` `width: 273` giữ baseline `#193`; `0` tràn ngang (`320–1440`, `100%/200%`), `/toi` giữ bottom clearance không footer `#build-info` |
| 3. Thị giác | Đạt | Đã mở kiểm ảnh chụp thật (`paper-loading`, `paper-no-js`, `paper-clean-320-100-light`, `paper-clean-1440-100-light-tools`, `editor-clean-1440-100-light/dark`, `editor-clean-320-200-dark`, `editor-conflict-320-100-light-recovery`, `editor-conflict-1440-100-dark-recovery`, `editor-failed-320-100-light-recovery`, `editor-leave-390-100-dark`, `editor-leave-confirm-return-390x420-200-dark`, `screen.editor--default--light--1440`, `screen.editor--default--dark--390`): tương phản tối thiểu trên các phần tử đo của editor đạt `6.28:1` (Sáng) / `7.84:1` (Tối) |
| 4. Trợ năng & Responsive | Đạt | Chrome AX tree lộ link `Bàn viết`, `Thêm` (`DisclosureTriangle`), `Xem trước` (`button`), hộp thoại rời không chứa nút tải; vòng focus rõ khi thu/phóng (`320–1440`, `100%/200%`) và `forced-colors` (`LinkText` / `Highlight`); SSR không JS giữ nút `← Bàn viết` và H1 `Bản nháp` |
| 5. Tương tác | Đạt | Bàn phím Tab/Enter/Space/Escape, focus trả về `#leave-discard` / `.cm-content`, giữ nguyên `EditorView`, caret/selection và scroll |

- **Bằng chứng trình duyệt:** `cargo xtask verify web --scenario main --scenario restart` (PostgreSQL 17.11 + Google Chrome 157) và `cargo xtask verify preview` (902 captures, 0 lỗi console); `visual-evidence.test.mjs` (9/9) cùng `verify quick` và `cargo test --locked -p vink-patterns --tests` kiểm tra tính hợp lệ liên kết, định dạng và unit/contract sau sửa đổi.
- **Chưa chạy (`NOT_RUN`):** `verify web` cho các scenario tách riêng khác (`endorsement`, `review`, `auth`, `password`, `publication`), `verify postgres`, `verify discussion`, `verify expressions`, `verify kani`, CMP/Android/iOS, thiết bị di động thật, screen reader thật, Safari/Firefox và OS IME ngoài CDP.

## Tài liệu liên quan

- [Paper First #218](ui-editor-paper-first-218.md) · [Reflow #193](editor-writing-reflow-193.md) · [Dialog Anatomy #208](dialog-anatomy-r1-208.md) · [PRD WR-01](../../prd/requirements.md) · [Feature map](../development/feature-map.md)
