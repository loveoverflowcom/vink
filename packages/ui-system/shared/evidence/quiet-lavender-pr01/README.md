# PR-01 — Shared UI Controls · Quiet Lavender R1

Ngày kiểm local: 04/11/2026. Baseline `develop@33688b151af71f89f14ec1d8e02cc73efb5fd280`.
Source thiết kế: [LayoutSpec R2/R1](https://drive.google.com/file/d/1ChFchoWsrc9XFtkW5BvP5XUfr6GAAbKy/view),
[field/action HTML](https://drive.google.com/file/d/1HevBve7u9mJVuSGtASJeIAB6Ubr5uP5M/view),
[component board R2](https://drive.google.com/file/d/1Quf-d0yZX6id-Eh4eDeODcs5CrpFaEFb/view),
[issue #66](https://github.com/loveoverflowcom/vink/issues/66) và comments đọc mới trong phiên.
R2 chỉ là tham chiếu cho Appearance; PR này thực hiện R1 shared controls.

## Kết quả và contract

- `TextField` / `VinkTextField` có `FieldVariant::FloatingTonal` / `FieldVariant.FloatingTonal`;
  mặc định `ExternalLabel`. PR-02 có thể dùng variant mới cho title, giữ nguyên `id`, `name`,
  `max_length`, `input_ref`, `on_focus`, value và callback. Không đổi SearchField form contract.
- Native input không đổi identity khi nhãn di chuyển. Web floating shell có hai hàng intrinsic;
  helper/error có ID liên kết; textarea giữ nhãn ngoài. Disabled thiếu help có giải thích.
- Idle tonal, focus 2px, error 1px, error+focus 2px màu lỗi, một owner, không shadow vẽ viền.
  Chip tonal có check/pressed, slot check ổn định; Text button giữ geometry lúc pending.
- CHOICE-01: Material3 TextField của bản pin 1.8.2, tắt toàn bộ indicator và vẽ một border
  có điều kiện. Giữ `TextFieldValue`, composition, selection, native semantics/menu.
- CHOICE-02: shell sở hữu floating field; input sở hữu field nhãn ngoài/Search/Select/textarea.
  Trailing action có focus riêng; không dùng focus-within trên parent để thêm vòng.
- CHOICE-03: RadioGroup generic giữ native. Chỉ appearance cần icon/label/check; recipe thuộc PR-03.
- Không đổi token source 0.4.0; catalog 0.5.3 thêm `external-label`, outputs do generator sinh.

## Kiểm chứng local

Dùng Rust 1.99.1, Node 22+, Chrome trên macOS và JDK 24 / Gradle wrapper của repo.
Các lệnh Rust chạy với `CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0`
để vừa dung lượng máy. Preview Web đặt
`VINK_CHROME='/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'`.

| Lệnh / boundary | Kết quả |
| --- | --- |
| `cargo xtask ui-tokens --check` | PASS (cũng chạy trong quick và cả hai preview) |
| `cargo xtask verify quick` | PASS: fmt, clippy SSR/hydrate/CSR, doctest, unit/contract, repository guards |
| `cargo xtask ui-preview leptos verify` | PASS: 754 capture contexts, không console error; mọi scenario reflow 320/390/834/1024/1440 chữ100/200 |
| `cargo xtask ui-preview cmp verify` | PASS: common/JVM + desktop UI tests và captures |
| `./gradlew --no-daemon --console=plain -Dorg.gradle.java.installations.auto-download=false :preview:desktopTest --tests '*SharedControls*'` (cwd `packages/ui-system/cmp`) | PASS: 3 tests, Unicode/composition/selection/readonly/disabled, pending/chip geometry và ảnh lỗi+focus |

Web component regression đã PASS: label click, empty/focus/filled, native value restoration,
autofill pseudo-state mô phỏng CDP, one error/focus perimeter kể cả khi chèn fallback focus CSS
thật của app, 320px/text200 nhãn dài, controlled caret, IME CDP, readonly/disabled, native
radio Arrow, Search Enter/clear, 48px targets và resolved contrast light/dark.
CMP giữ NFD `TextFieldValue` gồm composition và selection qua theme recomposition; nhập giữa
chữ không normalize, readonly vẫn chọn được, disabled không enabled. Geometry Text button và
chip không đổi qua pending/selected. Ảnh state matrix là off-screen JVM, không là native-device QA.

Các lần fail được giữ trong log của work chat: thiếu VINK_CHROME → đặt đường dẫn Chrome có sẵn;
hết disk → xóa artifact target của chính worktree và build compact; sandbox cấm socket/bind →
chạy lại đúng lane ngoài sandbox. Lỗi lifetime của specimen và specificity focus/error đã sửa.
Regression harness ban đầu so `outline-width` dù `outline-style:none`, so AnnotatedString gồm
span underline IME thay vì text, và set native restored value trước reactive flush; đã sửa đúng
thao tác/observable. Không đổi production expectation hoặc bỏ scenario để đạt PASS.

## Ảnh đã mở

Ảnh runtime thật; design board trên Drive không được tính là runtime. Before dùng source baseline
nêu trên, after dùng source fingerprint trong [manifest](manifest.json). Các cặp giữ cùng fixture,
entry/scenario, theme và viewport. Default TextField preview opt-in FloatingTonal ở after; scenario
`external-label` kiểm compatibility riêng. Đây là khác biệt chủ ý của component specimen.

| Ảnh | Kịch bản / điều kiện |
| --- | --- |
| [Web trước](web-field-before.png) / [Web sau](web-field-after.png) | `field.text-field/default`, light, 1440 CSS px, DPR1, chữ100; raw gallery toàn trang |
| [CMP trước](cmp-field-before.png) / [CMP sau](cmp-field-after.png) | `field.text-field/default`, light, 1280dp, density1, scale1, workbench off-screen |
| [CMP chữ200](cmp-text200.png) | 390dp, light, preview scale2, font Be Vietnam Pro/Literata tự host; empty/filled/error/readonly/disabled/long-label/textarea |
| [CMP lỗi+focus](cmp-error-focus.png) | native text field đang focus trong UI test, dark, density1/scale1, Be Vietnam Pro; crop của node thật |

## Coverage còn thiếu

- NOT_RUN: Android/iOS diagnostic builds/runtime, OS IME thật, TalkBack/VoiceOver và named screen-reader walkthrough.
- NOT_RUN: saved-profile autofill thật; chỉ kiểm CSS pseudo-state và native restored DOM value (gán `.value` sau reactive flush, không phải saved-profile restore).
- NOT_RUN: app `verify web`, SSR GET/Explore/Desk/editor adoption và Appearance/Account R2; các consumer đó thuộc PR-02/03.
- Matrix browser headless và preview text scale không tương đương thiết bị native/OS font scale.
- Không kiểm/chờ GitHub CI, không deploy, không thay đổi issue hoặc đánh dấu acceptance trên issue.

Review cuối theo `vink-code-review`: không còn finding có căn cứ trong diff scoped; các giới hạn
bằng chứng trên vẫn còn. Toàn bộ thay đổi nằm ở `packages/ui-system/**`, không sửa application,
domain, persistence, API, CodeMirror hoặc save state.
