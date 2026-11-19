# Kiểm chứng CMP diagnostic #261 · 19/11/2026 UTC

Baseline `develop@eb7e65ee8a42d512343eb657abdd29cdec3d4f89`. Hai lượt Android cuối dùng
baseline này cộng [bản test Android](../../../../../packages/ui-system/cmp/androidDiagnostic/src/androidTest/kotlin/vink/ui/diagnostic/DiagnosticTest.kt)
có [SHA-256](api24-final/test-source-sha256.txt). APK host dùng
source của baseline. [LayoutSpec R1](../../../../design/empty-artwork-cmp-261.md) ·
[#269](https://github.com/loveoverflowcom/vink/pull/269) · [#272](https://github.com/loveoverflowcom/vink/pull/272) ·
[#317](https://github.com/loveoverflowcom/vink/pull/317) · [#318](https://github.com/loveoverflowcom/vink/pull/318).

## Đối chiếu lúc bắt đầu

| Trạng thái | Hiện trạng trên develop |
| --- | --- |
| EXISTING | #269 đóng gói ba alpha WebP canonical qua Gradle generator, closed enum và decoder theo source-set. #272 thống nhất trần 30 KiB inclusive. #317 có pixel, ma trận Android và sửa focus legacy lúc mở. #318 lưu focus của container qua tái tạo Activity. |
| PARTIAL | Android có kết quả geometry/CTA khi bàn phím đóng. Focus, query và selection của Explore sau tái tạo mới có test Compose Desktop và test chung cho field Android. |
| MISSING | Ca ảnh alpha lỗi trên native, CTA khi IME còn mở, nhập chữ qua bàn phím tiếng Việt, iOS Kotlin/Native và XCTest. |
| REGRESSION | Suite Android từ #317 có ca Search đỏ ở specimen Interaction, đã có trước delta. |
| UNKNOWN | Dynamic Type, VoiceOver, TalkBack, thiết bị vật lý và bộ gõ Telex/VNI. |

## Lệnh và ranh giới

Mac mini M4 arm64, macOS 15.6 (24G84), RAM 24 GiB, 17 GiB trống lúc chuẩn bị, JBR
21.0.11, Gradle 8.14.3, Kotlin 2.2.20, Compose Multiplatform 1.9.0, SDK compile 36.
Bootstrap `./tools/local-budget run --timeout 600 -- cargo build --locked -p xtask`
thoát 0 sau 30 giây với hai Cargo jobs, pressure normal. Launcher báo Gradle và native
`UNSUPPORTED`; chạy ngoài launcher theo lượt như #317 với hai Gradle workers, compiler Kotlin
in-process và một AVD ở mỗi thời điểm (2 core, 2 GiB RAM). API 24 dùng lại image arm64 từ #317
với AVD mới. [Preflight](preflight.log).

| Boundary / lệnh | Kết quả thực |
| --- | --- |
| Asset — `node apps/vink-web/assets/empty/verify.mjs` và `node --test apps/vink-web/assets/empty/verify.test.mjs` | PASS, exit 0, 3/3 validator tests. Ba WebP 512×427, 20.070 / 16.444 / 19.190 byte, codec/alpha và SHA-256 khớp. [Log](asset-check.log) |
| Desktop/common — `cargo xtask ui-preview cmp verify` | PASS, exit 0, 110/110, 0 skip. Tạo 48 ca riêng của #261. Common kiểm pixel thật trên Skia JVM ở 192×160 và 512×427, opaque rejection, và hủy khi kết quả đã chờ dispatcher. [Gradle](desktop/verify.log) · [Số ca](desktop/test-counts.json) · [JUnit](desktop/alpha-and-cancellation.xml) |
| API 35 baseline — `ANDROID_SERIAL=emulator-5580 cargo xtask ui-preview android verify` | FAIL exit 1, 7/8. Ca đọc pixel và CTA #261 PASS. [Runner](api35-baseline/verify.log) · [JUnit](api35-baseline/instrumentation.xml) · [Thiết bị](api35-baseline/device.log) |
| API 35 sau bổ sung test — cùng lệnh | FAIL exit 1, 9/10. Cả hai ca mới và ca pixel #261 PASS. [Runner](api35-final/verify.log) · [JUnit](api35-final/instrumentation.xml) · [Pixel](api35-final/alpha.log) |
| API 24 — `ANDROID_SERIAL=emulator-5582 cargo xtask ui-preview android verify` | FAIL exit 1, 9/10 ở framebuffer 1080×1920, 420dpi. Cả hai ca mới và ca pixel #261 PASS. [Runner](api24-final/verify.log) · [JUnit](api24-final/instrumentation.xml) · [Pixel](api24-final/alpha.log) · [Thiết bị](api24-final/device.log) |
| iOS Kotlin/Native, iOS Simulator | NOT_RUN. Máy có Command Line Tools, thiếu Xcode, `simctl`, XcodeGen và UDID sẵn dùng. |
| `cargo xtask verify quick` | NOT_RUN. Delta chỉ thêm Android instrumentation và bằng chứng. Lane CMP đã chạy token drift, focused Android đã biên dịch và chạy trên cả hai API. |

Hai suite cuối cùng đều đỏ ở `interactionImeClearAndSheetKeepControlledText`. Expected
“Đã tìm 1 lần”, actual “Đã tìm 0 lần”. Cùng assertion đã FAIL ở baseline API 35 lần này
và baseline hai API ở [lượt 18/11](../native-qualification-20261118/README.md). Source
`InteractionSpecimen.kt`, `VinkFields.kt` và thân test này giữ nguyên. Mẫu
`composition.interaction/sheet` dùng callback Search riêng. Explore/Desk dùng
`VinkSearchField` với query thuộc `ScreenRecipe` và CTA riêng. Phân loại
`OUT_OF_SCOPE` cho #261, giữ assertion và kết quả đỏ. Follow-up đề xuất trong phạm vi #24
là xác định vì sao Search qua instrumentation không dispatch sau luồng Next → clear → nhập lại.

Ghi thêm các lần thử. Bản nháp test API 35 không compile vì `performTextReplacement`
trả `Unit`, đã tách lệnh gọi. Lượt API 24 ở framebuffer 390×820 nhưng density mặc định 420
có khoảng 149dp bề ngang và 7/10 test PASS. Hai lỗi bổ sung là nút pending và focus của
field chung. Cả hai qua sau khi chạy lại cùng test source ở framebuffer chuẩn. Khác biệt
viewport đã được lưu, cùng lỗi Interaction ở bước Clear. [Lỗi compile](api35-final/test-draft-compile.log) ·
[Runner API 24 khung hẹp](api24-narrow-preflight/verify.log) ·
[JUnit](api24-narrow-preflight/instrumentation.xml) · [Kích thước/density](api24-narrow-preflight/wm.log).

## AC-04 · Pixel và fallback

BitmapFactory (API 24) và ImageDecoder software (API 35) trả 192×160, 122.880 byte
pixel ARGB8888 với alpha tối đa 254/255. Số pixel alpha 0 / bán trong suốt đo từ output thật.

| Asset | API 24 | API 35 |
| --- | ---: | ---: |
| no-results | 22.290 / 8.430 | 22.176 / 8.544 |
| no-public-works | 22.814 / 7.906 | 22.740 / 7.980 |
| first-draft | 22.455 / 8.265 | 22.348 / 8.372 |

Test Android mới mở Desk/empty ở multiplier 2 khi host báo không hoạt động. Câu trạng thái
và CTA tồn tại khi chưa có ảnh, click phát đúng intent. Sau khi bật lại host, alpha WebP
render, copy và CTA vẫn tồn tại. Cả API 24 và 35 PASS. Decoder và policy ảnh bìa opaque
byte-identical với baseline.

Lượt fault injection riêng trên API 35 chỉ thay bytes sinh trong `build/generated/empty-artwork`
cho `first-draft` bằng Base64 `AA==` (một byte), rồi chạy
`./gradlew --no-daemon --console=plain --max-workers=2 -Pkotlin.compiler.execution.strategy=in-process -Pvink.ui.nativeTargets=true :androidDiagnostic:assembleDebug -x :diagnostics:generateEmptyArtwork`.
Ở cùng Desk/empty, 390×820dp, Dark, OS 100%, ảnh lỗi biến mất, rect câu và nút giữ nguyên.
Bấm nút ở cả hai APK nhận “Mẫu · Đích #screen.editor/dirty”. Cả hai ảnh gốc đã mở. Sau thử
nghiệm đã phục hồi generated bytes và cài lại APK canonical. [Lệnh/build](fault-build.log) ·
[Readback](fault-run.log) · [SHA APK](fault-apk-sha256.txt) · [Bình thường](api35-runtime/desk-empty-390-dark-100-canonical.png) ·
[Lỗi decode](api35-runtime/desk-empty-390-dark-100-corrupt.png) ·
[Bounds canonical](api35-runtime/canonical.json) · [Bounds fault](api35-runtime/corrupt.json).

## AC-06 · Layout, bàn phím và selection

Desktop tạo ma trận 48/48: bốn trạng thái × 320/390/834dp × Light/Dark × multiplier
1/2, off-screen Skia với font TTF. JUnit xác nhận slot 176/240dp ở các viewport thử, tỷ lệ
512:427, trục giữa, ảnh trang trí và CTA sau cuộn. Mở trực tiếp bốn ảnh đại diện.
[Context](desktop/context.json) · [JUnit](desktop/preview-actions-and-geometry.xml) ·
[320 Dark 200](desktop/screen.desk--empty--320--dark--200.png) ·
[390 Light 200](desktop/screen.explore--no-results--390--light--200.png) ·
[834 Dark 100](desktop/screen.explore--empty--834--dark--100.png) ·
[834 Light 100](desktop/screen.desk--no-results--834--light--100.png).

API 35 chạy bốn ca bổ sung khi bàn phím OS còn mở, phủ đủ bốn state và ba độ rộng. Đặt
`wm size`, `wm density 160` và `settings put system font_scale 1.0|2.0` trong AVD.
Mở diagnostic Activity với
`entry/scenario/theme` và `scale=100` (multiplier 1). Đọc `mCurrentConfig` của
`DiagnosticActivity` từ `dumpsys activity top`. Khi mở, ô tìm chưa focus. Chạm ô,
cuộn bằng touch trên vùng app. Đọc rect của node bấm được từ UI Automator và mép
IME từ `dumpsys window displays`. CTA nằm trên IME, tap phát đúng intent cả 4/4.
Mở đủ bốn ảnh gốc. Hai lượt đo trước đã hiệu chỉnh parser `visibleFrame` và đo node bấm
thay vì TextView con. [Số đo](api35-runtime/keyboard-matrix.json).

| Ca | IME bắt đầu tại Y | Khoảng Y của nút | Ảnh |
| --- | ---: | ---: | --- |
| Desk/empty, 320 Dark 200% | 386 | 39–130 | [CTA](api35-runtime/screen.desk-empty-320-dark-200-keyboard-cta.png) |
| Explore/no-results, 320 Dark 200% | 386 | 280–334 | [CTA](api35-runtime/screen.explore-no-results-320-dark-200-keyboard-cta.png) |
| Explore/empty, 390 Light 100% | 490 | 313–361 | [CTA](api35-runtime/screen.explore-empty-390-light-100-keyboard-cta.png) |
| Desk/no-results, 834 Light 200% | 517 | 292–346 | [CTA](api35-runtime/screen.desk-no-results-834-light-200-keyboard-cta.png) |

Ca mới trên cả hai API nhập chuỗi Unicode có dấu tổ hợp và chọn `TextRange(2, 5)` trong
Explore/no-results, đổi theme, tái tạo Activity rồi kiểm lại query/selection và thao tác
xóa. Instrumentation này dùng Compose semantics. Phép thử IME riêng trên API 35 dùng
LatinIME tiếng Việt (subtype `vi`), chạm T/H, giữ O và chọn Ơ. Đọc query “thơ” lúc
đang gạch chân qua đổi theme, rồi “thơ ” sau phím Space. Cả hai ảnh query và ảnh bảng
dấu đã mở. Lượt 18/11 vẫn sở hữu ma trận native đầy đủ khi bàn phím đóng của cả hai API.
[Query Light](api35-runtime/latinime-vi-query-light.png) ·
[Query Dark](api35-runtime/latinime-vi-query-dark.png) · [Chọn dấu](api35-runtime/latinime-held.png) ·
[Readback](api35-runtime/latinime-vi.log).

## AC-07 · Kết quả theo chiều

Self-review theo LayoutSpec R1, `fidelity: NO_APPROVED_REFERENCE`. Kết quả của lượt này
chỉ gắn những ca thực chạy. [SHA-256 các ảnh gốc đã mở](capture-sha256.txt).

| Chiều | Desktop/common | Android API 24 | Android API 35 | iOS Simulator |
| --- | --- | --- | --- | --- |
| Functional | PASS pixel, cancellation, fallback và callback | PASS pixel, CTA khi ảnh chưa có và sau khi render | PASS thêm fault injection của ImageDecoder | NOT_RUN |
| Geometry | PASS tỷ lệ slot và trục giữa | PARTIAL, ma trận fontScale và safe area là bằng chứng lượt 18/11 | PASS 4/4 rect CTA với IME, ma trận khi IME đóng từ lượt 18/11 | NOT_RUN |
| Visual | PASS bốn ảnh đại diện đã mở | PARTIAL, ảnh ma trận 18/11 | PASS sáu ảnh gốc CTA/fallback, kèm ba ảnh thao tác IME | NOT_RUN |
| Accessibility/Responsive | PASS reflow multiplier 2 và semantics | PARTIAL, TalkBack và IME mở chưa chạy trong lượt này | PARTIAL, reflow OS 2.0 và node CTA đạt, TalkBack chưa chạy | NOT_RUN |
| Interaction | PASS selection, focus, cuộn/callback trong Compose | PASS ca #261 query/selection qua tái tạo, suite vẫn có lỗi Interaction riêng | PASS ca #261 trên emulator, LatinIME tiếng Việt và chạm CTA, suite vẫn có lỗi Interaction riêng | NOT_RUN |

| AC | Trước | Sau lượt này |
| --- | --- | --- |
| AC-04 | PARTIAL sau #317 | PARTIAL. Tái kiểm hai decoder Android, thêm lỗi thật ở ImageDecoder và test fallback host. Thiếu iOS pixel/runtime. |
| AC-06 | PARTIAL sau #317/#318 | PARTIAL. Thêm IME inset và thao tác CTA trên API 35, giữ query/selection qua tái tạo cho cả hai API. Thiếu Dynamic Type và ma trận iOS, ca CTA với bàn phím mở trên API 24. |
| AC-07 | PARTIAL sau #317 | PARTIAL. Thêm kết quả Android ở HEAD và thao tác LatinIME có dấu. Thiếu Telex/VNI, TalkBack, VoiceOver, thiết bị vật lý và iOS. |

Tiếp tục iOS trên Mac arm64 có Xcode đầy đủ, XcodeGen và iOS Simulator. Xác nhận UDID bằng
`xcrun simctl list devices available`, sau đó chạy từng lệnh theo ngân sách của repo.

```sh
VINK_UI_NATIVE_TARGETS=1 cargo xtask ui-preview cmp verify
VINK_UI_IOS_DEVICE=<UDID> cargo xtask ui-preview ios verify
```

Chạy thêm bốn trạng thái ở Dynamic Type, kiểm mép safe area/keyboard và VoiceOver, và
named TalkBack/IME trên thiết bị sẵn có. Giữ #261 mở với AC-04/06/07 còn thiếu.
