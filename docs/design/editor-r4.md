# Editor R4 · delta triển khai Web ngày 24/11/2026

Phạm vi [#475](https://github.com/loveoverflowcom/vink/issues/475), PR 1/2 của
[#428](https://github.com/loveoverflowcom/vink/issues/428). Nguồn thiết kế giữ nguyên theo
[LayoutSpec r4 ngày 23/11](https://docs.google.com/document/d/15iMZyQFaPRJ_0ExTnpdHe5zDAlav-OC-/edit)
và [bàn giao 13 ảnh, ZIP](https://github.com/loveoverflowcom/vink/issues/428#issuecomment-7103859904)
(`DESIGN_PROPOSED`, `NO_APPROVED_REFERENCE`). Báo cáo preview mô tả fixture riêng của thiết kế.
[VDR-EDITOR-R4@r1](../../decisions/proposals/VDR-EDITOR-R4.r1.toml) còn `PENDING` trong kho quyết định.

## Bố cục và nguồn dữ liệu

- Route `/ban-viet/{id}` dùng một EditorView và DraftSession. Thanh đầu trang giữ nút Bàn viết,
  tên màn, trạng thái lưu, một ô thao tác lưu và menu Chi tiết lưu. “Gửi” ở bản mới vẫn là lưu nháp
  theo projection hiện có. Gửi kiểm duyệt và chuyển riêng tư là thao tác xác nhận trong phần Công khai.
- Mặt giấy dùng `bg-content`, cột bên rộng 17rem và giấy tối đa 48rem. Khi nội dung còn dưới 50rem
  sau rail và lề, cùng bảng Tùy chọn bài viết xuống dưới giấy và gập. Disclosure native theo
  container query, hỗ trợ Enter/Space; co màn khi focus trong bảng chuyển focus về summary.
  Mục lục Markdown ở cùng cột phụ khi đủ chỗ và dùng sheet native khi hẹp.
- Dạng bài dùng select, giữ tối đa ba chủ đề và giữ ô tìm/chip đã chọn khi gập bảng. Định dạng giữ
  xem trước đích, Hủy và Áp dụng. Công tắc Công khai chỉ đổi ý định; trạng thái áp dụng và tên bản
  đang chờ duyệt lấy từ máy chủ, chỉ thay đổi qua hành động riêng hiện có. Bảng gập vẫn báo Chờ duyệt.
- Tác giả thử lưu lại tại thanh lưu. Xử lý xung đột chữ mở dialog có “Ở lại viết” và “Lưu đè bản
  của tôi”. Guard rời hai bước và các lựa chọn xử lý xung đột thẻ tiếp tục dùng các owner hiện có.
  Màu và cỡ chữ (100%/150%) chỉnh tại `/toi`; 200% dùng cho kiểm stress.

## Delta đối với nút con mắt trong r4

Bỏ nút con mắt và hộp thoại Xem trước bản nháp. Markdown Live Preview vẫn nằm ngay trong
EditorView. Dialog đổi định dạng vẫn xem trước kết quả chuyển. Reader và màn kiểm duyệt vẫn
render snapshot đúng quyền. Nháp riêng và snapshot kiểm duyệt giữ quy tắc không tính lượt xem
([PUB-01, VIEW-05](../../prd/requirements.md)).

## Kiểm kê dọn legacy (#476)

| Ứng viên | Quyết định | Caller trước / hiện tại và lý do |
| --- | --- | --- |
| `#draft-preview`, `#preview`, `#preview-close`, `open_preview`, `.vk-preview`, `status_text` (`desk.rs`, `vink.css`) | `REMOVE` | Chỉ phục vụ modal xem trước bản nháp độc lập đã bỏ theo delta 24/11; xóa sạch cả đoạn CSS mồ côi và import `SupportingDetails`/`TooltipIconButton` thừa trong `desk.rs`. |
| `.vk-editor-recovery .vk-ui-supporting-details*` (`vink.css`), `.vk-editor-heading__title`, `.vk-editor-work-info`, `.vk-editor-format` (`desk.rs`), `#[cfg_attr(not(test), allow(dead_code))]` trên `draft_status_*` | `REMOVE` | Không còn caller sau khi bỏ `#save-failure-details`/`#save-conflict-details` và gom vào `#work-options`; giữ `.vk-editor-recovery .vk-ui-button--text` cho recovery thẻ/Công khai và chuyển helper test sang `#[cfg(test)]`. |
| Nút mẫu `"Xem trước"` trong `screen.editor` (`packages/ui-system/leptos/preview/src/screens.rs`) | `REMOVE` | Specimen cũ của Paper First trước delta 24/11; chuyển sang `EditorToolbar` `compact_details=true`. |
| Override tầng `@layer legacy` đấu với `@layer utilities` (`vink.css`, `patterns.css`, `lib.rs`, `tags.rs`, `work_info.rs`) | `CONSOLIDATE` | Đưa `justify-start`/`flex-[0_1_auto]`/`p-vk-3` (`EditorToolbar`), `gap-vk-3` (`TagSelector`), `empty:hidden` (`WorkInfo`) vào utility theo prop/owner và đặt `.vk-tag-query` + `[hidden]` cùng `@layer legacy` trong `patterns.css`. |
| `EditorToolbar` (`paper_first`/default), `TagSelector` (non-compact), `SupportingDetails` (`vink-ui`), `ReadingDocument`, `IconName::Visibility` | `RETAIN` | Còn caller thực: `editor_tools.rs`, Leptos/CMP Preview (`preview-smoke.mjs` kiểm `TagSelector` mặc định), Reader (`pages/mod.rs`), kiểm duyệt (`pages/moderation.rs`), `AuthorWorkRow` và `TagSelector` browse. |

## Kiểm chứng và phần tiếp theo

Ảnh bàn giao thuộc preview. Trên head tích hợp sau rebase lên `develop@ef8a55b8`:
- `PASS`: `cargo xtask verify quick` (UI tokens, empty artwork, expressions, visual/perf oracles, `fmt`, Clippy server SSR / browser WASM hydrate / UI preview CSR, doctests, Reader SSR boundary, toàn bộ unit/contract tests gồm Tailwind compile và repository checks) trên commit fix và tái kiểm `fmt`, `git diff --check`, `ui-tokens --check`, `decision validate`, Clippy SSR/hydrate/preview cùng unit tests `vink-patterns` và `vink-web` trên commit #476.
- `NOT_RUN`: `cargo xtask verify web` (bỏ qua theo yêu cầu của người dùng trong phiên) và `cargo xtask verify preview`, cùng đối chiếu ảnh app thật ED-INV-01..06 và kiểm tra bằng chứng phát hành.
- `VDR-EDITOR-R4@r1` giữ trạng thái `PENDING` cho tới khi có signoff đúng revision.
