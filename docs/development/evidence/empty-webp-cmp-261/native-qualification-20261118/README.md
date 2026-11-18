# Kiểm chứng CMP diagnostic #261 · 18/11/2026 UTC

Baseline `develop@ee20e9402879c697b66445f490488ee19ac9aba1` sau #316. Lượt chạy sau sửa
Android dùng `27cc2dfe6f59becb268a5e785a64bf4aa4c91114`. Phạm vi `screen.explore` và
`screen.desk`, hai scenario `empty` và `no-results` của diagnostic hosts.
Review trực tiếp capture theo LayoutSpec R1, `fidelity: NO_APPROVED_REFERENCE`.
[LayoutSpec](../../../../design/empty-artwork-cmp-261.md) · [Bằng chứng gốc](../README.md) ·
[Audit 17/11](../../empty-state-audit-261/README.md).

## Môi trường và lệnh

Mac arm64, macOS 15.6 (24G84), 24 GiB RAM, 43 GiB trống lúc bắt đầu. JBR 21.0.11, Gradle
8.14.3, Compose Multiplatform 1.9.0. Bootstrap bằng
`./tools/local-budget run --timeout 600 -- cargo build --locked -p xtask` với pressure normal.
Desktop dùng checkout sạch riêng cùng SHA. Gradle và hai AVD chạy theo lượt ngoài launcher,
với Gradle tối đa 2 workers và mỗi AVD 2 CPU, 2 GiB RAM.

| Boundary | Lệnh | Kết quả |
| --- | --- | --- |
| Asset canonical | `node apps/vink-web/assets/empty/verify.mjs` và `node --test apps/vink-web/assets/empty/verify.test.mjs` | PASS, 55.704 byte cho ba WebP 512×427 có alpha, 3/3 validator test. [Log](asset-check.log) |
| Desktop/common, Skia JVM | `VINK_EMPTY_CAPTURE_DIR=<thư-mục-capture> cargo xtask ui-preview cmp verify` | PASS 110/110, 0 skip, 48 ảnh #261. [Runner](desktop/verify.log) · [Test counts](desktop/suite-results.json) · [Context](desktop/capture-context.json) |
| Android 7.0/API 24, `NYC/8695085`, Pixel 2 AVD arm64-v8a `vink261-api24` | `ANDROID_SERIAL=emulator-5564 cargo xtask ui-preview android verify` | FAIL 1/8 ở baseline và sau sửa. Ca #261 đọc pixel và CTA PASS qua BitmapFactory. [Thiết bị](android-24/device.log) · [Baseline](android-24/instrumentation.xml) · [Runner sau sửa](android-24/verify-after.log) · [JUnit sau sửa](android-24/instrumentation-after.xml) |
| Android 15/API 35, `AE3A.240806.019/12368160`, Pixel 2 AVD arm64-v8a `vink261-api35` | `ANDROID_SERIAL=emulator-5562 cargo xtask ui-preview android verify` | FAIL 1/8 ở baseline, lần lặp và sau sửa. Ca #261 đọc pixel và CTA PASS qua ImageDecoder. [Thiết bị](android-35/device.log) · [Baseline](android-35/instrumentation.xml) · [Runner sau sửa](android-35/verify-after.log) · [JUnit sau sửa](android-35/instrumentation-after.xml) |
| iOS Kotlin/Native và XCTest | `VINK_UI_NATIVE_TARGETS=1 cargo xtask ui-preview cmp verify`, sau đó `VINK_UI_IOS_DEVICE=<UDID> cargo xtask ui-preview ios verify` | NOT_RUN. Máy có Command Line Tools. `xcodebuild` yêu cầu Xcode, `simctl` và XcodeGen vắng mặt. [Preflight](ios-prerequisite.log) |

Cả hai API Android đều chạy đủ 8 ca instrumentation. Mẫu `composition.interaction/sheet`
thất bại ở `interactionImeClearAndSheetKeepControlledText`. Sau Search, assertion thấy
“Đã tìm 0 lần” thay vì “Đã tìm 1 lần”. Kết quả lặp lại trên cùng API 35 ở baseline và trên
hai API sau sửa. Assertion mới về focus ban đầu trong
`controlledVietnameseTextSurvivesThemeAndRecreation` PASS ở cả hai API.
[Lượt lặp API 35](android-35/instrumentation-retry.xml).

## AC-04 · Decode và fallback

Ca `exactDecorativeAlphaWebpDecodesAndKeepsRecovery` đọc trực tiếp pixel đầu ra của từng asset.
Cả hai API trả ảnh 192×160, allocation bitmap 122.880 byte và alpha tối đa 254/255
ở mẫu thu nhỏ. Bảng là số pixel alpha 0 và số pixel alpha nằm giữa 0 và 1.
[Pixel Desktop](desktop/EmptyArtworkTest.xml) · [Log API 24 sau sửa](android-24/alpha-after.log) ·
[Log API 35 sau sửa](android-35/alpha-after.log).

