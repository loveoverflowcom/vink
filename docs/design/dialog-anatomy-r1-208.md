# Chuẩn hóa anatomy, gutter và phân cấp hành động Dialog Editor — R1 (#208)

> Bàn giao lịch sử. [Editor R4 và delta #475 ngày 24/11](editor-r4.md) bỏ hộp thoại xem trước bản nháp độc lập và mô tả bố cục hiện tại. Các bằng chứng bên dưới giữ nguyên phạm vi và thời điểm gốc.

**Trạng thái:**
- **Product / UX:** `DESIGN_PROPOSED` (PR 1/2 thuộc phạm vi [#208](https://github.com/loveoverflowcom/vink/issues/208); không tự đánh dấu `Accepted`/`approved` hay thay thế chủ nguồn nghiệp vụ).
- **Reference class:** `NO_APPROVED_REFERENCE` (hình ảnh đích `R1` chỉ là phác thảo bố cục nội bộ; không dùng làm chuẩn pixel bắt buộc).
- **Implementation level:** `web` (`vink-ui` Web + `apps/vink-web` `#leave-editor` / `#draft-preview`; bổ sung `ButtonVariant::DangerText` và `VinkDialog.secondaryActions` ở chữ ký CMP nhưng runtime native CMP chưa chạy kiểm thử trong PR này).

## Phạm vi và lựa chọn kỹ thuật

- **CHOICE-01 (B — mở rộng `Dialog` opt-in & gom quyền sở hữu anatomy):**
  `Dialog` (`packages/ui-system/leptos/src/feedback.rs`) nhận `context: Option<Signal<String>>` (reactive, vẫn nhận chuỗi tĩnh) và `secondary_actions: Option<ViewFn>` (opt-in cho nhóm hành động phục hồi/phá hủy thứ cấp). `vink-ui.css` sở hữu chung bề rộng lề (`--vk-overlay-margin`), gutter (`--vk-overlay-gutter: clamp(var(--vk-space-3), 10cqi, var(--vk-space-6))` = 24px tại desktop và 390px/100%), vùng cuộn (`overflow-y: auto`, `overscroll-behavior: contain`, `scroll-padding-block: var(--vk-size-hit-target)`), khoảng trống divider-free (`space-3` giữa `title`–`context`, `space-4` trước `actions` và giữa nhóm chính–phụ) và stacking ở `@container vk-overlay (max-width: 24rem)`.
- **CHOICE-02 (A — biến thể ngữ nghĩa `ButtonVariant::DangerText`):**
  Thêm `ButtonVariant::DangerText` (nền trong suốt, chữ `var(--vk-error)` — đo WCAG contrast `5.03:1` Sáng trên `#e8e0f0` và `7.58:1` Tối trên `#372e42`, trên sàn 4.5:1) và đổi `Button.variant` nhận `Signal<ButtonVariant>` để chuyển cấp CTA tại chỗ mà không remount DOM/mất focus. `ButtonVariant::Danger` đặc chỉ dùng ở bước xác nhận cuối (`#discard-confirm`).

## LayoutSpec R1 đã triển khai

```text
Dialog #leave-editor / #draft-preview
  Surface (--vk-surface-container-high, radius-xl)
    Panel (padding: --vk-overlay-gutter = clamp(space-3, 10cqi, space-6))
      Title (#{id}-title)
      Context (#{id}-context, --vk-on-surface-variant)
      [Content (.vk-ui-dialog__content, ẩn khi rỗng; #draft-preview chứa article.vk-preview)]
      Actions (.vk-ui-dialog__actions, safe action đầu tiên trong DOM)
      [SecondaryActions (.vk-ui-dialog__actions--secondary, ẩn khi rỗng, ngăn bằng space-4, không divider)]
```

### Phân cấp hành động theo trạng thái

*(Delta #254 ngày 17/11/2026: bỏ `#leave-export` khỏi nhóm phụ của `#leave-editor`; giữ `#leave-stay`, `#leave-save`, `#leave-discard`, `#discard-back` (`Tonal`) và `#discard-confirm` (`Danger`).)*

- **`#leave-editor` (có thể lưu — Unsaved / Failed):** `#leave-stay` (`Text`, đầu DOM) · `#leave-save` (`Filled`, lý do qua `aria-describedby="leave-editor-context"`) · nhóm phụ nếu `can_discard`: `#leave-discard` (`DangerText`).
- **`#leave-editor` (xung đột — Text/Tag Conflict):** `#leave-stay` (`Filled`, đầu DOM) · `#leave-save` (`Tonal`, `ActionState::Disabled` + giải thích xung đột ở `#leave-editor-context`) · nhóm phụ: `#leave-discard` (`DangerText`).
- **`#leave-editor` (xác nhận bỏ thay đổi — `ConfirmingDiscard`):** `#discard-back` (`Tonal`, đầu DOM + focus khởi đầu an toàn) · `#discard-confirm` (`Danger` đặc).
- **`#draft-preview`:** `title="Xem trước bản nháp"` (`focus_title=true`) · `context` ở slot `Dialog` · `<article class="vk-preview">` (`DocumentView` giữ khổ/thụt lề thơ) · `#preview-close` (`Tonal`).
- **Xóa bỏ khỏi `apps/vink-web/style/vink.css`:** xóa `.vk-dialog__body`, `.vk-dialog__actions`, `.vk-dialog__recovery` (cùng divider `border-top`), override `padding: clamp(...)` trên `.vk-ui-dialog__panel`, `@media (max-width: 40rem)` `padding: var(--vk-space-2)` (8px), và `#leave-editor .vk-ui-dialog__actions:empty { display: none; }`.

## Đánh giá 5 chiều & giới hạn bằng chứng

| Chiều | Kết quả | Bằng chứng chính |
| --- | --- | --- |
| 1. Chức năng | Đạt | Giữ nguyên `LeaveGuard`, `can_discard`, `ActionState::Disabled` khi xung đột/đang lưu, export buffer đang sửa, late ACK, guard điều hướng và khôi phục scroll/caret/selection/undo |
| 2. Hình học | Đạt | `verify web --scenario main` (`editorChrome` + `mainPhase`) & `ui-preview leptos verify` (`838` captures, `state.dialog` `16` ca `INV-1..5`): gutter `>=24px` ở `>=390px/100%`, `0` horizontal overflow, nút `>=44px`, divider `0px` |
| 3. Thị giác | Đạt | Đã mở kiểm PNG thật tại `320/390/834/1440` × Sáng/Tối × `100/200%` (conflict, confirm, preview); tối đa `1` nút `Filled`; contrast `DangerText` `5.03:1` (Sáng) / `7.58:1` (Tối) |
| 4. Trợ năng & Responsive | Đạt | Tiêu đề thấy khi mở (`scrollTop=0`), focus ring không cắt, Tab/Shift+Tab/Escape/reopen giữ trong dialog, `#leave-save` disabled có `aria-describedby="leave-editor-context"`, chữ Việt ngắt theo từ nguyên vẹn |
| 5. Tương tác | Đạt | Chọn hủy rồi `Quay lại` trả focus về `#leave-discard`; `Ở lại` / Escape trả focus & vùng chọn CodeMirror + `scrollY` sau 2 frame |

- **Các phạm vi `NOT_RUN` (giữ ranh giới trung thực):** OS IME thật ngoài CDP, browser zoom gốc, thiết bị/bàn phím di động thật, screen reader thật, Safari/Firefox, và runtime CMP/Android/iOS của `ButtonVariant::DangerText` / `secondaryActions`.
- **Liên kết liên quan:** [Contract UIS-03](../../packages/ui-system/shared/states.md), [Contract UIS-02](../../packages/ui-system/shared/components.md), [Trạng thái & phản hồi](state-and-feedback.md), [Editor R4/R5](ui-editor-r5.md), [Feature map](../development/feature-map.md).
