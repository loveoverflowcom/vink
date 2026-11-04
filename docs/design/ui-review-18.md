# UI-REVIEW-18 — trả focus khi quay lại lựa chọn rời editor

Ngày 04/11/2026. Review PR #62 từ `1969dab` trên base `eb41d4b`;
phần editor giữ nguyên khi đồng bộ các sửa nhận xét/composer của #60–61.

Khi lưu lỗi, xung đột hoặc còn chờ, người viết mở “Rời mà không lưu…” rồi
“Quay lại”. Nút đang focus bị gỡ khỏi DOM khi đổi bước; trước sửa, focus rơi
khỏi lựa chọn trong dialog. Bản sửa trả focus sau khi các nút được render lại.

```text
Dialog #leave-editor
  Choosing — #leave-stay, #leave-save; recovery có #leave-export, #leave-discard
  ConfirmingDiscard — #discard-back, #discard-confirm
  Quay lại → Choosing → focus #leave-discard nếu còn kết nối DOM
                       → focus #leave-stay nếu ACK đã gỡ thao tác discard
```

Callback bỏ qua nếu dialog đã đóng hoặc guard không còn ở bước `Choosing`.
Không thay luật lưu, version/CAS, nội dung đang viết hoặc owner của dialog.
Màn hẹp/chữ lớn giữ cấu trúc hiện tại; focus cần cuộn nút vào vùng nhìn thấy.

Ảnh từ head gốc của PR đã được mở trong review: Chrome headless, PostgreSQL
cục bộ, viewport 320×900, theme tối, root text 200% (không phải browser zoom).

| Trước | Sau |
| --- | --- |
| [Mất focus khi quay lại](evidence/ui-review-18/before-return.png) | [Focus ở nút rời mà không lưu](evidence/ui-review-18/after-return.png) |

`editorChrome` trong [browser smoke](../../apps/vink-web/e2e/browser-smoke.mjs)
kiểm Enter để quay lại, vị trí nút được focus, Tab trong modal, Escape và fallback
về “Ở lại” khi ACK tới trong bước xác nhận. Kết quả chạy trên head cuối ghi trong
PR; các assertion và ảnh này không chứng minh screen reader, OS IME hoặc bàn phím
mềm/thiết bị thật.
