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

## Lossy WebP trong CMP (#116)

Base `develop@3638d65` đã có policy, generator và fixtures của
[#115](https://github.com/loveoverflowcom/vink/issues/115) từ
[PR #122](https://github.com/loveoverflowcom/vink/pull/122). CMP dùng lại hai vector dương/âm.
Ảnh chi tiết chẩn đoán dùng cùng nền tổng hợp 768 × 512, thêm chữ “ĐƯỜNG VỀ” và “Một buổi
chiều” bằng Be Vietnam Pro Semibold. Bản q78 dài 7.460 byte, SHA-256
`a6a741df4341ff692c60f6478841907fddb7188a6a86abc5bb9f2edee49ecc4b`.
[Profile chung](../../packages/ui-system/shared/content-images.md) ·
[Fixture chi tiết](../../packages/ui-system/cmp/diagnostics/src/commonMain/kotlin/vink/ui/preview/ContentImageFixture.kt)

Host kiểm RIFF và chunk `VP8 `, kích thước, phân vùng đầu trước khi cấp buffer ảnh.
Giới hạn diagnostic là 512 KiB mã hóa, 2.048 px mỗi cạnh nguồn và 2.097.152 pixel nguồn.
Kích thước decode lấy từ khung đo bằng pixel đã tính density và đủ để crop theo tỷ lệ
nguồn trong giới hạn cạnh ra 1.024 px. Worker dùng chung chạy một lượt một lúc. Khi
đổi bytes/khung hoặc rời composition, lượt cũ bị hủy ở phía nhận. Khi host trở lại
active, lượt còn Loading mới được chạy lại. Bitmap hợp lệ vào `BitmapPainter` và
`ImageState.Ready`; lỗi đi qua `ImageState.Failed` để khung/initials hiển thị fallback.
Ví dụ mẫu chung 256 × 171 cho ảnh thật với alt hiện hành. Mẫu âm 20 byte dừng
trước payload và hiện “Không tải được ảnh”.

- **CHOICE-01** chọn `ImageDecoder` từ API 28 để lấy đúng kích thước và từ chối ảnh
  giải mã một phần. API 24–27 dùng `BitmapFactory` có sampling và thu về đúng kích thước.
  Nhánh cũ chỉ nhận đúng bytes của hai fixture đóng gói vì `BitmapFactory` có thể trả
  bitmap khi decoder báo input chưa đầy đủ.
  [ImageDecoder](https://developer.android.com/reference/android/graphics/ImageDecoder) ·
  [BitmapFactory trong AOSP](https://android.googlesource.com/platform/frameworks/base/+/70f5c48e8e0403abe7e0f489b30890e46aac0d8b/core/jni/android/graphics/BitmapFactory.cpp)
- **CHOICE-02** chọn bitmap software ARGB_8888 để giữ nét chữ/màu và cho phép đọc lại
  pixel trong test. iOS/Desktop dùng Skia `Codec.readPixels` trước khi chuyển bitmap
  immutable sang Compose. [Skiko 0.9.22.2](https://github.com/JetBrains/skiko/blob/v0.9.22.2/skiko/src/commonMain/kotlin/org/jetbrains/skia/Codec.kt)

### Kiểm chứng cục bộ ngày 09/11/2026

Chạy từ root Vink trên macOS arm64 (Mac16,10), JDK 17 và Xcode 26.6.
Android dùng SDK 36 để build. Cả hai emulator có mật độ 420 dpi (2,625×).
iPhone 17 Pro Simulator chạy iOS 26.5 (23F77), 402 × 874 pt ở 3×.

| Lệnh | Kết quả |
| --- | --- |
| `cargo xtask ui-tokens --check` | PASS. Vectors sinh ra khớp source chung. |
| `VINK_UI_NATIVE_TARGETS=1 cargo xtask ui-preview cmp build` | PASS. Desktop và các target thư viện bật native. |
| `VINK_UI_NATIVE_TARGETS=1 cargo xtask ui-preview cmp verify` | PASS. Common/Desktop và các test codec/đo workload bằng Kotlin trên iOS Simulator. |
| `ANDROID_SERIAL=emulator-5554 cargo xtask ui-preview android verify` | PASS 5/5 trên Pixel 7 AVD, Android 17/API 37, `ImageDecoder`. Test render đọc lại pixel và kiểm alt/fallback. |
| `ANDROID_SERIAL=emulator-5556 cargo xtask ui-preview android verify` | PASS 5/5 trên Pixel AVD arm64, Android 7.0/API 24, `BitmapFactory`. Kiểm alt/fallback qua semantics và đối chiếu ảnh chụp màn. |
| `VINK_UI_IOS_DEVICE=AB3B8430-88BD-4C77-AE22-19AFBFDA0BE1 cargo xtask ui-preview ios verify` | Ca WebP mới PASS, toàn suite FAIL 1/4. Test Sharing hiện hữu chờ nhãn cũ và yêu cầu ẩn nút gửi, trong khi `FeedbackComposer` tại base luôn render nút. |

Ảnh tham chiếu được giải mã từ fixture q78. Đã mở và đối chiếu ảnh chụp ở hai
emulator Android và iPhone Simulator. Dấu tiếng Việt trong chữ raster vẫn đọc rõ,
nền lavender và mặt trăng giữ màu gần ảnh tham chiếu. Mẫu 256 px hiện mềm hơn ở
khung lớn trên cả hai nền tảng. Khung 16:9 cắt chiều cao như trước. Fallback vẫn
đọc được ở theme tối với mức chữ mẫu 200%. Ảnh avatar hỏng trên API 24 hiện initials.
PNG chụp cục bộ trong `target/ui-preview/cmp-webp/`. XCTest giữ một attachment Ready.
Matrix chụp Desktop có thể ghi frame đầu Loading. Test giao diện Desktop đợi
trạng thái cuối trước khi kiểm alt và fallback.

### Workload trong bộ nhớ

Mỗi lượt dùng cùng fixture chi tiết 768 × 512, khung yêu cầu 192 × 108 px, trả
192 × 128 ARGB_8888 để cover crop. Bảy lượt tuần tự đo từ trước khi chuyển sang
worker đến khi nhận bitmap, bao gồm parse/base64 và lập lịch. Kích thước pixel thực
đo được là 98.304 byte mỗi output. Các số bộ nhớ sau đây đo toàn tiến trình,
gồm cả harness/renderer đang chạy và các ca trước đó.

| Target | Thời gian 7 lượt (µs), theo thứ tự | Bộ nhớ toàn tiến trình |
| --- | --- | --- |
| Android API 37 | 5774, 3029, 1541, 2405, 1563, 1564, 1370 | PSS 106.812 → 107.532 KiB. Đỉnh RSS 223.008 → 223.008 KiB. |
| Android API 24 | 23147, 1567, 1329, 1249, 1156, 1262, 1768 | PSS 46.949 → 47.881 KiB. Đỉnh RSS 127.000 → 128.456 KiB. |
| iOS 26.5 | 1093, 1066, 1042, 975, 951, 1076, 1041 | Đỉnh RSS 87.457.792 → 88.309.760 byte trong tiến trình test Kotlin. |

| Acceptance #116 | Bằng chứng và giới hạn |
| --- | --- |
| AC-01, AC-03 | Dùng lại fixture dương/âm sinh từ #115 và URL allow-list đã tích hợp, giữ ca từ chối AVIF trong test âm. `:ui` vẫn nhận Painter/ImageState. |
| AC-02 | Codec Android/iOS giải mã được WebP VP8 và từ chối input lỗi trong test. Các ca âm gồm cắt file, sai chunk, vượt giới hạn và cắt mất phân vùng pixel sau khi sửa độ dài container. |
| AC-04 | Giới hạn parse/sizing, một worker và test hủy trước khi giao kết quả. Ca UI đổi scenario kiểm state cuối và alt tương ứng. |
| AC-05 | Cả hai nhánh Android đã chạy. Software ARGB_8888 có số đo allocation trên từng API. |
| AC-06, AC-07 | Test render/fallback theo từng host và kiểm ảnh mở. Thiết bị vật lý và screen reader `NOT_RUN`. Suite iOS đầy đủ còn lỗi Sharing nêu trên. |
| AC-08 | Decode/count/kích thước/config và bộ nhớ theo workload trên. Ảnh bìa native qua mạng `NOT_IMPLEMENTED`. |
| AC-09 | Kiểm generated fixtures, lệnh và hồ sơ theo owner. |

## Interaction foundation 12/11/2026

Base `origin/develop@7459b91b21ed37e9142f6ffb8b91bd62733884a5` (đã có PR skills #172).
Source/runtime/tests được chốt tại `695cc86c911e634ec10f980289c6afb4ddc3eed9`; commit hồ sơ
sau đó chỉ bổ sung tài liệu. [Findings I-01–I-07, LayoutSpec, contract và dependency](../design/ui-interaction-foundation.md).
Phạm vi là UI System CMP và hosts chẩn đoán. Không có native product app, không đổi quyết định
sản phẩm/kiến trúc. Bằng chứng dưới đây ở boundary renderer/platform, không là model/backend proof.

### Lệnh và ranh giới

Máy macOS arm64; JDK Microsoft 17.0.19; Kotlin 2.2.20, Compose 1.9.0, Material3 1.8.2,
Gradle 8.14.3. Các lệnh chạy từ root; Gradle dùng `JAVA_HOME` JDK 17 và native dùng
`ANDROID_HOME` SDK đã cài. Logs, ảnh và hash cục bộ:
`target/ui-evidence/interaction-foundation-20261112/manifest.json` (không đưa binary vào Git).

| Lệnh / target | Kết quả và phạm vi |
| --- | --- |
| `cargo xtask ui-tokens` hai lần; `cargo xtask ui-tokens --check` | PASS, lượt hai không đổi output; tokens giữ 0.5.0, catalog 0.8.0, admission map 0.4.0. |
| `git diff --check`; `cargo xtask verify quick` | PASS. Kiểm source/metadata/contract và các lane quick hiện hữu; không thay thế runtime native. |
| `cargo xtask ui-preview cmp verify` | PASS toàn lane. `:ui` 43, `:diagnostics` 6, `:preview` 48 tests, không skip; có common policy chạy JVM, Compose Desktop và capture thực. Không suy iOS common test từ JVM. |
| `VINK_CHROME='/Applications/Google Chrome.app/Contents/MacOS/Google Chrome' cargo xtask ui-preview leptos verify` | PASS build, hydration fixture, keyboard/focus/reduced-motion assertions và 806 captures trong Chrome 157.0.8103.79; không suy mọi ảnh đã mở. |
| `ANDROID_SERIAL=emulator-5554 cargo xtask ui-preview android verify` | PASS 7/7 instrumentation tests trên Pixel_7, Android 17/API 37.1. Có Next → Search, clear/focus, sheet, Back ở dialog window và Explicit từ chối Back. |
| `VINK_UI_IOS_DEVICE=4B214753-8C0E-44D7-90A1-0AD5B0960A9F cargo xtask ui-preview ios verify` | PASS 5/5 XCTest, Xcode 26.6 (17F113), iPhone 17 Simulator iOS 26.0. Có nhập chữ, đổi theme, mở bàn phím, cuộn sheet, đóng và đọc lại chữ. |
| `cargo xtask verify web` / PostgreSQL / các lane model | NOT_RUN cục bộ trong lát này: không đổi app Web, nghiệp vụ hay DB. Leptos preview chạy browser riêng; CI có các lane rộng hơn. |
| Recipe `composition.interaction` trên Leptos | UNSUPPORTED theo thiết kế: IME/FocusRequester/Android Back là native API. Preview hiển thị lý do; không dùng trạng thái này để che test chưa chạy. |

`InteractionTest.searchClearReturnsFocusToTheEditableTargetAndDispatchesOnce` FAIL trước sửa
và PASS sau sửa; phép kiểm wrapper FocusRequester đã PASS trước sửa nên không gọi wrapper là lỗi.
`InteractionLifecycleTest` kiểm press kéo ra ngoài/cancel, pending activation, IME composition,
retarget, reduced giữa chừng, inactive ancestor, disposal, close reason Navigate, caller từ chối
dismiss và callback cũ sau reopen. Clock do Compose test điều khiển; ảnh tĩnh không chứng minh timing.

### Ảnh đã mở và thao tác native

Tám ảnh `desktop/interaction--{default,sheet}--{light,dark}--360--text{100,200}.png` đã mở kiểm.
Context: Desktop offscreen 360×800dp, density 1, Be Vietnam Pro/Literata TTF pinned, OS font scale 1,
preview multiplier 1/2, reduced motion, không có OS keyboard/insets. Sheet được cuộn tới action;
ở chữ 200% title có thể ở ngoài viewport nhưng close vẫn tới được qua cuộn. Không yêu cầu sticky
actions hoặc tự thay RowScope API.

Hai ảnh native với keyboard đã mở: `native/android-sheet-dark-keyboard.png` (1080×2400,
420dpi/2.625×, khoảng 411dp rộng) và `native/ios-sheet-light-keyboard.png` (1206×2622,
3×, 402×874pt). Native dùng system font fallback, OS font scale và preview multiplier đều 1,
reduced motion trong recipe. Nút đóng hiện trọn phía trên IME; field và text vẫn giữ khi cuộn.
Android manual: nhập thêm `Vink`, Back lần đầu ẩn IME và giữ sheet, lần hai đóng sheet,
readback vẫn `Tiếng Việt [Mẫu]Vink`; hai UI dump được giữ cùng ảnh.

### Lỗi gặp và giới hạn

- Giữ logs trước sửa: clear mất focus; test Android ban đầu dùng touch mode để đòi keyboard
  focus trên Button. Test cuối chuyển keyboard input mode bằng Tab, gọi platform Back thật;
  không thay focusability của Button để chiều test.
- Lượt iOS đầu FAIL vì swipe trên vùng keyboard và test Sharing cũ trái contract D2T-03
  đã có ở base (Sharing là intent, chỉ commentsClosed khóa gửi). Test Sharing nay kiểm composer,
  explicit submit và local intent; không đổi production FeedbackComposer hoặc bỏ test.
- Lượt iOS tiếp FAIL vì XCTest app-idle không chờ Compose scroll deceleration: frame action
  còn dịch chuyển khi tap. Test cuối swipe trong panel phía trên IME và đợi các snapshot frame
  ổn định trước tap; vẫn đòi đóng và giữ chữ, không retry activation. XCResult PASS:
  `Test-VinkUIDiagnostic-2026.11.12_18-03-07-+0700.xcresult`.
- Leptos lượt đầu build đạt nhưng browser prerequisite FAIL vì runner không tự tìm Chrome ở
  macOS. Lượt cuối chỉ rõ `VINK_CHROME`; log lần đầu được giữ.
- NOT_RUN: thiết bị vật lý, TalkBack/VoiceOver walkthrough, OS Vietnamese IME composition,
  native OS font scale 200%, platform predictive Back, release trace/performance. Prerequisite:
  thiết bị/assistive technology và IME tương ứng, cùng recipe; release benchmark cần workload
  và trace riêng. Desktop composition tests không thay thế walkthrough IME thực.
- Drag-dismiss/nested drag arbitration không được thêm; chỉ cuộn nội dung sheet. ModalBottomSheet
  đã được đánh giá nhưng default drag/hide lifecycle chưa phải contract được chốt. Không có
  proposal/signoff mới và không tuyên bố hiệu năng mượt hơn khi chưa đo.
