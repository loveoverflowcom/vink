# UI-REPAIR-05 — Chrome của editor và trạng thái lưu

Ngày 03/11/2026. Base `develop@0fc8c04b29a4a4de7c57b17ad34981949abfc865`
sau PR [#46](https://github.com/loveoverflowcom/vink/pull/46) (UI-REPAIR-04), nhánh
`codex/ui-editor-chrome`. Đóng góp vào WR-01/02/03/05 và UX-01; không hoàn tất
autosave, S-01, auth, publication hay native editor.

## Nguồn và bug contract

- [Current design](https://drive.google.com/file/d/1cNYQh-iukYP-nP0cphLOGdywROAmtkSr/view)
  giữ Bàn viết/editor hiện có và thứ tự điều hướng. Đã đọc file index hiện hành.
- Đã mở [ảnh editor dirty dark 390px được giao](https://drive.google.com/file/d/1DmMCd9BZUK3aH8-DVaoWNJt7Bii11-VB/view).
  Scale/build của ảnh chưa xác định; chỉ dùng để hiểu triệu chứng, không làm bằng
  chứng before/after cùng build.
- Trước sửa: toolbar nhóm ngang bằng `Stack`, trong khi CSS
  `.vk-editor-toolbar__actions` không có consumer DOM. Heading có selector riêng
  nhưng chưa gắn vào H1. Hướng dẫn ngắt dòng luôn chiếm chỗ trước nội dung. Preview
  dùng bề rộng dialog mặc định dù cần đọc cả bản thảo dài.
- Baseline local giữ ở `target/ui-repair-05/before/`: hai ảnh và manifest từ lượt
  kiểm PR 04 trên `15c9f41` (source tương ứng với merge base `0fc8c04`), gồm
  `390-editor-dirty-dark` và `1280-preview-long-title-dark`. Chỉ ảnh có cùng tên/
  fixture/viewport được đối chiếu trực tiếp sau sửa; ma trận mở rộng là kiểm sau sửa.

Mục tiêu: nhận ra trạng thái và hành động lưu, đọc/viết yên tĩnh, truy cập được
phục hồi và mọi nút ở 320px/chữ 200%, giữ nguyên chữ khi lỗi hoặc đổi giao diện.

## HOW / WHY và chủ nguồn

`DraftEditor` tải nháp bằng `LocalResource` trong browser; SSR chỉ có khung.
`Editor` giữ title signal, một `BodySource`/`EditorView`, `EditorSession` và
`LeaveGuard`. `EditorToolbar` trong `vink-patterns` chỉ nhận status/token và
children; shared `Button` có wrapper `.vk-ui-action`, nút và ghi chú trạng thái.

`begin_save` cấp ticket gắn edit/expected_version; `send` chụp title/body ngay
trong lượt đó. Chỉ ack của ticket được áp vào session; gõ tiếp trong lúc chờ vẫn
là chữ chưa lưu sau ack. Preview/export đọc `BodySource::snapshot`, không gọi lưu.
Dialog/rail/theme không remount editor. Guard điều hướng giữ URL, chữ và ý định
rời; ack muộn sau khi ở lại hoặc sang nháp khác không điều hướng.

Lý do lưu thủ công và CodeMirror/guard được ghi trong
[lựa chọn bootstrap](../development/README.md#lựa-chọn-bootstrap-và-điều-kiện-xem-xét-lại)
và commit `ac3fb75`: kiểm CAS/snapshot, giữ selection/undo/IME, chưa chốt autosave
hay rich text. Suy luận từ source: làm sạch chrome không cần sửa transaction
CodeMirror, document v1, server API hay PostgreSQL.

Finding `UI-REPAIR-05-UNCERTAIN-01` (P2): mất phản hồi PUT có thể xảy ra sau commit,
nhưng trước sửa cả nút retry/shortcut/lưu rồi rời đều gửi lại trực tiếp. Bản sửa
đọc GET trước **mọi lần lưu lại từ Failed**. Giữ nguyên expected_version; server
khác version đi vào conflict hiện có, không gửi PUT. GET lỗi vẫn giữ lỗi/chữ và
lần lưu lại sau đó vẫn phải GET. GET cùng version mới cho PUT có CAS; writer khác
chen giữa GET/PUT vẫn bị server chặn. Không suy GET trùng chữ là ack của ticket
cũ, không tự xác nhận saved hay tự retry/ghi đè. Caller điều phối GET/PUT qua
[API client hiện có](../../apps/vink-web/src/api.rs), không đổi state machine.
Trước PUT, caller kiểm ticket còn hiện hành và intent chưa được xác nhận bỏ;
GET đến sau khi editor kết thúc không khởi động mutation mới.

## LayoutSpec r1 — đã trình bày trong chat trước patch UI

```text
#editor — /ban-viet/{id}
  #heading — link Bàn viết + H1 Bản nháp
  #toolbar — EditorToolbar, status bằng chữ + chấm phụ
    #secondary-actions — Xem trước / Tải .txt (Text Button)
    #primary-action — Lưu nháp (Filled Button)
  #recovery — notice lỗi/xung đột + retry/giữ bản local/export hiện có
  #writing-surface
    #title-field — TextField, tối đa 200, font đọc và đường phân cách
    #body — nhãn Nội dung + shortcut ngắn + CodeMirror hiện có
    #writing-help — native details: Cách ngắt dòng và chia khổ
  #dialogs
    #draft-preview — snapshot đang viết, tiêu đề + thân bài + Đóng
    #leave-editor — Ở lại / Lưu và rời; phục hồi và xác nhận bỏ theo state
```

Toolbar dùng `.vk-editor-toolbar__actions` thật, wrap theo `.vk-ui-action`;
thứ tự DOM/focus status → preview → export → save → title → body. Lưu là Filled,
công cụ phụ là Text. Khung editor giới hạn bằng `measure + 2 × space-6`; body
giữ padding/font/leading và source, title có khoảng tách riêng. Hướng dẫn ngắt
dòng mở bằng Enter/Space trên `summary`; shortcut thiết yếu vẫn ở cạnh nhãn body.

Sticky chỉ khi container editor ≥44rem và viewport cao ≥40rem. Container query
theo root text scale giải phóng toolbar khi chữ lớn, khác media query `rem` vốn
dựa trên cỡ chữ ban đầu. Màn hẹp/cửa sổ thấp cuộn cả toolbar với trang, không
thêm fixed action. CodeMirror tiếp tục đo chiều cao sticky; title/button có
scroll margin. Toolbar pending giữ nhãn nút, cung trạng thái và ghi chú shared.

Preview rộng theo measure, co theo viewport; cả dialog cuộn được. Leave dialog
ở màn hẹp xếp target nguyên hàng theo wrapper thật của Button; nút an toàn trước
hành động mất chữ, bước xác nhận bỏ vẫn riêng. Giữ native modal, EscapeOnly,
focus title ở preview, focus lựa chọn an toàn và trả focus/cuộn hiện có.

Hai theme dùng cùng cây và token `surface-container-high/low`, `on-surface`,
`on-surface-variant`, `primary`, `error`, `warning`, `focus`, typography/space/radius
hiện có. Không sửa token chung, account/profile, dashboard, native hay recipe.
Notice lỗi/xung đột dùng Polite theo contract trạng thái; không tự cướp focus.
Chữ `Chưa xác nhận lưu` phân biệt outcome mơ hồ với lỗi từ chối chắc chắn.

## Kiểm chứng và evidence

Owner: [desk.rs](../../apps/vink-web/src/pages/desk.rs),
[vink.css](../../apps/vink-web/style/vink.css),
[EditorToolbar](../../packages/vink-patterns/leptos/src/lib.rs),
[patterns.css](../../packages/vink-patterns/leptos/patterns.css),
[browser-smoke.mjs](../../apps/vink-web/e2e/browser-smoke.mjs).

`editorChrome` dùng nháp thử thật, title 200 ký tự, thơ tám khổ, thêm dấu kết hợp
và gõ tiếp khi PUT đang chờ. Capture clean/dirty/saving/failed/conflict,
preview/leave tại 1440/1024/834/390/320 × light/dark × root text 100/200%.
Ảnh phục hồi và ảnh cuộn tới action tách riêng; manifest ghi route, viewport,
theme, root font, scroll và geometry editor. Root text scale không là browser zoom.

Assertions gồm reflow/target ≥44px, một EditorView, Tab tới công cụ/title/body,
status có chữ, sticky không che title, viewport mobile thấp không có fixed toolbar,
slow save + click/shortcut lặp chỉ một PUT, ack cũ không xác nhận chữ mới,
retry sau thất bại, mất ack sau commit, GET đối chiếu 403 rồi retry vẫn không PUT,
conflict giữ chữ/version, preview của chữ dirty, dialog cuộn tới nút an toàn bằng
Tab, explicit recovery + reload và Back/Forward lúc dirty. `mainPhase` và
`afterRestartPhase` giữ kiểm save/leave/discard/late ack, selection/undo/redo,
theme/rail, export unsaved và restart/persist hiện có.

Lịch sử chạy (giữ trạng thái runner, không đổi FAIL thành NOT_RUN):

| Lượt | Kết quả |
| --- | --- |
| `verify quick`, r1 | PASS; patch UI/API ban đầu, trước cập nhật test và copy cuối |
| `verify web`, r1 | FAIL prerequisites: Chrome không nằm trong PATH; các bước sau NOT_RUN. Chạy lại với `VINK_CHROME` trỏ binary đã cài |
| `verify web`, r2 | FAIL ở assertion focus preview: test cuộn về đầu rồi gọi focus vào chính nút đang focus; không tạo chuyển focus. Sửa test dùng title → Tab thật; các case editor trước đó PASS |
| `verify web`, r3 | FAIL khi đo chữ của nút giữa transition đổi theme; bổ sung chờ transition hữu hạn, không chờ vòng lặp pending |
| `verify web`, r4 | FAIL helper contrast chưa hỗ trợ nền alpha của control disabled; ghi rõ disabled được miễn ngưỡng, tiếp tục đo nút pending và mọi chữ đang hoạt động |
| `verify web`, r5 | PASS main + after-restart; trước bổ sung capture title/body chữ lớn, help và guard GET đến muộn sau discard |
| `ui-preview leptos verify`, r1 | PASS build/prerequisites/headless scenarios và captures; shared EditorToolbar dùng DOM mới, không overflow ở gallery |
| `verify quick`, final | PASS token drift, fmt, clippy SSR/WASM/preview, doctest, unit/contract và repository checks |
| `verify web`, final | PASS toàn bộ main + after-restart, gồm guard GET đến muộn sau discard; không console error/exception |

Môi trường cuối: Rust 1.99.1, Node 26.12.0, Chrome 156.0.8078.79 headless macOS,
PostgreSQL 17.11 Homebrew tại cluster thử riêng cổng 55435. Bản ghi
[kết quả và SHA-256 source đã kiểm](evidence/ui-repair-05/check-results.txt) giữ
summary runner, counterexamples và tương phản. Chữ đang hoạt động/pending tối
thiểu **5,03:1 light**, **7,57:1 dark**; control disabled được miễn ngưỡng và ghi
trong JSON. Không dùng màu giữa transition làm kết quả của theme đã ổn định.

Đã mở 33 contact sheet của **263 ảnh editor** trên patch cuối, cùng ảnh nguyên
kích thước của vùng đại diện và desktop gallery EditorToolbar. Ma trận mới có
thêm ảnh title/body ở 320/390px chữ 200%; notice dài và thao tác dưới phải cuộn
ở cỡ chữ đó. Đó là reflow có chủ đích, toolbar không fixed. Native focus đưa
nút an toàn vào viewport; không thu nhỏ chữ để nhét hết vào một ảnh.

Ảnh đại diện được lưu trong repo, có
[manifest viewport/theme/scroll/geometry](evidence/ui-repair-05/contexts.json):

| Vùng | Ảnh đã mở |
| --- | --- |
| Mobile dirty cùng fixture 390/dark/100% | [Trước](evidence/ui-repair-05/before-390-editor-dirty-dark.png) · [Sau](evidence/ui-repair-05/390-editor-dirty-dark.png) |
| Desktop clean, writing measure | [1440/light](evidence/ui-repair-05/editor-clean-1440-100-light.png) |
| Pending, lỗi và xung đột | [Saving 1024/dark](evidence/ui-repair-05/editor-saving-1024-100-dark.png) · [Failed 390/light](evidence/ui-repair-05/editor-failed-390-100-light.png) · [Conflict 320/dark](evidence/ui-repair-05/editor-conflict-320-100-dark.png) |
| Title/body chữ 200% | [Title 320/dark](evidence/ui-repair-05/editor-dirty-320-200-dark-title.png) · [Writing 320/dark](evidence/ui-repair-05/editor-dirty-320-200-dark-writing.png) |
| Dialog và focus khi cuộn | [Preview 320/200%](evidence/ui-repair-05/editor-preview-320-200-dark-actions.png) · [Ở lại 320/200%](evidence/ui-repair-05/editor-leave-320-200-light-actions.png) |
| Nội dung được focus | [Viewport thấp 390](evidence/ui-repair-05/editor-short-mobile-390.png) · [Title dưới sticky 1440](evidence/ui-repair-05/editor-sticky-title-1440.png) |

Đối chiếu LayoutSpec: các vùng, thứ tự, mức nhấn Lưu/công cụ phụ, recovery và
hai dialog khớp. Toolbar 1440/100% cao 64px ở clean/dirty, khoảng 86px khi pending
vì giữ ghi chú shared; 390/100% dirty cao 93px. Text field dài cuộn ngang bên
trong input, full title được giữ; body/preview giữ line/stanza và wrap mềm. Review
diff/caller và counterexamples không còn vấn đề cần sửa trong phạm
vi đã kiểm. Không đổi `desk_state.rs`, `editor.rs`, runtime TypeScript, schema hay
HTTP contract; chỉ caller thêm đối chiếu trước retry và kiểm ownership trước PUT.

Log ở `target/ui-repair-05/logs/`; ảnh gốc/manifest của lượt hiện hành ở
`target/verify-web/screenshots/`, contact sheets mở kiểm ở
`target/ui-repair-05/inspection/`. Lượt web kế tiếp làm mới thư mục verify-web.

NOT_RUN: IME hệ điều hành, bàn phím mềm và thiết bị Android/iOS thật,
Safari/Firefox, screen reader, paste Word/Docs. CDP composition và viewport thấp
chỉ là mô phỏng, không chứng minh các ranh giới đó. Không có auth/provider/AI
hay hành động xuất bản được thực hiện trong kiểm thử.
