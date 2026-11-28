# Bàn viết Markdown Live Preview (#256), Mục lục Cách 1 (#257), Chọn/Đổi định dạng (#262) & Nút Gửi/Lưu

> [Editor R4 và delta #475](editor-r4.md) bỏ nút con mắt và hộp thoại Xem trước bản nháp trong cây lịch sử bên dưới. Markdown Live Preview, Mục lục và xem trước chuyển định dạng tiếp tục có hiệu lực.

Từ #617, Bàn viết dùng preset Document của CodeMirror. H1, mục lục và chuyển định dạng thuộc riêng Bàn viết. Xem [hợp đồng runtime và vòng đời](../../apps/vink-web/editor/README.md#preset-định-dạng-và-vòng-đời-617) và [số đo baseline/final](../development/evidence/editor-core-617/README.md).

Ngày: 18/11/2026 · Base tích hợp: `origin/develop@fd4cb980` · Nhánh: `codex/editor-live-preview`

## Bốn chiều trạng thái

- **Product decision:** `PRODUCT_CONFIRMED` theo lời chốt 16–18/11/2026 trong [#256](https://github.com/loveoverflowcom/vink/issues/256), [#257](https://github.com/loveoverflowcom/vink/issues/257), [#262](https://github.com/loveoverflowcom/vink/issues/262) và [#102](https://github.com/loveoverflowcom/vink/issues/102):
  - Một editor **Live Preview** duy nhất trong `EditorView` cho bản thảo Markdown (bỏ cặp nút Soạn thảo / Mã nguồn).
  - Thanh soạn giữ **H, B, I, Liên kết** và điểm mở **Mục lục**; chọn **Mặc định / Markdown** ở luồng tạo nháp hoặc tác vụ đổi định dạng có chủ đích (có xem trước / Hủy).
  - **Mặc định** có tên riêng (`#draft-title`) + body thuần, không đoán Markdown từ dấu `#`. **Markdown** lấy **H1 đầu tiên** làm tên tác phẩm (`#263`), không có ô nhập tên riêng. **AsciiDoc** tiếp tục `DEFERRED`.
  - Mục lục theo **Cách 1**: phân cấp bằng chữ (H1 Literata đậm, H2 bán đậm, các cấp sâu hơn nhỏ và thụt vào), nền Lavender cho mục đang viết (`aria-current="location"`), không đường nhánh/bullet/số.
  - Rail điều hướng tách bằng tầng nền (`--vk-surface-container`), không thêm divider; logo Vink ở Khám phá (`/`) kế thừa PR `#105` / `#102`.
  - Nút hành động chính trên thanh đầu trang viết (`nut-gui-luu`): lần gửi đầu (`acknowledged_version == 1`) là **Gửi** kèm icon máy bay giấy (`send`); khi sửa bản đã gửi (`acknowledged_version >= 2`) là **Lưu** kèm icon đĩa mềm (`save`); khi đang xử lý đổi thành spinner + **Đang gửi…** / **Đang lưu…**; khi thành công hiện **Đã lưu** bằng chữ nhẹ.
- **Architecture decision:** `ACCEPTED` cho hợp đồng canonical document (`VDR-DOCUMENT-SOURCE-FORMAT@r2`) và `DESIGN_PROPOSED` cho lựa chọn kỹ thuật cục bộ của editor/outline/conversion. Pipeline đọc và xuất bản Markdown thuộc [#264](../development/document-source-format.md#2-pipeline-đọc-web).
- **Implementation level:** `persistence-api` (`POST /api/v1/drafts` nhận `format` tùy chọn `default` | `markdown`) + `web` (CM6 Live Preview, AST outline Cách 1, picker tạo nháp + đổi định dạng có xem trước, nút Gửi/Lưu). CMP: `NOT_IMPLEMENTED`.
- **Reference class:** `NO_APPROVED_REFERENCE`. Các bản mẫu HTML và ảnh trên Drive (`ban-viet-live-preview.html`, `ban-viet-gui-luu.html`, thư mục 5 ảnh Drive) là `candidate / NO_APPROVED_REFERENCE` định hướng thiết kế, không thay thế kiểm chứng runtime App/PostgreSQL/Chrome.

## LayoutSpec hợp nhất

```text
LayoutSpec: Single Live Preview + Mục lục Cách 1 + Chọn/Đổi định dạng + Nút Gửi/Lưu (18/11/2026)
Context: Tác giả soạn/sửa bản thảo Mặc định hoặc Markdown tại /ban-viet và /ban-viet/:id trên Vink Web.
Intent: Giữ mặt viết yên tĩnh, một EditorView duy nhất bảo toàn nguồn và undo/IME, tách rõ nút hành động chính theo trạng thái phiên bản.
Sources: origin/develop@fd4cb980; apps/vink-web/src/pages/desk.rs, apps/vink-web/src/editor.rs, apps/vink-web/editor/src/editor.ts, apps/vink-web/style/vink.css, packages/ui-system/shared/tokens.json.
References: Bản mẫu 18/11 (ban-viet-live-preview.html, ban-viet-gui-luu.html, thư mục 5 ảnh Drive); NO_APPROVED_REFERENCE.

Screen #ban-viet (/ban-viet — DeskIndex)
  Region #heading (.vk-desk-heading)
    Heading #desk-title ("Bàn viết")
    [Khi đã có tác phẩm hoặc đang tìm] Row .vk-desk-create
      Select #create-draft-format (SelectVariant::Inset16, "Định dạng bản thảo": "Mặc định" | "Markdown", mặc định "Mặc định")
      Button #create-draft ("Tạo bản thảo")
  [Kết quả không lọc có 0 tác phẩm] EmptyState tạo bản thảo đầu tiên
    Row .vk-desk-create (cùng Select #create-draft-format)
      Button #create-draft ("Viết bản thảo đầu tiên")

Screen #ban-viet-editor (/ban-viet/:id — DraftEditor)
  Column .vk-desk--editor[data-format=default|markdown]
    Header .vk-editor-heading
      Link .vk-editor-heading__back[href="/ban-viet"] (Tonal pill + ArrowBack + "Bàn viết")
      Heading #editor-heading ("Bản nháp")
    Region #editor-toolbar (EditorToolbar, paper_first=true)
      Slot #save-slot[data-status=saved|unsaved|saving|failed|conflict][data-just-saved=true|false]
        Readout [data-testid="save-status"] ("Đã lưu" chữ nhẹ khi clean; trạng thái lỗi/xung đột khi cần xử lý)
        Action #save (ButtonVariant::Filled khi bẩn, Tonal khi Retry):
          - Version 1 (chưa gửi): SVG Send (Lucide 18×18 stroke, cách chữ 8px) + "Gửi" | khi pending: Spinner 18×18 + "Đang gửi…"
          - Version >= 2 (đã gửi): SVG Save (Lucide 18×18 stroke, cách chữ 8px) + "Lưu" | khi pending: Spinner 18×18 + "Đang lưu…"
      Actions: TooltipIconButton#preview ("Xem trước") + Native <details id="editor-save-details"> ("Thêm")
    Region .vk-editor-recovery (chỉ hiện khi failed hoặc conflict)
    Region #editor-markdown-toolbar (chỉ hiện khi format == markdown, role="group", aria-label="Công cụ Markdown")
      Group .vk-editor-format-tools
        Button #md-heading-tool (H + ExpandMore, mở menu #md-heading-menu chọn Văn bản thường / Tiêu đề cấp 1..6)
        Button #md-bold ("B" — Đậm)
        Button #md-italic ("I" — Nghiêng)
        Button #md-link (icon Liên kết)
      Button #outline-toggle[aria-expanded=true|false] ("Mục lục")
    Grid .vk-editor-workspace[data-outline=open|closed]
      Region .vk-editor-paper
        [Chỉ format == default]: TextField #draft-title + <details id="draft-title-details">
        FieldBody: Label#editor-body-label + BodyEditor [data-testid="editor-body"] (1 EditorView duy nhất)
        [Markdown có lỗi title] Lỗi H1 đầu tiên cạnh nhãn Nội dung, liên kết qua aria-describedby
      Aside #editor-outline-panel.vk-editor-outline (chỉ format == markdown & đang mở)
        Header .vk-editor-outline__top: Heading#outline-title ("Mục lục") + IconButton#outline-close ("Đóng mục lục")
        Nav [aria-label="Các tiêu đề mục trong bản thảo"]:
          OrderedList #outline-list.vk-editor-outline__list
            - Item H1 đầu tiên (.vk-editor-outline__item--title): Literata 650 1.12rem
            - Item H2 (.vk-editor-outline__item--h2): Be Vietnam Pro 600 0.89rem
            - Item H3–H6 (.vk-editor-outline__item--sub): 0.83rem, thụt --outline-indent (tối đa 3 nấc * 16px desktop / 12px hẹp)
            - Nút chọn mục: min-block-size 48px, nền Lavender --vk-primary-container + aria-current="location" khi đang viết
          Empty/Pending #outline-empty ("Chưa có tiêu đề mục" | "Đang cập nhật mục lục…")
    Region .vk-editor-work-info (WorkInfo — gắn thẻ TAG-PR-05)
    Region #editor-format-region (đổi định dạng có chủ đích)
      Select #editor-format-select (SelectVariant::Inset16, "Định dạng bản thảo": "Mặc định" | "Markdown")
      Dialog #format-convert-dialog (xem trước định dạng đích, tiêu đề dự kiến, nguồn văn bản dự kiến; #format-convert-cancel + #format-convert-apply)
    Region .vk-publication (PublicationPanel — gửi duyệt revision đã lưu)

Hierarchy: Một primary CTA trên #editor-toolbar theo trạng thái dirty/saving/retry; ở format Mặc định giữ nguyên alignment #editor-toolbar / .vk-editor-paper / .vk-work-info; ở format Markdown thanh công cụ nằm ngay trên vùng soạn và cột mục lục nằm bên phải khi đủ rộng.
Sizing: Nút điều khiển đạt tối thiểu 48×48 CSS px (hoặc ≥44px compact); icon trong #save cố định 18×18 CSS px, gap 8px; Select Inset16 dùng 16/16/52 CSS px; cột mục lục 17rem khi viewport rộng ≥1024px ở scale 100%, gap 24px.
Responsive: <1024px hoặc chữ 200%, #editor-outline-panel chuyển thành panel/sheet nổi toàn chiều ngang có backdrop và trả focus về #outline-toggle khi đóng; các cụm nút trong #editor-markdown-toolbar tự xuống hàng không tràn ngang ở 320px/200%.
Scroll: Trang dùng một trục cuộn chính; nhảy mục lục dùng scrollIntoView({ y: "center" }) trên EditorView.
Overlays: #md-heading-menu dùng role="menu" gọn dưới #md-heading-tool (Escape trả focus về #md-heading-tool); #format-convert-dialog dùng shared vink_ui::Dialog với Escape = Hủy và trả focus về #editor-format-select; trên mobile/200%, sheet mục lục dùng Dialog/sheet có Escape trả focus về #outline-toggle.
Themes: Light/Dark dùng chung token --vk-surface, --vk-surface-container, --vk-primary, --vk-primary-container, --vk-on-primary-container, --vk-on-surface-variant.
States: phân biệt rõ loading, clean ("Đã lưu"), dirty ("Gửi"/"Lưu"), pending ("Đang gửi…"/"Đang lưu…"), failed/uncertain, conflict, outline empty/partial/ready, và conversion blocked khi composing/saving/conflict.
Interaction: Caret vào dòng heading lộ dấu # với class .cm-markdown-marker, rời dòng ẩn dấu bằng Decoration.replace({}); chạm vùng **đậm**/*nghiêng*/[liên kết](url) lộ cú pháp cục bộ; khi view.composing/compositionStarted chỉ map decoration theo changes để bảo vệ IME tiếng Việt; mọi lệnh H/B/I/Liên kết kiểm tra generation/composing/readOnly và ghi một bước Undo.
Accessibility: 1 H1 trang (#editor-heading), aria-current="location" trên mục đang viết, vùng bấm ≥48px, aria-busy và aria-disabled trên #save khi đang gửi/lưu, vòng focus visible 2px.
Invariants:
  - Icon của #save: 18×18 CSS px, stroke-width 2px, cách nhãn 8 CSS px (dung sai ±1px).
  - SelectVariant::Inset16 (#create-draft-format, #editor-format-select): icon 16×16px, cách inline-end 16px, padding-inline-end 52px (±1px).
  - Không tràn ngang (scrollWidth <= clientWidth + 1) ở 320/390/834/1024/1440px × 100%/200%.
  - Một EditorView duy nhất không bị remount khi ẩn/hiện cú pháp, mở/đóng mục lục, đổi theme hoặc đổi định dạng qua conversion.
Compare: So sánh ảnh chụp và assertion thực tế trên Chrome (Light/Dark, 320/390/1024/1440, 100%/200%) cho cả Default và Markdown.
```

## Lựa chọn kỹ thuật

- **`#256 CHOICE-01`:** Chọn **A + phối hợp bounded**: dùng `commonmarkLanguage` (`@codemirror/lang-markdown`) cùng `ViewPlugin` đọc `view.visibleRanges` để gắn line decoration (`cm-writing-h1..6`, `cm-writing-quote`) và inline mark/replace decorations (`HeaderMark`, `StrongEmphasis`, `Emphasis`, `Link`, `InlineCode`, `QuoteMark`), kèm guards trong `compositionStarted` chỉ map `decorations.map(update.changes)`. Lý do: tái dụng incremental parse của Lezer/CodeMirror mà không parse lại toàn văn bản mỗi khi di chuyển caret. Với link nhiều dòng, replacement chỉ che cú pháp trong từng dòng nhìn thấy và giữ nguyên các chỗ xuống dòng.
- **`#256 CHOICE-02`:** Chọn **B — CommonMark 0.31.2 lõi không extension**, đồng nhất với `packages/markdown-text` dùng chung khi tính tiêu đề ở server và xem trước chuyển đổi (`pulldown-cmark` `Options::empty()`). Mọi cú pháp ngoài CommonMark lõi (fence, HTML thô, escape) hiển thị nguyên văn an toàn.
- **`#257 CHOICE-01`:** Chọn **A — duyệt có giới hạn (`ensureSyntaxTree(state, doc.length, 8)`) trên cùng cây cú pháp** và công bố trạng thái `ready` / `partial` kèm `generation` (bộ đếm tăng khi sửa nguồn hoặc đổi định dạng) để chặn click mục cũ (`stale`). Khi cây còn partial, tiếp tục parse ở frame sau, kể cả khi tác giả không gõ thêm.
- **`#257 CHOICE-02`:** Chọn **Cột phải (landmark ở desktop) ở desktop rộng (`>= 1024px`, scale 100%) và sheet/modal dialog ở màn hẹp hoặc chữ `200%`**, kiểm `Escape` và trả focus về `#outline-toggle`.
- **`#262 CHOICE-02`:** Chọn **A — đổi định dạng in-place nguyên tử trên cùng `draft` qua `PUT /api/v1/drafts/{id}` với `expected_version`** sau khi tác giả xem trước tiêu đề/nguồn trong `#format-convert-dialog` và bấm `Áp dụng` (`#format-convert-apply`). Khi mất phản hồi, đọc lại source, format, title và version trước khi xác nhận. Tiêu đề quá 200 ký tự sau NFC chặn Áp dụng. Khi `Mặc định -> Markdown`, tiêu đề riêng được gắn thành `# <tiêu đề>` ở đầu (nếu chưa có và khác rỗng) và các dòng bắt đầu bằng `#` được escape (`\#`) trong bản xem trước rõ ràng; khi `Markdown -> Mặc định`, H1 đầu tiên trở thành tiêu đề riêng và giữ nguyên toàn bộ chữ nguồn.
