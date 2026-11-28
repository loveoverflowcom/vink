# Runtime CodeMirror dùng chung của Vink

Gói TypeScript riêng cho vùng soạn nội dung trong `/ban-viet/{id}`. Đây không phải ứng dụng web
thứ hai: Leptos vẫn sở hữu trang, trạng thái lưu và giao tiếp với máy chủ.

| Phần | Owner |
| --- | --- |
| `EditorView`, lịch sử undo/redo, keymap, IME, sự kiện thay đổi | [src/editor.ts](src/editor.ts) |
| Cầu JS mà wasm-bindgen nạp cùng `vink.js`; tải runtime khi editor mount | [bridge.js](bridge.js) |
| `EditorHandle`, `BodySource::snapshot`, mount/teardown có kiểm owner | [src/editor.rs](../src/editor.rs) |
| Dirty/saving/saved/conflict, ticket version, ý định rời trang | [src/desk_state.rs](../src/desk_state.rs) (`EditorSession`, `LeaveGuard`) |
| Chữ nguồn ↔ document v1 | [src/document.rs](../src/document.rs) |
| Màu, chữ, khoảng cách của editor | `.vk-editor-body` trong [vink.css](../style/vink.css); rule `.cm-*` không layer trong [web.css](../../../packages/ui-system/leptos/tailwind/web.css) |

CodeMirror sửa **chữ nguồn**: đoạn văn và câu thơ ngăn bằng xuống dòng, khổ ngăn bằng dòng trống.
Với bản Markdown, runtime chỉ trang trí Live Preview và đọc mục lục ([markdown.ts](src/markdown.ts));
không sinh HTML và không đổi contract document v1. Enter chỉ xuống
dòng (không tự chép thụt lề); Tab để chuyển focus. Mỗi bản nháp đang mở có đúng một `EditorView`;
đổi theme là thay CSS variables; dialog, thông báo lưu và đổi định dạng (`applyConvertedDocument`)
không remount view.

Lưu, xem trước đổi định dạng và guard rời trang đều đọc `BodySource::snapshot()`, tức
`view.state.doc.toString()` tại thời điểm đó. Ctrl/⌘+S chờ IME commit xong.

## Focus và selection

Focus từ rail, overlay hoặc phím Tab dùng lại `EditorView.focus()` ngay trong focus handler
của runtime để khôi phục selection từ state trước khi trình duyệt đọc lại caret tạm ở đầu
`contenteditable`. Không tạo view mới, sửa nội dung hay ghi thêm lịch sử undo. Browser smoke
kiểm cả Tab và native blur/focus sau đổi theme/rail; đây vẫn là input mô phỏng trên Chrome.

Khi dialog đóng trong lúc resize, fallback focus vào shell có thể đưa DOM caret
vào đầu editor đã blur. Cầu giữ selection cuối của editor đang focus, map qua
document changes và khôi phục khi focus trở lại; caret phát sinh khi blur không
ghi đè lựa chọn này. Regression `shellWithDirtyEditor` giữ nguyên kiểm selection
sau mobile resize, EditorView/text/undo và không phát draft writes.
Giữ vùng chọn qua frame đầu sau focus rồi khôi phục ở frame kế tiếp, vì Chrome có thể
gửi sự kiện đổi caret của hộp thoại sau frame đầu;
keydown, mousedown, document edit, lần focus mới hoặc destroy hủy callback cũ.
Keydown/mousedown là `domEventObservers`, không phải handler: CodeMirror dừng ở handler
đầu tiên nhận sự kiện, và keymap nhận End, mũi tên, Mod-a, undo… trước. Khi còn là handler,
End bấm trong hai frame đó (CI Linux, ~20 ms sau khi hộp thoại rời trang đóng) bị lần khôi
phục chọn lại cả dòng, rồi chữ gõ tiếp thay mất dòng ấy. Smoke giữ animation frame từ lúc
focus trở lại tới khi phím vào editor, nên kiểm được cửa sổ này trên mọi nền tảng.
Chọn vị trí bằng chuột tiếp tục dùng hành vi CodeMirror hiện có.

## Preset, định dạng và vòng đời (#617)

