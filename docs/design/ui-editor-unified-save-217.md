# Hợp nhất trạng thái–hành động lưu trong Bàn viết — #217 (PR 1/2)

> Bàn giao lịch sử. Projection lưu và slot `#save-slot` vẫn có hiệu lực. [Editor R4](editor-r4.md)
> bỏ `#preview`, `#export` và nhóm secondary actions của `EditorToolbar`; thanh lưu chỉ còn
> readout, `save_action` và “Chi tiết lưu”.

Proposal kỹ thuật [VDR-EDITOR-SAVE-PROJECTION@r1](../../decisions/proposals/VDR-EDITOR-SAVE-PROJECTION.r1.toml) **PENDING** ghi projection và guard thủ công đã triển khai; không chốt autosave, không thay D-04.

## Trạng thái trên bốn trục độc lập

- **Phạm vi sản phẩm:** [issue #217](https://github.com/loveoverflowcom/vink/issues/217) yêu cầu hợp nhất trạng thái và thao tác lưu thủ công. Đối chiếu `WR-01`, `WR-02`, `WR-05`, `UX-01` ([PRD](../../prd/requirements.md)) và [`INV-08`](../architecture/decisions.md). Autosave trong `WR-01` và khôi phục cục bộ vẫn chưa triển khai; thay đổi UI không tạo quyết định `PRODUCT_CONFIRMED` mới.
- **Quyết định kiến trúc:** `D-04@r1` giữ `DESIGN_PROPOSED`; đây là thiết kế UI và projection ở client thuộc phạm vi PR 1/2.
- **Mức triển khai:** `web` (`apps/vink-web`, `packages/vink-patterns/leptos`, `packages/ui-system/leptos/preview`, `apps/vink-web/editor`). `cmp` chỉ giữ chrome mẫu tương thích tại `Patterns.kt`, chưa có editor thật hay native save runtime.
- **Bằng chứng kiểm chứng:** Chrome headless CDP (`ui-preview leptos verify`, `verify web` đầy đủ) + PostgreSQL scratch thực + Cargo unit/clippy/UI token checks (`verify quick`); chưa kiểm thiết bị mobile thật, Safari/Firefox, OS IME hay screen reader thật. Không coi preview mẫu là bằng chứng lưu/xác thực.

## Phạm vi, owner và base

- Issue [#217](https://github.com/loveoverflowcom/vink/issues/217), kế thừa [UI R5 (#101)](ui-editor-r5.md), [R1.4](editor-r14.md) và [Reflow (#193)](editor-writing-reflow-193.md); base triển khai `6de515d`; review tích hợp trên `origin/develop@a3e6536`.
- Fidelity tham chiếu: `NO_APPROVED_REFERENCE` (kế thừa R5 Lavender tokens, tự kiểm theo rubric 5 chiều của `ux-quality`).
- Owners:
  - State & guards: [`DraftSession` / `EditorSession`](../../apps/vink-web/src/desk_state.rs), [`TagSession`](../../apps/vink-web/src/tag_state.rs), [`TagLoad`](../../apps/vink-web/src/pages/work_info.rs)
  - Pattern & preview: [`EditorToolbar`](../../packages/vink-patterns/leptos/src/lib.rs), [`patterns.css`](../../packages/vink-patterns/leptos/patterns.css), [`screens.rs`](../../packages/ui-system/leptos/preview/src/screens.rs)
  - Web desk & CSS & CM bridge: [`Editor` / `project_save_slot`](../../apps/vink-web/src/pages/desk.rs), [`vink.css`](../../apps/vink-web/style/vink.css), [`editor.ts`](../../apps/vink-web/editor/src/editor.ts)

## LayoutSpec và `CHOICE-01 B`

```text
Header Bàn viết / Quay lại
  └── Toolbar (#editor-toolbar · .vk-editor-toolbar, sticky ≥44rem & ≥40rem trừ lúc mở chi tiết)
        ├── Save slot thống nhất (#save-slot · .vk-editor-toolbar__status.vk-save-slot)
        │     ├── Nhãn trạng thái sống duy nhất ([data-testid="save-status"], role="status", aria-live="polite")
        │     ├── Nút hành động theo ngữ cảnh (chỉ hiện khi cần, giữ cùng #save-slot)
        │     │     ├── Dirty / Retryable fail: Button#save (Filled khi dirty, Tonal khi Retry; pending -> ActionState::pending)
        │     │     └── Conflict / thẻ bị từ chối hay chưa tải được: đưa đến khôi phục nếu không còn phần có thể lưu
        │     └── SupportingDetails#editor-save-details ("Chi tiết trạng thái lưu")
        └── Secondary actions (.vk-editor-toolbar__actions)
              ├── TooltipIconButton#preview ("Xem trước")
              └── TooltipIconButton#export ("Tải bản thảo (.txt)")
  ├── Recovery band (.vk-editor-recovery — ngoài mọi disclosure, Announce::Off)
  └── Writing paper (#draft-title + BodyEditor CodeMirror đơn nhất + Công khai)
```

### Chọn `CHOICE-01 B` thay vì `CHOICE-01 A`

- **Option B (được chọn):** `#save-slot` giữ một live readout tĩnh (`✓ Đã lưu`, `--vk-on-surface-variant` text + `--vk-success` `IconName::Check` khi clean; không hiển thị `#save` CTA) và hoán đổi có điều kiện với `<Button id="save">` (`dirty`, `saving`, `failed`) hoặc `<Button id="save-recovery-jump">` (`conflict`, `refused`). Khi `send_text` / `send_tags` nhận ACK làm `#save` rời DOM, `sync_save_slot_focus` kiểm tra **chỉ khi** focus đang nằm trên `#save` mới trả focus (`preventScroll`) vào `#editor-save-details > summary` (clean) hoặc `#save-recovery-jump` (conflict/refused); nếu focus đang ở `BodyEditor`, `#draft-title` hay `WorkInfo` thì tuyệt đối không chạm focus/caret/selection/cuộn.
- **Lý do không chọn Option A** (dùng một `<button id="save">` xuyên suốt đổi nhãn sang `"✓ Đã lưu"` để mở details): làm nút mang hai vai vừa gửi mutation vừa mở disclosure cạnh `<SupportingDetails id="editor-save-details">` sẵn có, phá tách bạch tĩnh/hành động của AC-01.
- **Đánh đổi & kiểm chứng:** Option B cần đồng bộ focus khi nút `#save` bị gỡ; đã khóa bằng regression tự động `dirty -> pending -> saved` và `dirty -> pending -> conflict` trong `browser-smoke`.

## Sự thật trạng thái chữ + thẻ và chống ghi thừa

1. **Không sửa toàn cục `DraftSession::can_save` / `begin_save`:** `PublicationPanel`, `WorkInfo` và `LeaveGuard` tiếp tục dùng hợp đồng cũ. Riêng hành động lưu thủ công trên toolbar, form `Enter` tiêu đề và `Ctrl/Cmd+S` (ngoài IME composition) gọi `DraftSession::begin_manual_save` (`needs_manual_save`): không ghi khi đã lưu hoặc còn bất kỳ yêu cầu lưu nào đang chờ. Xung đột hoặc từ chối chỉ chặn phần liên quan: có thể lưu chữ khi thẻ bị từ chối và lưu thẻ khi chữ xung đột. Từ chối đến muộn không khóa lựa chọn nếu tác giả đã bỏ thẻ bị từ chối.
2. **Projection hợp nhất (`project_save_slot`):** tổng hợp `SaveStatus`, `EditorSession::has_pending_newer_edits()`, `Option<TagStatus>`, `TagLoad`, `TagSession::has_pending_newer_selection()`.
   - Chỉ hiện `Đã lưu` khi cả chữ và thẻ đã được xác nhận; lúc đang tải hoặc lỗi tải thẻ, nêu rõ mới xác nhận phần chữ. Tách rõ partial dirty (`Chưa lưu chữ`, `Đã lưu chữ; chưa lưu thẻ`), both dirty (`Chưa lưu chữ và thẻ`), pending + sửa mới trong lúc chờ (`Đang lưu; có chữ mới…`), unconfirmed vs rejected (`Chưa xác nhận lưu.` vs `Chưa lưu được.`, gồm partial text saved + tags uncertain/rejected/refused/conflict), conflict (`Có bản mới trên máy chủ.` / `Đã lưu chữ; thẻ đã đổi ở nơi khác.`).
   - Mọi bản sao/chi tiết nêu rõ máy chủ (`trên máy chủ`), không dùng copy khôi phục trên máy hay autosave.
   - CSS `.vk-save-status` chuyển từ exception `0.9375rem` sang token `--vk-type-label-size` (xóa entry `style_exceptions` cũ trong [`api-coverage.json`](../../packages/ui-system/shared/api-coverage.json)); cảnh báo uncertain/conflict dùng `warning-container` / `on-warning-container`.
3. **Ổn định caret sau scale change (`editor.ts`):** bổ sung `ensureCaretVisible` (tối đa 2 `rAF`, chỉ chạy khi editor đang focus và có `tr.scrollIntoView`) để sau khi root text scale co lại (`200% → 100%`) trong lúc `.cm-editor` từng nằm ngoài viewport, CodeMirror hoàn tất rút gọn `heightMap` vẫn giữ con trỏ trong vùng nhìn thấy.

## Giao diện bàn giao cho PR 2/2 (#218)

PR 2/2 (#218) kế thừa trực tiếp seam sau mà không đổi logic save hay status projection:

- **Primary slot `#save-slot` trong `EditorToolbar`:** giữ `id="save-slot"`, `data-status`, `data-problem`, live readout `[data-testid="save-status"]` (`role="status"` duy nhất), `#editor-save-details` và slot `save_action` (`#save` / `#save-recovery-jump`).
- **Secondary actions `{children()}` trong `.vk-editor-toolbar__actions`:** hiện giữ `#preview` và `#export`. PR 2/2 có thể gom hoặc chuyển các secondary tools / details sang overflow menu ở header mà không cần sửa `DraftSession::needs_manual_save` / `begin_manual_save`, `project_save_slot`, `sync_save_slot_focus` hay live announcement owner.

### Delta trình bày #218 (thay seam anatomy, giữ protocol)

[Paper First](ui-editor-paper-first-218.md) dùng `paper_first=true`:
`#editor-save-details > summary` là chính trigger **Thêm** luôn hiển thị, ngoài
bất kỳ disclosure đóng nào. Chi tiết máy chủ và export thường nằm bên trong;
`#preview` trực tiếp, recovery/export lỗi vẫn trực tiếp. Selector handoff của
`sync_save_slot_focus` không đổi, nhưng đích giờ là Thêm thay vì icon Chi tiết.
Không tự mở tools khi ACK, không cướp focus đang gõ. Default pattern giữ anatomy cũ.
Nội dung và kết quả #217 bên dưới là lịch sử của PR1, không evidence cho delta #218.

## Kiểm chứng ban đầu tại `18885ee` (độ tương phản, đơn vị, preview và trình duyệt)

- **`cargo xtask verify quick`:** `ui tokens` (15 files khớp), `expressions` (3/3), `fmt`, `clippy` (SSR/hydrate WASM/preview CSR WASM), 22 + 31 doctests, `unit + contract tests` (`desk_state::tests::manual_save_skips_clean_pending_conflict_and_unchanged_refusal` và `editor_presentation_tests` trong `desk.rs` mở rộng), `repository checks` (xóa style exception `0.9375rem`).
- **`cargo xtask ui-preview leptos verify`:** build + media hydration + **838 captures** qua Chrome CDP (`screen.editor` cập nhật cho `default`/`dirty`/`pending`/`error`/`uncertain`/`conflict`, 0 errors). *(Đã thêm `stream.set_nonblocking(false)` tại `xtask/src/ui_preview.rs` vì `accept()` trên macOS/BSD kế thừa `O_NONBLOCK` từ listener khiến file `.wasm` bị ngắt giữa chừng).*
- **`cargo xtask verify web` (toàn bộ 9 scenarios trên PostgreSQL scratch + Identity thật + Chrome CDP):** PASS 100% (`main`, `storage`, `discovery`, `restart`, `endorsement`, `review`, `auth`, `password`, `publication`).
- **Độ tương phản chữ thực đo bởi Chrome (`textContrast`, `1440px`, 100%):**
  - Clean (`[data-testid=save-status]` `"Đã lưu"` trên `surface-container-high`): Light **5.40:1**, Dark **7.68:1** (cả hai ≥ 4.5:1; `#editor-save-details > summary` 12.69:1 / 10.72:1).
  - Dirty/Saving (`#save`): Light **6.98:1**, Dark **8.16:1** (`status` 5.40:1 / 7.68:1).
  - Uncertain/Conflict (`[data-testid=save-status]` trên `warning-container`): Light **8.37:1**, Dark **7.84:1**; `#save` (Tonal fail): **11.14:1** / **7.87:1**; `#save-recovery-jump`: **7.87:1** / **7.87:1**.

### Đánh giá UX 5 chiều

| Chiều đánh giá | Kết quả | Bằng chứng / Ghi chú |
| --- | --- | --- |
| Phân cấp & Nhịp | Đạt | Clean chỉ còn `✓ Đã lưu` tĩnh nhẹ (không còn nút `Lưu` nổi bật vô nghĩa); dirty mới dựng CTA chính `Lưu`; conflict/uncertain nổi bằng container vàng và ôm gọn nội dung (`flex: 0 1 auto`) |
| Bố cục & Hồi đáp hình học | Đạt | 1440 / 1024 / 834 / 390 / 320px × Sáng / Tối × 100% / 200%, 0 overflow, hit target ≥ 48px, mở ảnh PNG xác nhận 320/390/1440 và short viewport |
| Chủ đề & Tương phản | Đạt | Light & Dark đo thật trên Chrome: clean status 5.40:1 / 7.68:1, conflict/uncertain 8.37:1 / 7.84:1, buttons ≥ 6.98:1 |
| Tương tác bàn phím & IME | Đạt | Tab order nhất quán, focus không rơi về `<body>` qua `dirty → pending → saved / failed / conflict`, `Ctrl/Cmd+S` trong CodeMirror giữ con trỏ/EditorView, composition vẫn chặn lưu sớm |
| Trung thực trạng thái & Dữ liệu | Đạt | Tách clean / partial text+tags / newer edits while pending / unconfirmed / rejected / refused / conflict; clean toolbar/shortcut không phát `PUT` thừa; không hứa autosave hay lưu trên máy |
