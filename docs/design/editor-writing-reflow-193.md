# Mặt viết mobile và tiêu đề dài — #193

Phạm vi WR-01/WR-02 tại `/ban-viet/{id}`, kế thừa [R5](ui-editor-r5.md).
Owner: `apps/vink-web/src/pages/desk.rs` và `apps/vink-web/style/vink.css`.
Không thay shared field, CodeMirror, schema, normalization, API hay save authority.
Lựa chọn dưới đây là kỹ thuật cục bộ của #193; D-04@r1 vẫn `DESIGN_PROPOSED`,
mức triển khai `web`, không hoàn tất S-01 hoặc autosave.

## LayoutSpec và lựa chọn

```text
Heading / Quay lại
  → Toolbar
  → Recovery khi cần
  → Một paper
      → Native input “Tiêu đề”
      → Disclosure “Xem trọn tiêu đề”
          → Bản đọc có soft-wrap, lấy trực tiếp từ title
          → “Chỉnh tiêu đề” trả focus về input
      → Nội dung CodeMirror
  → Thông tin tác phẩm
```

**CHOICE-01 B:** giữ một native input và một `title: RwSignal<String>`. Bản đọc
không có state hoặc control nhập thứ hai, không phát save và không tạo newline.
Input giữ `id=draft-title`, `name=title`, `maxlength=200`, label, placeholder,
autocomplete, native selection/undo/composition. Người viết xem trọn title bằng
disclosure, quay lại chỉnh đúng vùng chọn qua nút “Chỉnh tiêu đề”; bản đọc tiếp
tục cập nhật trong khi gõ. Input vẫn cuộn ngang theo caret như trước.

Không chọn textarea vì xuống dòng/paste của control multiline khác input;
`TextArea` hiện cũng chưa có name/limit/input_ref/on_focus tương đương. Để đạt
parity sẽ cần adapter chỉnh sửa dữ liệu hoặc history ngoài phạm vi presentation.
Hướng B đổi thêm một thao tác xem title, nhưng giữ nguyên editing semantics.
Không phụ thuộc `field-sizing`, không có fallback auto-height cần chạy.

Disclosure dùng native details/summary: pointer, touch, Enter/Space; thu gọn giữ
focus ở summary. Nút chỉnh dùng focus native để hiện lại dòng nhập, không gọi
select/setSelectionRange hoặc ghi value. Escape của preview/leave tiếp tục dùng
guard hiện có và `prevent-scroll` để trả focus; điều hướng không focus trang cũ.
Title Enter ngoài composition vẫn submit; body Enter chỉ xuống dòng. Chỉ mở/đóng
disclosure, resize, theme hoặc text scale không đánh dấu dirty.

**CHOICE-02 B:** giữ gutter trang và inset paper nhưng chặn mỗi lớp ở
`min(var(--vk-space-2), 8px)` cho viewport dưới 48rem. Back link không còn negative
margin tại breakpoint này. Body/title có cùng trục, 8px trong paper dành chỗ cho
focus. Desktop giữ padding/measure gốc. Typography và hit targets không thu nhỏ.
Đây là giới hạn decoration có scope, không phải global rem-to-px override.

Toolbar vẫn ở flow trên màn hẹp/thấp; desktop sticky và caret clearance giữ owner
hiện hành. Disclosure mở làm dài document, dùng cuộn trang, không có overlay hay
animation. Reduced motion không thay state. Bàn phím ảo/thiết bị thật chưa được
chứng minh bởi viewport giả lập.

## Giới hạn dữ liệu

HTML maxlength đếm UTF-16 code units; server trim rồi NFC và đếm tối đa 200 Unicode
scalar. Ví dụ 200 emoji có 400 code units vẫn hợp lệ ở API nhưng vượt giới hạn
native input hiện hành. NFD có thể dài hơn NFC trước khi gửi. Bản sửa giữ nguyên
sự khác biệt này; không normalize giữa composition hoặc thay domain để ép test.
LF/CRLF, tab và separators phải phân biệt insertion/paste của native input với
validation API. Server từ chối ký tự cấm còn lại sau trim; UI không tự làm sạch mới.

