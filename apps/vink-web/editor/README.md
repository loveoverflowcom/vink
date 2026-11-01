# Runtime CodeMirror của Bàn viết

Gói TypeScript riêng cho vùng soạn nội dung trong `/ban-viet/{id}`. Đây không phải ứng dụng web
thứ hai: Leptos vẫn sở hữu trang, trạng thái lưu và giao tiếp với máy chủ.

| Phần | Owner |
| --- | --- |
| `EditorView`, lịch sử undo/redo, keymap, IME, sự kiện thay đổi | [src/editor.ts](src/editor.ts) |
| Cầu JS mà wasm-bindgen nạp cùng `vink.js`; tải runtime khi editor mount | [bridge.js](bridge.js) |
| `EditorHandle`, `BodySource::snapshot`, mount/teardown có kiểm owner | [src/editor.rs](../src/editor.rs) |
| Dirty/saving/saved/conflict, ticket version, ý định rời trang | [src/desk_state.rs](../src/desk_state.rs) (`EditorSession`, `LeaveGuard`) |
| Chữ nguồn ↔ document v1, export `.txt` | [src/document.rs](../src/document.rs) |
| Màu, chữ, khoảng cách của editor | `.vk-editor-body` trong [vink.css](../style/vink.css) |

CodeMirror sửa **chữ nguồn**: đoạn văn và câu thơ ngăn bằng xuống dòng, khổ ngăn bằng dòng trống.
Runtime không hiểu Markdown, không sinh HTML và không đổi contract document v1. Enter chỉ xuống
dòng (không tự chép thụt lề); Tab để chuyển focus. Mỗi bản nháp đang mở có đúng một `EditorView`;
đổi theme là thay CSS variables, preview/dialog/thông báo lưu không remount view.

Save, preview, export và guard rời trang đều đọc `BodySource::snapshot()`, tức
`view.state.doc.toString()` tại thời điểm đó. Ctrl/⌘+S chờ IME commit xong.

## Build

Focus từ rail, overlay hoặc phím Tab dùng lại `EditorView.focus()` ngay trong focus handler
của runtime để khôi phục selection từ state trước khi trình duyệt đọc lại caret tạm ở đầu
`contenteditable`. Không tạo view mới, sửa nội dung hay ghi thêm lịch sử undo. Browser smoke
kiểm cả Tab và native blur/focus sau đổi theme/rail; đây vẫn là input mô phỏng trên Chrome.

`cargo xtask build` (và `dev`, `verify web`) chạy `npm ci --ignore-scripts` khi
`package-lock.json` đổi so với lần cài trước, rồi `node scripts/build.mjs` để type-check bằng
TypeScript và bundle bằng esbuild vào `target/site/pkg/vink-editor.js`. Phiên bản package được pin
chính xác; `verify quick` kiểm điều đó và kiểm lockfile khớp `package.json`. Đổi dependency:

```bash
npm install --prefix apps/vink-web/editor --ignore-scripts
```

Runtime chỉ tải ở trang editor (trang đọc công khai không tải). Bundle hiện khoảng 270 KB, 88 KB
gzip. Vink chưa phát Content-Security-Policy; runtime là file cùng origin, không có inline script
hay `eval`. CodeMirror chèn style bằng constructable stylesheet — khi chọn CSP, cần cho phép điều đó
hoặc truyền nonce qua `EditorView.cspNonce`.

Kiến trúc tách runtime/cầu/handle học từ `editor-runtime` của VOT; không mang worker ngữ nghĩa,
Markdown, outline hay autosave của VOT sang.