Cả hai preset dùng cùng `createEditor(host, options)`, history, keymap, grammar CommonMark và cơ chế giữ vùng chọn qua focus. Mỗi host nhận một handle. `preset` chọn bề mặt. `format` chọn cách hiển thị chữ nguồn. Adapter Bàn viết ánh xạ DTO `default` sang `plain` ngay tại biên Rust. Reader dùng TextArea của #616 ngay khi mở. Nút Định dạng kích hoạt preset Comment cho nháp mới.

| Khả năng | `document` | `comment` |
| --- | --- | --- |
| Plain hoặc Markdown, Enter xuống dòng, Tab chuyển focus, undo/redo | Có | Có |
| Live Preview B/I/link, quote và marker theo caret trong Markdown | Có | Có |
| Heading trang giấy, projection H1, lịch mục lục và nhảy đến heading | Có, projection khi có subscriber | Giữ ký hiệu heading như chữ nguồn |
| Lệnh toolbar B/I/link | Khi Markdown, có quyền sửa và IME đã commit | Cùng điều kiện |
| Chuyển format sau khi máy chủ xác nhận | Trên cùng view | Lệnh bị chặn |
| Hình học | Giấy Bàn viết, chiều cao nội dung tối thiểu 50vh | Chữ UI, tối thiểu hai dòng, padding riêng |

Rust gọi `editor::bridge::EditorHandle::load().await` khi cần nhập. Owner kiểm tra lại chính host và trạng thái còn sống **sau await**, rồi gọi `mount(host, MountOptions { preset, format, text, labelled_by, described_by, read_only, on_change, on_composition, on_outline })`. Mount giữ nguyên focus. Các ID nhãn và mô tả do caller cấp. Lần load đồng thời dùng chung promise của bridge.

- `on_change()` báo source đã đổi. Đọc `handle.text()` tại thời điểm cần snapshot. Gõ một ký tự trong Bàn viết không buộc Rust sao chép toàn bộ tài liệu qua WASM.
- `on_composition(true|false)` báo cả khi source giữ nguyên. Bàn viết theo dõi cả form vì tiêu đề và thẻ cũng nhập bằng IME. Caller compact có thể dùng callback này để giữ trạng thái soạn của đúng composer.
- `on_outline(json)` dành cho Document. Snapshot có generation để từ chối mục lục cũ. Comment không tạo owner mục lục. Preset chỉ điều khiển runtime, quyền submit và lưu vẫn thuộc caller.
- `read_only` ở lúc mount áp dụng cho nội dung DOM và lệnh sửa. Khi owner bị thay, thả handle. Rust gọi destroy trước khi nhả closures. JS hủy frame khôi phục selection, frame cuộn caret, timer sau composition và frame mục lục.
- Adapter Discussion sở hữu fallback TextArea, thời điểm nâng cấp, draft theo ComposerKey, quote/list actions, send/retry, format lưu trữ và render nhận xét. Khi dùng kết quả load trễ, caller phải kiểm lại quyền, selection, focus và IME của đúng composer trước khi nâng cấp.

Chạy corpus bằng `node apps/vink-web/editor/scripts/build.mjs --check`. Với Chrome và PostgreSQL scratch, `cargo xtask verify web --scenario editor-format --release` đo trên route Product và fixture runtime. `desk-core-budget.json` giữ mẫu cold/warm, typing/caret, khoảng cách frame, long task và 40 vòng teardown. `editor-markdown-p95.json` giữ các cỡ corpus 10k–200k scalar.

## Build

Các lệnh build, dev và verify Web kiểm manifest/lock, typecheck TypeScript và chạy corpus
Markdown ở mỗi lượt. Khi cần tạo lại `vink-editor.js`, esbuild bundle vào staging. Xem
[đầu vào và điều kiện tái dùng](../../../docs/development/scoped-workflows.md#build-package-web).
Dependency dùng phiên bản pin chính xác. Cập nhật dependency bằng lệnh sau.

```bash
npm install --prefix apps/vink-web/editor --ignore-scripts
```

Bàn viết tải runtime khi mount. Reader chỉ tải theo hành động soạn có định dạng. Vink chưa phát Content-Security-Policy; runtime là file cùng origin, không có inline script
hay `eval`. CodeMirror chèn style bằng constructable stylesheet — khi chọn CSP, cần cho phép điều đó
hoặc truyền nonce qua `EditorView.cspNonce`.

Kiến trúc tách runtime/cầu/handle học từ `editor-runtime` của VOT; không mang worker ngữ nghĩa,
Markdown, outline hay autosave của VOT sang.