| Asset | API 24 · trong suốt / bán trong suốt | API 35 · trong suốt / bán trong suốt |
| --- | ---: | ---: |
| no-results | 22.290 / 8.430 | 22.176 / 8.544 |
| no-public-works | 22.814 / 7.906 | 22.740 / 7.980 |
| first-draft | 22.455 / 8.265 | 22.348 / 8.372 |

Desktop kiểm `EmptyArtworkPanel(null)` vẫn đọc được câu trạng thái và bấm được CTA.
Android có ca lỗi ảnh bìa opaque `identity.image-frame/image-error` đang chạy thật.
Thử buộc lỗi decoder của chính artwork alpha trên Android và kiểm tra pixel iOS còn `NOT_RUN`.
[Fallback API 24](android-24/opaque-image-error-390-light-100.png) ·
[Fallback API 35](android-35/opaque-image-error-390-light-100.png).

## AC-06 · Ma trận và thao tác

Ma trận baseline mỗi renderer gồm 48 ca, từ 2 màn × 2 trạng thái × 320/390/834dp × sáng/tối ×
chữ 100/200%. Desktop dùng `ImageComposeScene` 1000dp cao, density 1, phông chữ TTF
PreviewFonts và multiplier 1/2. Android dùng font hệ thống, 160dpi, viewport cao 680dp
ở 320/390dp và 912dp ở 834dp. `settings put system font_scale` đặt 1.0/2.0.
Đọc lại cấu hình Activity. Multiplier của diagnostic giữ 1. Android 7 dùng
`-skin <width>x<height>` để khớp framebuffer với viewport. Android 15 dùng `wm size`.

Dùng `adb -s <serial> shell am start` với extras `entry`, `scenario`, `theme` và
`scale=100`, rồi kéo bằng `input swipe` khi cần. Đọc rect CTA từ cây UI Automator khi nút
đã vào màn hình và đối chiếu biên thanh hệ thống từ `dumpsys window displays` hoặc
`dumpsys window policy`. Bấm CTA tại các ca 200% ở 320 và 834dp, rồi đọc thông báo
intent cục bộ. Capture chụp bằng `adb exec-out screencap -p`. Hình ghép là để mở xem,
còn rect đo đến từ thiết bị. [Ma trận API 24](android-24/matrix.json) ·
[Ma trận API 35](android-35/matrix.json).

| Renderer | CTA và biên hệ thống | Focus |
| --- | --- | --- |
| Desktop | PASS bố cục và cuộn tới CTA ở 200% qua Compose test. [JUnit](desktop/EmptyArtworkPreviewTest.xml) | Input, selection, focus và callback trong Compose |
| Android API 24 | Baseline 48/48 rect nằm trong vùng an toàn, 16/16 tap có xác nhận intent sau khi đóng bàn phím. Vùng an toàn theo Y là 24–632px ở 320/390dp và 24–864px ở 834dp. | Baseline tự focus. Hai ca chạy lại ở 390dp sau sửa mở với ô tìm kiếm chưa focus, chạm vào ô mở bàn phím, đổi theme giữ focus và query. [Trước](android-24/focus-before.json) · [Sau](android-24/focus-after.json) |
| Android API 35 | 48/48 rect nằm trong vùng an toàn, 16/16 tap có xác nhận intent. Vùng an toàn theo Y là 24–632px ở 320/390dp và 24–856px ở 834dp. | 2/2 ca giữ focus và chuỗi nhập ASCII qua đổi theme. [Readback](android-35/focus-and-opaque-fallback.json) · [Ảnh](android-35/focus-explore-390-100-dark.png) |

Lượt đầu trên Android 7 giữ framebuffer 1080×1920 dù đã đổi `wm size`. Sau khi đặt
kích thước framebuffer, lượt thử tiếp theo còn bàn phím mở giữa các lần launch. Kiểm lại
với frame trùng viewport, đóng bàn phím còn mở và kéo trong vùng biên của app.
[Log các lần thử](android-24/capture-preflight.log).
Ở mẫu tối API 35 320dp, biểu tượng nav 3 nút của AVD có tương phản 2,30:1 ở điểm lấy mẫu
(`#575862` trên `#201F2B`). Rect CTA vẫn nằm trên thanh nav và bấm được.
[Xem mẫu tối trong hàng dưới](android-35/320-200.png).

### Focus lúc mở trên Android 7

