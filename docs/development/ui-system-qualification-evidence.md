# UIS-06 — quality gates, replay và diagnostic hosts

Bổ sung hiện hành sau UIS-06:
[UI-REPAIR-06/07](../design/ui-repair-06-07.md) ghi requalification của optional
StatePanel action slot (common/Desktop + native compile/runtime), capture focus
Web và mẫu evidence Android/iOS ngày 03/11/2026. Các kết quả dưới đây vẫn là lịch
sử theo revision đã ghi; không thay chúng bằng PASS của lượt mới. Slot callback
được test trực tiếp trên JVM; native runtime chạy ba regression sẵn có mỗi host.

Ngày 03/11/2026 (UTC+07), nền `develop@204d319` (UIS-05/#36 đã merge), nhánh
`codex/ui-system-qualification`. Phạm vi #24; requirement liên quan UX-01/UX-03, WR-01/WR-03
chỉ được giữ qua regressions hiện có, không nghiệm thu toàn MVP hay D-xx.

## Owner và delta

`shared/tokens.json` 0.4.0 không đổi giá trị; catalog 0.5.1 thêm lý do Web không có diagnostic
router đối ứng CMP. 49 entry/169 scenario, 110 public component declarations được map tới
entry ở `shared/api-coverage.json`. `verify quick` phát hiện API thiếu admission, mapping cũ,
preview dependency, native import sai owner, raw shared controls trong app, literal màu/type/motion.
Scan có phạm vi source/declaration đã mô tả; không thay dependency graph/browser review.
Ngoại lệ exact declaration có owner/reason/removal condition, không blanket ignore artwork.

CMP chuyển năm file specimen/Workbench từ Desktop sang `:diagnostics/commonMain`; Desktop,
Font Desktop hiện dùng derivative TTF từ WOFF2; lịch sử WOFF2 bên dưới giữ nguyên theo lượt
đã ghi. Nguồn và kết quả Linux/macOS mới nằm trong [PR 08](../design/ui-repair-08.md).

Android và iOS gọi cùng Specimen và public `:ui`/`:patterns`. Chỉ decoder ảnh, window/fonts,
lifecycle, OS motion và system bars thuộc host. App Web không import diagnostic/fixture;
không đổi query, API contract, authority, body editor hay save state machine.

LayoutSpec đã trình bày trước UI: diagnostic heading/controls → specimen → inspector/log,
một cột/cuộn trên màn hẹp, safe drawing/IME padding. Theme/utility giữ một call site specimen;
controlled native sample dùng TextFieldValue + Saver khi recreation. Android chỉ debug variant;
iOS host chỉ arm64 simulator, ad-hoc local signing, không production certificate/distribution.

Sau mở ảnh, nới nhãn compact rail trong padding và scrollbar viewport để “Khám”/“phá” không
bị bẻ ở 200%; rail vẫn 96px, toggle ≥44px. Focus ring vẽ bên trong vùng clickable ở compact.
Giữ nguyên cỡ chữ resolved khi thay literals bằng token (13/14/16/22px, weight 600).
Gỡ rule `vk-feed-end` sau audit source, dynamic selectors, build CSS và ListEnd consumer.

## Môi trường và lệnh

macOS arm64; Rust 1.99.1, Node 26.12.0, wasm-bindgen 0.2.129; Chrome 156.0.8078.79 headless/DPR 1;
PostgreSQL 17.11 cluster scratch riêng trên 55432 (không dùng DB/volume có sẵn).
JDK 17.0.19, Gradle 8.14.3, Kotlin 2.2.20, CMP 1.9.0/Material3 1.8.2, AGP 8.11.1.
Xcode 26.6 (17F113), simulator iPhone 17/iOS 26.0; SDK build iOS 26.5.
Android Pixel_7 AVD API 37.1/Android 17, arm64/16KB, 1080×2400px, 420dpi
(~411×914dp). Android compile/target SDK 36, min 24; AndroidX Test 1.7.0/JUnit 1.3.0,
Espresso 3.7.0. Native font: hệ thống; Web/Desktop: Be Vietnam Pro/Literata WOFF2 tự host.

| Command | Kết quả | Boundary |
| --- | --- | --- |
| `cargo xtask verify quick` | PASS 179 unit/contract + 8 doctests | fmt, Clippy SSR/hydrate/CSR, unit/contract/doctests, repository/admission guards |
| `VINK_TEST_DATABASE_URL=postgres://vink@127.0.0.1:55432/postgres cargo xtask verify web` | PASS sau rail delta cuối | Chrome + app SSR/hydrate + PostgreSQL scratch, main/after-restart |
| `cargo xtask ui-preview leptos verify` | PASS 642 capture contexts + viewport rail | Actual browser preview, mọi scenario, 320/390/834/1024/1440px, light/dark, text100/200%, forced-colors/names/nested controls |
| `VINK_UI_NATIVE_TARGETS=1 cargo xtask ui-preview cmp verify` | PASS | Actual Desktop renderer/captures + common/JVM, iOS simulator common tests, Android/iOS library compile; không thay UI runtime |
| `cargo xtask ui-preview android verify` | PASS 3/3 | Actual instrumentation: native input/theme/recreation, lifecycle activity, pending click admission, system bar icon mode |
| `VINK_UI_IOS_DEVICE=<UDID> cargo xtask ui-preview ios verify` | PASS 3/3 | Actual XCTest: soft keyboard/input/theme giữ text, field hittable, disabled, Sharing không có Send |
| `cargo xtask verify postgres` | NOT_RUN | Không đổi store/query/contract/transaction; web lane vẫn dùng DB thật |

Canonical commands/prerequisites/replay ở [development](README.md#diagnostic-ui-native-uis-06).
CI gọi cùng quick/web/Leptos/CMP commands; native UI checks cần host riêng, không bị gộp vào
một aggregate PASS. CI của PR chưa phải bằng chứng local.

## Review và ảnh

Ảnh nguyên bản và hash/context được giữ ở [manifest](evidence/uis-06/manifest.json).
Không đính kèm font, credential, session hay manuscript riêng; DB và HTTP fixtures đều tổng hợp.
Ảnh native đi từ emulator/simulator screen, không phải Desktop PNG hay WebView.

Đã mở baseline UIS-05 `app-explore.png` và bản mới; baseline đó dùng một seed còn ảnh matrix
dùng 12 HTTP cards nên không gọi là pixel comparison cùng fixture. Dùng `explore-real-1440-light`
để đối chiếu cùng seed. Không auto-bless screenshot baseline. Mọi ảnh giữ trong manifest phải
được mở sau lần chạy cuối; ảnh khác chưa mở không được suy là visual review.

Các lần FAIL trong quá trình: runner Espresso cũ gọi InputManager API đã bỏ ở Android 17
(đổi sang stable 3.7.0); plist iOS thiếu boolean high-refresh (sửa project.yml, giữ strict check);
XCTest tìm TextField nhưng CMP expose TextView và text ở native label (đổi đúng query, giữ
assert exact text); runner simulator cache bản test cũ (cài lại runner riêng task, local ad-hoc signing);
rail app PASS nhưng gallery short-height còn scrollbar lấy bề rộng (sửa owning CSS, không bỏ assertion).
Tự review theo caller/owner/failure và dependency graph; không có reviewer độc lập.

## Readiness và điều kiện còn mở

| Target | Source/code | Visual review | Qualified boundary | Còn thiếu |
| --- | --- | --- | --- | --- |
| Web app | Có | Ảnh app thật/fixtures | Chrome SSR/hydrate/editor/router trên matrix đã ghi | Safari/Firefox, zoom trình duyệt thật, OS IME, screen reader |
| Leptos preview | Có | Đại diện family/state/rail | Browser/scenario gate riêng | Không là app/API/DB proof hoặc chứng nhận WCAG |
| CMP Desktop | Có | Native renderer off-screen | JVM/common/callback/state/captures | Desktop window thật, hover/OS accessibility |
| Android diagnostic | Có, debug-only | Native emulator, OS font scale 1/2 | 3 instrumentation tests trên AVD đã ghi | TalkBack, Vietnamese IME, predictive Back, tablet/device/lifecycle toàn matrix |
| iOS diagnostic | Có, simulator-only | Native simulator, Dynamic Type category | 3 XCTest trên simulator đã ghi | VoiceOver, Vietnamese IME, swipe-back, iPad/device/signing, lifecycle toàn matrix |

OS font scale 2 và iOS accessibility-large được đổi thử rồi trả về 1.0/large; multiplier của
preview khác OS setting. System motion adapters có source; native motion preference UI chưa
được named manual run. Web timed frames/assertions và CMP lifecycle tests là các lớp riêng.
Native branded font pipeline TTF/OTF UNSUPPORTED hiện tại; fallback hệ thống được ghi rõ.
Không gọi đây là full cross-platform/native qualification. #24 giữ mở cho các target/manual gaps;
#14 B/C, auth/Discourse/publication/AI/native editor production vẫn thuộc owner cũ.

Extension/version/migration recipes: [EXTENDING](../../packages/ui-system/EXTENDING.md),
[compatibility](../../packages/ui-system/shared/compatibility.md).

Nguồn API/tooling đã đối chiếu: [UIKit integration của Kotlin](https://kotlinlang.org/docs/multiplatform/compose-uikit-integration.html),
[Android Compose UI testing](https://developer.android.com/develop/ui/compose/testing),
[AndroidX Test stable versions](https://developer.android.com/jetpack/androidx/releases/test).
Giữ pins Kotlin/CMP/AGP hiện có; thêm Activity Compose 1.10.1 và AndroidX Test cho diagnostic host.

Token SHA-256: `7771a63e66d5f5416f0694bce05f064e3c36864b520485a43a91973d335dba5e`. Implementation input digest và từng file/capture hash nằm trong manifest.
CMP: 67 tests JVM/Desktop (31 ui + 1 patterns + 2 replay + 33 preview), 31 iOS common (28 ui + 1 patterns + 2 replay), 429 PNG trên Desktop; không suy mọi PNG đã mở.

Mười ba ảnh giữ trong manifest đã mở. `web-explore.png` và `cmp-feedback.png` trùng byte với ảnh cùng fixture giữ ở UIS-05; các ảnh matrix khác không có baseline đồng cảnh để tuyên bố pixel parity. Native status bars đã mở lại sau sửa theme; ảnh chụp quá sớm ở splash/transition được bỏ và chụp lại khi UI idle.

Tích hợp `develop@fe18d9f` (#37, skills/capture/Drive docs) trước PR; 145 implementation inputs trong manifest không đổi. Chạy lại `verify quick` cho metadata/links mới; không suy bằng chứng runtime mới từ việc merge tài liệu. Cluster PostgreSQL scratch và native diagnostic installs/emulator riêng task đã được dọn; simulator có sẵn vẫn giữ booted.
