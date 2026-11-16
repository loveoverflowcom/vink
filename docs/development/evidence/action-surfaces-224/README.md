# Action surfaces #224 — Web/CMP

Phạm vi: shared action presentation, token aliases, TooltipIconButton/EndorsementAction,
contracts/catalog/specimens/tests; không migrate app pages. Nhánh `codex/action-surfaces-shared`,
base `origin/develop@a3e65360e316d27e5d029a43aef8b3083f622fb7` (fetch 16/11/2026).
Source/capture identity và hash ảnh được ghi trong `manifest.json`; final commit SHA nằm ở PR.
Review là **self-review**, `fidelity: NO_APPROVED_REFERENCE`. Mẫu không gọi dịch vụ.

## Lựa chọn và API

CHOICE-01 **B**: một enum `ActionSurface::{Standard,Tonal}` cho icon/toggle; không thay
ButtonVariant hay dựng action framework. Hai family có cùng vocabulary container, còn
pressed/value/phase/availability tiếp tục độc lập. TooltipIconButton và EndorsementAction
chuyển tiếp prop, giữ tên độc lập tooltip, copy “Thấm/Hữu ích” và Pen Flight.

CHOICE-02 **B**: `action.neutral.container/content` alias từ surface-container-high/on-surface-variant.
Cặp role dùng cho hai family; action-tonal của Button và selected của toggle/icon giữ nguyên.
Nền idle vì vậy khác selected; On luôn đi cùng pressed/check. Không thêm màu nguyên thủy.
Token 0.6.0; catalog 0.9.0; API map 0.5.0. [Recipe vai trò → variant/icon](../../../../packages/ui-system/shared/components.md#phân-cấp-hành-động-và-surface-opt-in-224).

```rust
// Leptos; props còn lại theo contract component.
<IconButton icon=IconName::Edit label="Sửa tiêu đề" surface=ActionSurface::Tonal />
// ToggleAction và EndorsementAction cũng nhận surface=ActionSurface::Tonal.
```

```kotlin
VinkIconButton(IconName.Edit, "Sửa tiêu đề", onPress, surface = ActionSurface.Tonal)
// VinkToggleAction / EndorsementAction: named surface, Standard mặc định.
```

Không cần migrate caller cũ. Standard giữ transparent idle và geometry cũ của icon/toggle.
CMP Button chỉ sửa reflow nhãn dài: label dành constraint riêng với leading/status, RadiusXl
khi nhiều dòng để tránh clip; nút một dòng vẫn RadiusFull. Không đổi font/variant/authority.
Tonal toggle đặt count dưới label, glyph/status có chỗ riêng; Web count có thể dùng cả cột status
ở hàng dưới để không bẻ số đếm tại 320px/text200.

Product rules không đổi; architecture vẫn `DESIGN_PROPOSED`, không có design signoff/ADR Accepted.
Implementation là shared `web`/`cmp` preview, chưa consumer #225/#226, không có native product app.

## Kiểm chứng local

Bản log trong PR chỉ chuẩn hóa khoảng trắng cuối dòng; nội dung kết quả giữ nguyên.
Kết quả chốt theo từng lệnh/target; [quick](quick.log), [Leptos](leptos.log),
[CMP dispatcher](cmp-dispatcher.log), [CMP tests](cmp-tests.log), [scoped Web](web-actions.log),
[HTTP regression sau sửa](http-regression-after.log) và các attempts được giữ.
Cấu hình Rust giảm dung lượng, không đổi assertions:
`CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0`.
Chrome Mac qua `VINK_CHROME=/Applications/Google Chrome.app/Contents/MacOS/Google Chrome`;
CMP dùng JDK Microsoft 21.0.7 đã có sẵn, Gradle no-daemon.

| Lệnh / boundary | Kết quả | Phạm vi |
| --- | --- | --- |
| `cargo xtask ui-tokens --check` | PASS | Token aliases, contrast pairs, generated files, catalog |
| `cargo xtask verify quick` | PASS | fmt/clippy SSR + hydrate + preview, doctests, unit/contracts và repo checks |
| `cargo xtask ui-preview leptos verify` | PASS | Preview CSR + media hydration fixture, Chrome CDP, toàn bộ gallery checks |
| `cargo xtask ui-preview cmp verify` | PASS | Common/JVM + Desktop tests/captures; không thay native |
| Chrome `preview-smoke.mjs --actions-only 1` | PASS | Scoped surface matrix; không là full Web PASS |
| `cargo xtask verify web` | NOT_RUN | App pages/editor không đổi; không kết luận caret 390×420 đã sửa |
| PostgreSQL/model/Kani | NOT_RUN | Không đổi durable behavior, DB hoặc pure domain policy |
| Android/iOS, OS IME, TalkBack/VoiceOver | NOT_RUN | Cần host/thiết bị và walkthrough riêng |
| GitHub CI | NOT_RUN | Không kiểm/chờ/poll theo yêu cầu |

Full Leptos dispatcher: 862 capture records; scoped actions: 24 cases; CMP: 16 captures.
11 ảnh đại diện được mở và đưa vào PR, không nhận 862 ảnh là đã review thị giác.

Preview matrix: light/dark; Web 1440/390 text100 và 320 text200, CMP offscreen 390dp
fontScale 1/2. Chữ 200 là multiplier preview; không thay browser zoom, Dynamic Type hay OS scale.
Web invariant đo trên native button ≥48 CSS px; CMP icon có vùng vẽ 40dp và **touch bounds ≥48dp**
của Material, không dùng kích thước wrapper để chứng minh. Toggle target ≥48dp.

Web checks cover pointer/Enter/Space một dispatch, zero khi pending/disabled, native utility type,
name/description/announce, default form/link/chip semantics, focus/width ổn định, ACK-only,
failure/uncertain/reconcile, truthful Unknown, leading/label/count/spinner không chồng/cắt,
forced-colors và reduced-motion. Neutral/selected text/icon ≥4.5; hover/press composite ≥4.5,
focus với container và surface cha ≥3. CMP tests kiểm semantics, key, injected touch,
focus khi đổi presentation, blocked/pending và controlled confirmed state; không suy native PASS.

CMP tổng cộng 104 tests (ui 43, patterns 5, diagnostics 6, preview 50), 0 failures/errors/skipped.
Rust focused owners trong quick: vink-ui 57, vink-ui-preview 9, vink-patterns 13.
Screen reader và focus appearance trên native OS không được suy từ semantics/JVM.

## Self-review và acceptance

Hai vòng sửa: (1) sửa overflow của tonal label/count ở CMP và khoảng chữ Web;
(2) cho Web count rộng hơn ở hàng dưới, sửa CMP Button multiline và đợi text layout ổn định trước capture.
Giữ ảnh FAIL ban đầu trong `round1/`. Không đổi baseline/golden ảnh để làm PASS.
Dispatcher preview có một sửa tương thích Mac: accepted socket kế thừa nonblocking từ listener,
`write_all` trả WouldBlock và gửi thiếu WASM lớn. Regression stream 4MiB FAIL trên code cũ,
PASS sau khi đặt accepted stream về blocking; không sửa HTTP server sản phẩm hoặc thêm runner.
Token golden assertions cập nhật đúng version 0.6.0 và exact recipe prefix có hai alias mới;
không bỏ assertion hoặc hạ ngưỡng. Một test icon ban đầu đo drawn bounds 40dp; lượt sau kiểm
đúng touchBoundsInRoot ≥48dp và giữ kiểm pointer/keyboard.

| Chiều | Web component preview | CMP Desktop/offscreen | Android/iOS |
| --- | --- | --- | --- |
| Functional | PASS · CDP | PASS · JVM | NOT_RUN |
| Geometry | PASS · DOM targets | PASS · touch bounds + reflow | NOT_RUN |
| Visual 100% | PASS · ảnh đã mở | PASS · ảnh đã mở | NOT_RUN |
| Accessibility/Responsive 200% | PASS · names/contrast/reflow | PASS · semantics/target/reflow | NOT_RUN |
| Interaction | PASS · pointer/key/focus | PASS · injected touch/key/focus | NOT_RUN |

| AC | Kết quả / evidence |
| --- | --- |
| AC-01 | PASS (phạm vi local) · Recipe vai trò trong components.md; không global Text→Tonal hoặc icon mọi nút |
| AC-02 | PASS (phạm vi local) · Optional surface + default Standard; wrapper propagation Rust/browser/CMP tests |
| AC-03 | PASS (phạm vi local) · Generated CSS/Rust/Kotlin, catalog paired scenarios; alias cùng owner |
| AC-04 | PASS (phạm vi local) · Browser và CMP tests; one native target, one dispatch, zero blocked, giữ focus |
| AC-05 | PASS (phạm vi local) · State matrix, contrast/focus layers, forced-colors/reduced-motion |
| AC-06 | PASS (phạm vi local) · Long Vietnamese leading/count/spinner + reflow; real target bounds |
| AC-07 | PASS (phạm vi local) · Ảnh đã mở liệt kê riêng trong manifest; local checks và native gap tách biệt |
| AC-08 | PASS (phạm vi local) · Diff shared presentation/pattern forwarding, không API/DB/auth/editor/Dialog/Appearance |

Attempts được giữ: hết đĩa ở doctest sau khi clippy pass; lượt quick sandbox bị dừng đúng owned
process group khi test outage không kết thúc; hai token golden tests phát hiện version/prefix cũ
và được cập nhật giữ nguyên ý nghĩa; một lượt preview lỗi tải WASM. Lượt cuối được ghi riêng,
không nhận lỗi hạ tầng hay scoped PASS thành full lane PASS. Cache build vừa tạo ở checkout chính
được dọn để giảm dung lượng; không chạm worktree #204, database hoặc evidence việc khác.

Ảnh đại diện đã mở: [Web icon light](images/web-icons-light-100.png),
[Web icon dark](images/web-icons-dark-100.png), [Web nhãn dài text200 (tile 6)](images/web-long-label-dark-200-part6.png),
[CMP thứ bậc text100](images/cmp-hierarchy-light-100.png), [CMP nhãn dài + spinner text200](images/cmp-hierarchy-dark-200.png),
[CMP state matrix text200](images/cmp-toggles-dark-200.png). Tile chỉ là phần được chỉ rõ;
metadata raw giữ toàn bộ phép chia và geometry, không gọi tile là ảnh đầy đủ của trang.

Không merge/deploy/đóng issue hoặc tick checklist GitHub. #225/#226 có thể dùng prop typed
khi shared contract được tích hợp; giữ guard, copy và Pen Flight của wrapper hiện hữu.

## Rebase và sửa từ bước review PR #228 (16/11/2026)

Nhánh được rebase lên `origin/develop@a81333c71faf6033014a00ec26c9f7b90109d1d9` (sau #227/#217 và #232/#229).
Các điều chỉnh trong phạm vi PR:
- **Precedence disabled trên action có chọn (`aria-pressed="true"` / `pressed = true`):** bổ sung
  `.vk-ui-icon-button[aria-pressed="true"][data-state="disabled"]` và
  `.vk-ui-chip[aria-pressed="true"][data-state="disabled"]` trong `vink-ui.css`, đồng thời cập nhật
  `disabledContainer` của `VinkIconButton` (`tonal || pressed == true`) trong `VinkActions.kt` để
  control đang bật nhưng bị khóa luôn hiển thị đúng `disabled-container`/`disabled-content` trên cả
  Web và CMP (caller `Standard` không bật vẫn giữ nền trong suốt).
- **Lưới CSS của `ToggleAction` Tonal khi không có glyph hoặc không có count:** chỉ kéo dài
  `.vk-ui-toggle__glyph` qua 2 hàng khi có `.vk-ui-toggle__count`, và thu gọn cột đầu khi không có
  glyph (`:not(:has(.vk-ui-toggle__glyph))`) để nhãn bắt đầu ngay tại `padding-inline-start` (12px),
  không tạo khoảng trống cột rỗng.
- **Đồng bộ bảng contract và dọn trùng sau rebase:** cập nhật chữ ký `TooltipIconButton` cùng hàng
  `IconButton`/`ToggleAction` trong bảng Props của `packages/ui-system/shared/components.md`, chuẩn
  hóa định dạng `Patterns.kt` và giữ một nơi đặt `set_nonblocking(false)` trong `respond()` của
  `xtask/src/ui_preview.rs`.
- **Kiểm chứng lại sau sửa:** `cargo xtask ui-tokens --check` PASS (15 generated files),
  `cargo xtask verify quick` PASS (toàn bộ 9 bước), `cargo xtask ui-preview cmp verify` PASS
  (104/104 tests, 0 failures), `cargo xtask ui-preview leptos build` + Chrome
  `preview-smoke.mjs --actions-only 1` PASS (bao gồm các assertion mới cho disabled `aria-pressed`,
  căn dòng glyph/label một hàng và khoảng lề khi không có glyph). Full app `verify web`,
  Android/iOS runtime, OS IME và screen reader thực tế giữ nguyên `NOT_RUN`.
