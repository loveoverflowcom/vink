# UI-REPAIR-09 — Rail riêng và scaffold theo frame

Ngày 03/11/2026. Nhánh `codex/ui-navigation-specimen-fit`, nền `origin/develop`
`2829cea` sau PR #51/UI-REPAIR-08. Phạm vi UX-01 là qualification preview và reflow
nhãn rail; không thay NavLayout policy, route, key preference, dirty guard hay API.
Không triển khai native product navigation.

## Xác nhận ở HEAD trước patch

| Finding | Source và tái hiện | Kết luận |
| --- | --- | --- |
| F09-01 | Chrome 156.0.8078.73/Linux, viewport390, text200: frame304.8, rail220, main82.8 CSS px; đã mở raw và focus tile. Cột trang tách gần từng ký tự | CONFIRMED |
| F09-02 | `Frame::default` chọn Expanded, `navigation.rail` gọi `shell` dựng AdaptiveScaffold với quiet page; container hẹp không đổi class cố định | CONFIRMED |
| Counterpart CMP | `Shell` dùng window cố định, rail specimen dựng cả scaffold; baseline common/Desktop verify PASS, giữ capture off-screen riêng | SOURCE_CONFIRMED; không suy từ CSS px sang dp |
| App thật | Owner `AdaptiveShell` khác recipe; không suy lỗi breakpoint production từ finding preview | Không đổi policy app; lane web kiểm regression |

Nguồn lịch sử: [UI-REPAIR-06/07](ui-repair-06-07.md), PR #48; chúng không là ảnh
baseline của run này. Baseline được chụp lại trên `2829cea`, fixture catalog synthetic.
Regression mới thất bại trên bundle trước sửa ở assertion rail-only không có
`nav-main`/`nav-shell`; không dùng ảnh cũ hay probe HTML độc lập để chứng minh fix.

## LayoutSpec đã trình bày trước patch

```text
#navigation-rail-specimen
  mô tả variant + route/metadata ngoài rail
  rail độc lập: default220, compact96, giữ toggle và chân Tôi
#navigation-scaffold-specimen
  mô tả requested/effective/frame + đơn vị
  fixed: canvas đủ rộng trong vùng cuộn ngang nhận focus
  adaptive / adaptive-editor: đo frame → policy hiện có
  page/input và overlay: giữ instance, state ngoài quyết định layout
#capture
  raw document-capped + focus/tile nguyên scale, dùng helper hiện có
  ghi source fingerprint, font, theme, DPR, frame và mode
```

Các scenario cũ giữ ID/mode, gắn nhãn ép cố định. Canvas Web tối thiểu 760/1100 CSS
px và CMP 600/840 dp theo owner WindowClass; không thu nhỏ UI để nhét frame.
`adaptive`/`adaptive-editor` là hai scenario mới, catalog sinh lại bằng
`cargo xtask ui-tokens`. `default` của rail vẫn 220. Không remount input để đổi mode.

Baseline xác nhận thêm rail220/text200 có thể tách từ “Khám”. CSS primitive reflow
icon lên trên nhãn khi bề rộng theo rem không đủ, giữ 220/cỡ chữ. CMP không áp query
CSS vì padding và constraints dùng dp. Stress `long-labels` ở rail 96 vẫn có nhãn
hạn chế rõ và toggle mở 220; không gọi đó là PASS usability đầy đủ.

Observer frame có cleanup; hidden/zero-width mount bắt đầu Compact rồi đo lại khi
hiện. Dialog adaptive giữ mở qua resize; đóng khi trigger bị ẩn trả focus main
với preventScroll. Đây là host mẫu, không đổi policy overlay của app production.

## Bằng chứng và kết quả

Tất cả bốn lane local PASS trên source UI cuối, tích hợp `develop@db90751`, tested
head `fef2a540`. Commit evidence tiếp theo chỉ thêm tài liệu/ảnh/metadata; CI được
kiểm lại theo PR head cuối. Source fingerprint (kể cả capture dirty trên base cũ)
được so từng file với source UI cuối trong [manifest](evidence/ui-repair-09/manifest.json).

| Boundary | Lệnh và kết quả |
| --- | --- |
| Repository | `cargo xtask verify quick`: PASS, fmt/clippy SSR/WASM/preview, doctest, unit/contract, link/metadata/asset guards |
| Preview Web | `VINK_UI_FOCUS_CAPTURE=1 cargo xtask ui-preview leptos verify`: PASS; 662 raw captures, 36 focus records; matrix/word/state/observer assertions |
| App thật | `cargo xtask verify web`: PASS main + after-restart trên PostgreSQL 17.11 tạm riêng cổng55439; navigation, dirty editor, save/conflict, restart và export |
| CMP | `cargo xtask ui-preview cmp verify`: PASS; common/Desktop và captures; 10/10 navigation tests, font tests3/3 |
| Native runtime | NOT_RUN: adb không có device/emulator, Linux không có Xcode/iOS host. Chỉ đổi common diagnostic recipes, không đổi platform host/lifecycle; Desktop không là native acceptance |