## Baseline và kiểm chứng

Baseline build lại từ `a677b0d4b5c53104ae03961a2d24c73d1d9f2ec5`, sau fetch
`origin/develop` ngày 15/11/2026. R5 là nguồn cấu trúc; QA `09ebede` là mốc lịch sử,
không là style anchor được duyệt: `fidelity: NO_APPROVED_REFERENCE`.
Self-review, một vòng sửa UI. Các cặp dùng cùng title 200 ký tự, poem 8 khổ,
saved/version 2, font đã tải, DPR 1, light/dark, viewport cao 900px; neo title/body
bằng `scrollIntoView(block:start)`, ghi scroll thật riêng, không ép scrollY bằng nhau.

| Viewport | Chữ | Layout client width | Writing width trước → sau (CSS px) |
| --- | --- | --- | --- |
| 320 | 100% / 200% | 305 | 249 → 273 / 193 → 273 |
| 390 | 100% / 200% | 375 | 319 → 343 / 263 → 343 |
| 834 | 100% / 200% | 819 | 611 → 611 / 544.969 → 544.969 |
| 1440 | 100% / 200% | 1425 | 672 → 672 / 1114.625 → 1114.625 |

Scrollbar đo được 15px; mobile không có rail. Ở 320/200: main padding 32→8 mỗi
bên, paper 24→8, field và `.cm-line` padding ngang 0; tổng gain 80px. Title font
28/56px. Font thực của `.cm-scroller` là 17/34px ở mobile, 18.085/34.085px tại
834 và 19.6/35.6px tại 1440; host body có font 16/32px. Giữ mọi font như baseline.
Dung sai overflow/rect 1 CSS px để xử lý làm tròn; font 0.02px cho computed decimal.
Literata có glyph bounds vượt line box, nên kiểm glyph không chồng control lân cận
và không vượt chiều ngang; không coi line box là clipping ancestor.

Hồi quy [editor-writing-smoke.mjs](../../apps/vink-web/e2e/editor-writing-smoke.mjs)
chạy trong `editorChrome` và selected `main`: full-title reflow, name/label/limit,
identity, selection có hướng, raw `EditorView.state.doc`, no-write, NFD/undo/IME
mô phỏng, Enter, real API giới hạn/normalization, preview/export, focus/forced-colors,
accessibility tree. Tab order thêm summary trước body và giữ assertion focus body.
Các ca #191/#194 cùng save/reconcile/conflict/leave vẫn nằm trong runner.

Tái chạy theo [hướng dẫn](../development/README.md):

```sh
# Có App + PostgreSQL scratch + synthetic fixture; phase không tự dựng stack.
node apps/vink-web/e2e/browser-smoke.mjs --base <loopback-url> \
  --chrome <chrome-binary> --state <scratch-state-file> --phase editor-chrome
# Đặt VINK_TEST_DATABASE_URL và VINK_CHROME cho host hiện hành.
cargo xtask verify web --scenario main
```

OS IME, clipboard/Word/Docs paste thật, browser zoom, screen reader,
Safari/Firefox và thiết bị/bàn phím mobile thật là boundary riêng. CDP committed
insertion có LF/CRLF/tab/separators không chứng minh clipboard paste hoặc OS IME.

Local ngày 15/11/2026, macOS/Headless Chrome 157, App + PostgreSQL 17 scratch:
focused `editor-chrome` PASS; `verify web --scenario main` PASS trên source cuối,
gồm hồi quy #191/#194. Lượt đầu `verify quick` FAIL ở repository check do placeholder
URL có sẵn tại `xtask/tests/scoped_cli.rs:35`. Theo yêu cầu tiếp nối, fixture đổi sang
hậu tố `-local-only`, giữ marker và assertion phát hiện rò rỉ; không nới checker.
Chạy lại `cargo xtask verify quick` PASS toàn bộ; `cargo test --locked -p xtask
--test scoped_cli` PASS (1 test). Source UI không đổi sau lượt Web đã PASS.
Bundle local `target/ui-evidence/editor-writing-reflow-193/` giữ manifest, source
hashes, log cả các lần FAIL, paired PNG và báo cáo năm chiều; không commit binary.