Baseline mở ô tìm kiếm và bàn phím trong cả hai ca tái hiện ở 390dp. Bàn viết chữ 200% tự cuộn
qua phần mở đầu. Android trước API 28 có thể tự chọn focus ban đầu trong touch mode. Host
Android giờ nhận focus ở container khi tạo Activity mới trên API 24–27, trước khi Compose
gắn ô tìm. Kết quả runtime của đường này được đo ở API 24.
[Hành vi Android](https://developer.android.com/about/versions/pie/android-9.0-changes-28#view-focus) ·
[Host](../../../../../packages/ui-system/cmp/androidDiagnostic/src/main/kotlin/vink/ui/diagnostic/DiagnosticActivity.kt) ·
[SHA-256 nguồn APK dùng cho ca tái hiện](android-24/focus-source-sha256.txt).

Hai ca tái hiện dùng `am force-stop`, thao tác chạm ở launcher, rồi `am start -W` với
`screen.explore/no-results` ở 100% và `screen.desk/no-results` ở 200%. Cả hai dùng khung
390×820dp, mở theme sáng, chạm ô tìm kiếm, nhập chuỗi ASCII và đổi theme.
[Bàn viết trước sửa](android-24/focus-before-desk-390-200.png) ·
[Bàn viết sau sửa](android-24/focus-after-desk-390-200.png) ·
[Khám phá sau đổi theme](android-24/focus-after-explore-dark.png).

Sau sửa đã mở lại 16 ảnh ở 320/834dp × sáng/tối × chữ 200% cho đủ hai màn và hai trạng thái.
Bàn phím vẫn đóng khi mở cả 16 ca. Mọi CTA đều nằm trọn trên thanh nav sau khi cuộn
và cả 16 lượt bấm trả đúng intent. Hai ca Bàn viết 320dp cần một lần kéo thêm để thấy
trọn nút hai dòng. [Số đo và thao tác](android-24/after-matrix.json) ·
[320dp sau sửa](android-24/after-320-200.png) · [834dp sau sửa](android-24/after-834-200.png).

### Ảnh đã mở

| Renderer | 320dp | 390dp | 834dp |
| --- | --- | --- | --- |
| Desktop | [100%](desktop/320-100.png) · [200%](desktop/320-200.png) | [100%](desktop/390-100.png) · [200%](desktop/390-200.png) | [100%](desktop/834-100.png) · [200%](desktop/834-200.png) |
| Android API 24 | [100%](android-24/320-100.png) · [200%](android-24/320-200.png) | [100%](android-24/390-100.png) · [200%](android-24/390-200.png) | [100%](android-24/834-100.png) · [200%](android-24/834-200.png) |
| Android API 35 | [100%](android-35/320-100.png) · [200%](android-35/320-200.png) | [100%](android-35/390-100.png) · [200%](android-35/390-200.png) | [100%](android-35/834-100.png) · [200%](android-35/834-200.png) |

Mỗi cụm baseline gồm cả theme sáng và tối. Đối chiếu thêm Bàn viết 320dp tối, chữ 200% từ khi mở
đến khi nhận xác nhận intent. [API 24 lúc mở](android-24/desk-empty-320-dark-200-initial.png) ·
[API 24 sau khi bấm](android-24/desk-empty-320-dark-200-result.png) ·
[API 35 lúc mở](android-35/desk-empty-320-dark-200-initial.png) ·
[API 35 sau khi bấm](android-35/desk-empty-320-dark-200-result.png) · [Hash ảnh gốc baseline](capture-sha256.txt).

## AC-07 · Kết quả theo chiều và phần còn thiếu

| Chiều | Desktop/common | Android API 24 | Android API 35 | iOS |
| --- | --- | --- | --- | --- |
| Functional | PASS ca #261, asset và action | PASS ca #261 và ảnh bìa lỗi | PASS ca #261 và ảnh bìa lỗi | NOT_RUN |
| Geometry | PASS slot 176/240dp, fit 512:427 và trục giữa | PASS rect CTA và biên thanh hệ thống | PASS rect CTA và biên thanh hệ thống | NOT_RUN |
| Visual | PASS theo LayoutSpec qua ma trận đã mở | PASS thứ tự hình/câu/CTA. Focus ban đầu đã kiểm lại qua ảnh trước và sau | PASS phần artwork/copy/CTA, ghi nhận tương phản thanh nav hẹp ở theme tối | NOT_RUN |
| Accessibility/Responsive | PASS reflow ở multiplier 2 và semantics của CTA | PASS reflow OS fontScale 2 và tên CTA trong cây, TalkBack NOT_RUN | PASS reflow OS fontScale 2 và tên CTA trong cây, TalkBack NOT_RUN | NOT_RUN |
| Interaction | PASS test focus, selection, cuộn và callback | PASS cuộn/tap/giữ focus của #261 sau sửa. FAIL một ca Search ở specimen Interaction | PASS cuộn/tap/giữ focus của #261. FAIL một ca Search ở specimen Interaction | NOT_RUN |

AC-04/06/07 vẫn **PARTIAL** và [#261](https://github.com/loveoverflowcom/vink/issues/261)
giữ mở. Dynamic Type, VoiceOver, TalkBack, IME tiếng Việt có composition trên hệ điều hành
và thiết bị vật lý đều `NOT_RUN`. Các thao tác Android ở trên dùng input shell và cây
accessibility đọc từ emulator. Cần máy có Xcode, XcodeGen và Simulator để chạy hai lệnh iOS
với UDID đã xác minh.
