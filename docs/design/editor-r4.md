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

PR 1 ([#482](https://github.com/loveoverflowcom/vink/pull/482)) xóa phần chỉ phục vụ hộp thoại xem
trước. PR 2 rà lại caller trên `develop@9e2eac57` (sau #482 và các thay đổi tới #508), gồm cả
feature `browser`, Preview, bridge JS và oracle browser.

| Ứng viên | Quyết định | Caller trước / hiện tại và lý do |
| --- | --- | --- |
| `#draft-preview`, `#preview`, `#preview-close`, `open_preview`, `.vk-preview`, `status_text` (`desk.rs`, `vink.css`) | `REMOVE` (#482) | Chỉ phục vụ modal xem trước bản nháp độc lập đã bỏ theo delta 24/11. Browser smoke giữ oracle vắng `#preview`/`#draft-preview` trong SSR, DOM và AX. |
| `.vk-editor-recovery .vk-ui-supporting-details*`, `.vk-editor-heading__title`, `.vk-editor-work-info`, `.vk-editor-format`, nút mẫu “Xem trước” của `screen.editor` | `REMOVE` (#482) | Không còn caller sau khi gom vào `#work-options`. |
| Override `@layer legacy` đấu với `@layer utilities` (`vink.css`, `patterns.css`, `tags.rs`, `work_info.rs`) | `CONSOLIDATE` (#482) | Utility theo prop/owner; `.vk-tag-query` + `[hidden]` cùng `@layer legacy` trong `patterns.css`. |
| `EditorToolbar`: prop `paper_first`, `compact_details`, `utilities`, `children`; nhánh mặc định (`SupportingDetails`, nền container) và summary “Thêm” | `REMOVE` | Ngoài test, chỉ Editor (`desk.rs`) và specimen `screen.editor` gọi pattern, cả hai đã dùng `paper_first=true compact_details=true`. Pattern còn một anatomy R4: readout, `save_action` và `<details id="editor-save-details">` “Chi tiết lưu”; `status_detail` bắt buộc. Xóa CSS nền/padding `.vk-editor-toolbar`, `--paper-first`, `__actions`, `__status > details`, `.vk-ui-supporting-details__content` và icon xoay của summary. `editor_tools.rs` đổi sang oracle R4. CMP `EditorToolbar` có chữ ký riêng, không đổi. |
| `BodySource::set_format`, `EditorHandle::set_format`, extern và bridge `setFormat`, `setFormat` trong `editor.ts` (đều `#[allow(dead_code)]` hoặc không caller) | `REMOVE` | Từ #262, đổi định dạng chỉ đi qua `applyConvertedDocument` sau khi máy chủ xác nhận. |
| `document::has_text` | `REMOVE` | Caller duy nhất là `#draft-preview` (gỡ ở #482); Reader và kiểm duyệt không dùng. |
| `DraftSession::needs_manual_save`, `TagSession::revert` | `RETAIN` dưới `#[cfg(test)]` | Không control nào gọi từ khi tạo (#217, #99). Test dùng chúng làm oracle cho `manual_save_parts` và để replay đổi lựa chọn khi đang lưu; editor gọi `begin_manual_save`. |
| `stickyAboveId`, `TOOLBAR_ID`, `EditorView.scrollMargins` và khoảng tránh toolbar sticky (`editor.ts`, `editor.rs`) | `REMOVE` | Từ #218 thanh lưu nằm trong luồng tài liệu (`position: static`, browser smoke kiểm). Nhánh chỉ chạy khi toolbar `sticky` nên không còn reachable. `ensureCaretVisible` giữ kiểm theo mép viewport. |
| `.vk-save-status[data-status="dirty"]`, `[data-status="pending"]` (`vink.css`) | `REMOVE` | Không có producer. App phát `saved/unsaved/saving/failed/conflict/loading`; Preview đổi `dirty → unsaved`, `pending → saving`. |
| `margin-top` của `.vk-notice__actions`, `.vk-desk--editor .vk-notice__actions`, hai rule `margin: 0` ở recovery thẻ/Công khai | `CONSOLIDATE` | Mọi `.vk-notice__actions` nằm trong `.vk-editor-recovery` của `WorkInfo`/`PublicationPanel`, margin tính ra luôn bằng 0. Giữ một rule flex/gap. |
| `.vk-editor-paper .vk-editor-body`, `.vk-editor-paper … .cm-content` và padding `@media (max-width: 30rem)` (`web.css`) | `CONSOLIDATE` | `BodyEditor` chỉ có caller trong `.vk-editor-paper`. Rule của giấy luôn thắng nên gộp giá trị vào rule gốc và xóa override chết. |
| `.vk-editor-topbar .vk-draft-action-icon` 18px, `justify-content` lặp ở summary, `.vk-save-slot`/`.vk-save-slot__readout { min-width: 0 }`, lớp `vk-editor-toolbar__status` | `REMOVE` | Trùng token `--vk-size-draft-action-icon` (18px), rule trong `patterns.css` hoặc utility `min-w-0` trên chính phần tử; lớp trạng thái không còn CSS hay test dùng (`#save-slot`/`.vk-save-slot` là hook). |
| Chuỗi lớp recovery `vk-editor-recovery [&>.vk-ui-notice]…` (5 nơi) | `CONSOLIDATE` | Hằng `work_info::RECOVERY_CLASS` cho thẻ và Công khai. |
| Builder `SaveDraftRequest` lặp ở lưu và đổi định dạng, kiểm reconcile sau lỗi, đuôi xử lý ACK chữ/thẻ, `status_detail`/`save_is_uncertain`, ba helper test `draft_status_*`, ba SVG nút Gửi/Lưu, focus `prevent_scroll` (`desk.rs`, `publication.rs`) | `CONSOLIDATE` | Dùng `save_request`, `reconcile_after_failure`, `after_ack`, `text_detail` với `tag_state::uncertain`, `SaveSlotAction::save_or_send`, `DraftActionIcon::paths` và `shell::focus_without_scroll`. Chuỗi hiển thị, payload và thứ tự ACK → focus → rời trang giữ nguyên. Test đơn vị đọc `slot_view`/`text_detail`, đúng chuỗi UI hiển thị. |
| `TagSelector` non-compact, `SupportingDetails`, `ReadingDocument`, `IconName::Visibility` | `RETAIN` | `TagSelector` mặc định là pattern TAG-PR-05 dùng chung với CMP (`Tags.kt`, `TagSpecimens.kt`) và có oracle `composition.tag-selector` trong `preview-smoke.mjs`; App dùng `compact=true`. `SupportingDetails` còn Preview/CMP, `ReadingDocument` phục vụ Reader và kiểm duyệt, `Visibility` phục vụ `AuthorWorkRow`. |
| `.vk-editor-heading__back`, `{view! {…}.into_any()}` quanh `WorkInfo`/`PublicationPanel`, `cfg_attr(not(feature = "browser"), allow(dead_code))` của `QUERY_ID`/`TagLoad` | `RETAIN` | Nút Bàn viết là control R4 mang ngữ nghĩa link trong forced colors (`LinkText`) và padding có icon đứng trước. Đổi sang tonal button chung là đổi giao diện, cần qualification riêng. Type erasure giữ độ sâu kiểu dưới giới hạn đệ quy (#482). Hai mục cfg chỉ có caller trong code trình duyệt. |

## Kiểm chứng và phần tiếp theo

Ảnh bàn giao thuộc preview. Kết quả PR 1 nằm trong [#482](https://github.com/loveoverflowcom/vink/pull/482).
Kết quả PR 2 ghi trong PR của #476. `VDR-EDITOR-R4@r1` giữ trạng thái `PENDING` cho tới khi có
signoff đúng revision.