Chrome156.0.8078.73 headless Linux, DPR1, root font100/200% (không phải zoom).
Font Be Vietnam Pro/Literata được browser kiểm sau `fonts.ready`; CMP giữ nguồn
font portable của PR #51. Fixture là mẫu catalog và database thử, không có tài khoản
hay bản thảo production. Lịch sử FAIL và lệnh cuối:
[check-results](evidence/ui-repair-09/check-results.txt).

Các vấn đề phát hiện khi kiểm và cách xử lý:

- Baseline negative control FAIL ở assertion standalone rail; lỗi cũ không bị test bỏ qua.
- Build Rust/Kotlin đầu có lỗi compile đã sửa; không đổi pins hay compiler để vượt gate.
- Assertion programmatic focus vào main đòi scrollLeft>0 là oracle sai khi main còn
  hiện một phần. Đã thay bằng focus vùng cuộn + phím ArrowRight thật.
- CMP pointer trong Workbench có thêm ancestor cuộn ngang. Test phải cuộn `nav-frame`
  vào viewport dọc trước khi bấm; vẫn dùng pointer thật, không thay bằng semantics-only click.
- Probe public primitive độc lập xác nhận TextField đang focus co selection3..9 về
  caret9 khi blur, cả với nút không đổi layout; callback toggle chỉ chạy sau đó.
  Test resize giữ range3..9; pointer toggle giữ chữ và caret9 theo semantics blur đã có;
  test fixture không focus vẫn giữ range5..12. Không gọi focused blur là giữ nguyên range.
  [Source probe](evidence/ui-repair-09/PublicNavigationSelectionProbe.kt),
  [log](evidence/ui-repair-09/cmp-public-selection-probe.log),
  [JUnit](evidence/ui-repair-09/cmp-public-selection-probe.xml).
- Lane app từng timeout ở dashboard/retry. Hit-test chứng minh CodeMirror có queued
  selection scroll sau input, đưa target ra ngoài viewport. Helper lấy lại node và
  kiểm hit area có giới hạn trước khi dispatch đúng một click; không replay mutation,
  không giảm assertion. Main/restart cuối PASS.
- Review độc lập phát hiện fallback focus khi rail trigger bị ẩn và panel ghi chú rỗng.
  Đã sửa host `on:close` trả focus main với preventScroll và chỉ dựng note có nội dung.

## Đối chiếu ảnh cùng fixture

Raw document-capped vẫn được giữ và mở: ở390/text200 nó thường chỉ chứa catalog phía
trên, không chứng minh usability của specimen bên dưới. Focus tiles giữ scale và
đầy đủ bounds; không tăng6000px hay crop để che failure. Manifest đánh dấu inspected
trên từng file, không coi cả bulk đã được mở.

| Vùng, light390/text200 | Trước | Sau |
| --- | --- | --- |
| Rail/default | [raw](evidence/ui-repair-09/before--navigation.rail--default--light--390--text200.png) · [focus rail/cột hẹp](evidence/ui-repair-09/before--navigation.rail--default--light--390--text200--focus--part3.png) | [raw](evidence/ui-repair-09/after--navigation.rail--default--light--390--text200.png) · [focus rail riêng](evidence/ui-repair-09/after--navigation.rail--default--light--390--text200--focus--part3.png) |
| Scaffold/default cố định | [raw](evidence/ui-repair-09/before--navigation.scaffold--default--light--390--text200.png) · [focus](evidence/ui-repair-09/before--navigation.scaffold--default--light--390--text200--focus--part4.png) | [raw](evidence/ui-repair-09/after--navigation.scaffold--default--light--390--text200.png) · [focus canvas cuộn](evidence/ui-repair-09/after--navigation.scaffold--default--light--390--text200--focus--part4.png) |
| Scaffold/adaptive mới | Không có counterpart trước | [focus page và bar](evidence/ui-repair-09/after--navigation.scaffold--adaptive--light--390--text200--focus--part4.png) |

| Recipe | Available frame | Canvas | Requested → effective | Main |
| --- | --- | --- | --- | --- |
| Before rail/default |304.8 CSS px |304.8 | Expanded → rail220 |82.8 |
| After rail/default |304.8 |304.8 | rail riêng → rail220 |không có cột trang |
| After scaffold/default |304.8 |1100 | Expanded cố định → rail220 |878, cuộn ngang |
| After scaffold/adaptive |305 (clientWidth nguyên px) |304.8 | adaptive → bar |302.8 |

Fixed canvas là khác biệt có chủ đích và có chỉ dẫn cuộn; adaptive là scenario mới,
không tráo nó vào default để làm ảnh trước/sau đẹp hơn. Dark và text100/200 dùng
cùng cây; long-labels96 giữ limitation đã ghi. Bulk/context được giữ riêng trong
artifact local, ảnh đại diện và hash ở repository. Không upload Drive hoặc deploy.

Giới hạn còn lại: không có OS IME, screen reader, Safari/Firefox, native device,
Android/iOS focus/Back/lifecycle evidence. Không hoàn tất UX-01 trên toàn sản phẩm
chỉ từ preview qualification này.
