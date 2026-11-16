# Editor Paper First — #218

Base thực thi: `develop@a78c9ff5138a0df26e9febe4fdf6146f5915d28a`, đã có
[#227](https://github.com/loveoverflowcom/vink/pull/227) và
[#228](https://github.com/loveoverflowcom/vink/pull/228). Kế thừa R3 của
[#218](https://github.com/loveoverflowcom/vink/issues/218),
[#217](ui-editor-unified-save-217.md), title/gutter [#193](editor-writing-reflow-193.md)
và Dialog [#208](dialog-anatomy-r1-208.md). Không kế thừa PASS từ các PR đó.

Quyết định sản phẩm WR-01/02/05, UX-01 và INV-08 giữ nguyên; thiết kế này là
`DESIGN_PROPOSED`, mức triển khai `web`. Không signoff registry hoặc đổi luật lưu,
publication, navigation, autosave/storage. Ảnh concept là candidate;
`NO_APPROVED_REFERENCE`. Lịch sử [R5](ui-editor-r5.md) vẫn có hiệu lực ngoài delta này.

## LayoutSpec — Paper First r1

Đã trình bày spec trước sửa source. Người viết tiếp tục soạn trên một mặt giấy yên;
chữ là trọng tâm, Lưu nổi bật khi cần và recovery đọc được theo facts.

```text
Screen #editor — /ban-viet/{id}, dữ liệu nháp riêng từ API hiện hữu
  Region #context — link thật ← Bàn viết + H1 Bản nháp, ngoài Suspense
  Form #writing — một form, thứ tự DOM cũng là thứ tự Tab
    Row #editor-toolbar — không card, không sticky, wrap theo nội dung
      Region #save-slot — sole live readout + save/retry/recovery theo #217
      Action #preview — trực tiếp; Standard transparent, giữ tên Xem trước
      Disclosure #editor-save-details — summary Thêm luôn hiển thị
        Action #export — Tải bản thảo (.txt), type=button
        Text #save-detail — chi tiết trạng thái máy chủ, không live region mới
    Region #recovery — warning + retry/overwrite/export trực tiếp, khi có lỗi
    Region #paper — native title + Xem trọn tiêu đề + một BodyEditor + help
    Region #work-info — giữ selector và recovery thẻ hiện hữu
    Region #publication — giữ consumer và facts hiện hữu
```

Hierarchy: clean chỉ `Đã lưu`; dirty `Lưu` Filled; retry/recovery dùng mức nhấn theo
projection hiện hữu. Utility và summary hòa canvas, state layer/focus vẫn rõ.
Canvas trong Editor dùng `surface-container-low`; paper dùng `surface`, màu chữ từ
semantic roles hiện hữu. Không viền/shadow/gradient mới, không đổi font/measure/gutter.
Dark không có giả định paper luôn sáng hơn canvas.

Responsive: flex wrap theo nội dung (CHOICE-02 A), preview giữ trực tiếp ở mọi width;
status giữ flex-basis `7rem` hiện hữu (không dự trữ thêm một cột trống ở text200).
200% có thể nhiều hàng. Thêm mở trong flow, lấy cả hàng để nội dung không chen save.
Document scroll duy nhất; không dock/sticky ở desktop, narrow, short hoặc IME.
Back/H1 vẫn có khi loading, lỗi và no-JS; không dựng action/H1 dự phòng trùng.

Interaction: CHOICE-01 B — native `details/summary`, không `role=menu`, không focus trap.
Enter/Space thu mở, Tab/Shift+Tab tới nút bình thường; Escape giữ native disclosure mở,
không giả làm dismiss overlay. Click ngoài không tự đóng. Preview là trigger riêng,
đóng tools trước khi mở Dialog; Dialog trả focus về Preview sống. Export đọc live
buffer lúc bấm, kể cả đã sửa trong khi tools mở; `.txt` chỉ chứa chữ, không metadata.
Không thao tác nào chờ animation. Native disclosure không thêm chuyển động; reduced
motion và forced colors giữ contract chung.

CHOICE-04 B — `#editor-save-details > summary` chính là trigger Thêm luôn hiển thị,
không còn summary chi tiết nằm trong overflow đóng. `sync_save_slot_focus` giữ nguyên:
chỉ handoff khi focused Save đã bị gỡ và focus chưa chuyển đi; clean tới Thêm,
recovery tới `#save-recovery-jump`. Không tự mở tools, không focus BODY/hidden/inert,
không cướp focus title/CM, không đưa ACK đã disposed sang draft khác.

CHOICE-03 B — surface swap scoped Editor; `EditorToolbar.paper_first` là opt-in,
default consumers giữ anatomy/surface cũ. `utilities: ChildrenFn` cung cấp action
trong native disclosure. Không blanket đổi Text→Tonal, không đổi shared default.

Invariants: một H1, một EditorView, một polite status owner; target hiện có 48 CSS px
không bị thu nhỏ; status text ≥4.5:1; không tràn ngang ở 320/390/834/1440 và text100/200.
Useful writing width/title font giữ baseline #193; field/CM identity, raw NFC/NFD,
newlines/indent, selection/undo không đổi qua layout/theme/scale. Recovery và full
title luôn có đường tới; không bắt mọi thứ nằm trên màn đầu.

## Owners và kiểm chứng

- `apps/vink-web/src/pages/desk.rs`: compose tools/export; đóng tools trước preview.
  `DraftSession`, `project_save_slot`, save protocol và `LeaveGuard` giữ nguyên.
- `packages/vink-patterns/leptos/{src/lib.rs,patterns.css}`: opt-in anatomy/surface,
  native disclosure; preview `screen.editor` là sample, không backend/CodeMirror thật.
- `apps/vink-web/style/vink.css`: scoped canvas/paper, context spacing và document flow.
- `apps/vink-web/e2e/editor-paper-first-smoke.mjs`: actual app text preference matrix,
  native tools, live export/preview, negative hidden-summary oracle.
- `browser-smoke.mjs` / `editor-writing-smoke.mjs`: cập nhật focus/Tab/export anatomy;
  giữ các save races, partial chữ/thẻ, recovery, leave và native title/CM regression.
- `packages/vink-patterns/leptos/tests/editor_tools.rs`: SSR anatomy/single live owner,
  utility type và default consumer compatibility.

Presentation không đổi state transition/domain model; không tạo Quint/registry song song.
WorkInfo/PublicationPanel không đổi code. Các kiểm bản thân publication không là claim
quyền công khai mới. Ma trận DOM scaling/zoom khác ma trận chọn cài đặt app; báo riêng.

Bằng chứng local, paired captures đã mở và bảng năm chiều được ghi ở
[bàn giao #218](../development/evidence/editor-paper-first-218/README.md).
OS IME, screen reader và mobile device chỉ được báo PASS khi thật sự chạy trên host đó.
