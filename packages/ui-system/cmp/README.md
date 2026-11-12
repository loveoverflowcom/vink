# UI system — Compose Multiplatform renderer

Gradle project riêng, không phụ thuộc backend hay app. Phiên bản pin (kiểm tương thích trên máy
dev ngày 02/11/2026, khớp tổ hợp đang chạy ở Cantos — chỉ học, không chép build):

| Thành phần | Pin | Ghi chú |
| --- | --- | --- |
| Gradle | 8.14.3 (wrapper + `distributionSha256Sum`) | `org.gradle.java.installations.auto-download=false` |
| JDK | 17+ của máy | Không tự tải toolchain |
| Kotlin | 2.2.20 | `kotlin("multiplatform")`, compose compiler plugin cùng phiên bản |
| Compose Multiplatform | 1.9.0 | Material3 1.8.2 ổn định, không dùng API Expressive alpha |
| AGP | 8.11.1 | Chỉ áp dụng khi `-Pvink.ui.nativeTargets=true`; `compileSdk` 36, `minSdk` 24 |

| Module | Target | Nội dung |
| --- | --- | --- |
| `:ui` | `desktop` (JVM) luôn bật; `android`, `iosArm64`, `iosSimulatorArm64` khi bật native | `commonMain`: `VinkTokens.kt`/`VinkCatalog.kt`/`VinkIcons.kt`/`VinkIdentityVectors.kt` (**sinh ra**), `VinkValues.kt`, `VinkTheme.kt` (`VinkTheme`, `VinkScope`, `LocalVink`, `RoleOverride`, `validate`), component theo họ: `VinkLayout.kt` (`VinkText`, `VinkHeading`, `VinkSurface`, `VinkContainer`, `VinkStack`, `VinkGrid`, `VinkDivider`), `VinkActions.kt` (Button pending có MicroArc ở slot status), `VinkFields.kt`, `VinkMedia.kt`, `VinkIdentity.kt` (`initials`, `tone`, `safeHref`, `expect fun nfc`); UIS-03: `VinkState.kt` (contract thuần: trục nội dung/pha, `RegionState.layout`, `Problem`, `placementFor`, `NoticeSpec`, `LoadingGate`…; `VinkStateVectors.kt` **sinh ra**) và `VinkFeedback.kt` (`VinkContentRegion`, `VinkStatePanel`, `VinkInlineNotice`, `VinkBanner`, `VinkSkeleton`, `VinkLoadingIndicator`, `VinkEdgeProgress`, `VinkStatusBadge`, `VinkListEnd`, `VinkSnackbar`, `VinkDialog`, `VinkMotionActivity`, `rememberLoadingGate`, `rememberSnackbarTimeout`); UIS-04: `VinkNavigation.kt` (`WindowClass.fromDp`, `NavLayout.resolve`, `RailPreference`, `OverlayLayer`/`CloseReason`/`FocusReturn` thuần; `VinkNavItem`, `VinkNavUtility`, `VinkNavigationRail`, `VinkBottomNavigation`, `VinkTopBar`, `VinkAdaptiveScaffold` gọi `content` từ một call site cố định nên đổi khung không remount; `VinkNavigationVectors.kt` **sinh ra**); LVE #73: `VinkToggle.kt` (`ToggleValue`, `TogglePhase`, `CountFact`, `ToggleView.resolve`, `ToggleCopy`, `VinkToggleAction`; `VinkToggleVectors.kt` **sinh ra**); `desktopMain`/`androidMain`/`iosMain`: `nfc` theo nền tảng |
| `:preview` | `desktop` | Workbench Compose Desktop, `PreviewFonts` (đọc WOFF2 tự host qua Skia), `StateSpecimens.kt` (họ `state.*`, playground có đồng hồ thủ công và reducer `Submission`), `NavigationSpecimens.kt` (họ `navigation.*` và adapter chẩn đoán `DiagnosticNavigator`: đích có kiểu, back stack, deep link nội bộ, chữ chưa lưu do host giữ — không phải app native), test và capture; `Main.kt` map cửa sổ thu nhỏ → `hostActive = false` (không vòng lặp nào chạy) |

Từ UIS-06, `Workbench.kt`, `Components.kt`, `StateSpecimens.kt`, `NavigationSpecimens.kt`,
`ScreenRecipes.kt` nằm ở `:diagnostics/commonMain`, không còn bản sao trong `:preview`.
`:preview` chỉ giữ Desktop window/fonts/tests. `:androidDiagnostic` là debug application host;
`iosDiagnostic/project.yml` sinh Xcode simulator host gọi `NativeSession.controller()` từ cùng
module. Settings chỉ include Android host khi native opt-in; Web/JVM không cần SDK/Xcode.
Decoder ảnh nằm trong diagnostic expect/actual, không đưa Skia/BitmapFactory vào UI commonMain.

Android inject lifecycle và theo dõi Animator duration scale; iOS inject app active và Reduce
Motion notification. UI giữ một call site specimen qua theme/utility; trường controlled dùng
`rememberSaveable(TextFieldValue.Saver)` ở sample để giữ chữ qua Android recreation. Đây không là
local draft recovery hay backend. Bộ nhập thực dùng public `VinkTextField`/`VinkTextArea`.

Lệnh native, replay, SDK/simulator prerequisites và output:
[development](../../../docs/development/README.md#diagnostic-ui-native-uis-06).
Readiness theo target: [UIS-06](../../../docs/development/ui-system-qualification-evidence.md).

Interaction foundation (motion chip/overlay, field IME/focus, dialog Back/lifetime):
[contract và findings](../../../docs/design/ui-interaction-foundation.md),
[bằng chứng theo target](../../../docs/development/ui-system-qualification-evidence.md#interaction-foundation-12112026).
Recipe `composition.interaction` dùng component xuất ra thật; vẫn chỉ là diagnostic, không phải app native.

`commonMain` không import `java.*`, `javax.*`, `android.*`, `platform.*` (kiểm trong `verify quick`);
API nền tảng (NFC) đi qua `expect`/`actual`. Text field nhận/trả `TextFieldValue`; ảnh là `ImageState`
đã giải mã do host cấp (preview giải mã bằng Skia, lỗi → `Failed`). Contract: [components.md](../shared/components.md).
Không có target Web Compose: Web là Leptos. Không dùng WebView.

```kotlin
VinkTheme(VinkPreferences(theme = ThemePreference.System), systemReducedMotion = hostReducedMotion) {
    VinkScope(theme = ThemePreference.Dark) {
        val vink = LocalVink.current
        Text("Chữ mẫu", style = vink.text(VinkTextRoles.Reading), color = vink.colors[ColorRole.OnSurface])
    }
}
```

Font: thư viện mặc định `FontFamily.SansSerif/Serif/Monospace`; host inject `VinkFonts`. Preview
desktop nạp Be Vietnam Pro 400/600 và Literata (variation `wght`) từ TTF giải nén lossless của
WOFF2 Web (Gradle sao derivative đã pin vào `build/generated/preview-fonts`). Nguồn, OFL, hash
và cách tái tạo có một owner tại [font assets](../../../apps/vink-web/assets/fonts/README.md).
`FontTest` kiểm metadata/glyph và đo font qua Compose; `FontCaptureTest` giữ 24 ảnh đối chiếu
typography/byline/reader ở light/dark, chữ 100/200% và sidecar `font-contexts.json`.
Android/iOS chưa có pipeline
TTF/OTF có provenance: dùng font hệ thống, ghi `UNSUPPORTED` cho tới khi có.

Lệnh: `cargo xtask ui-preview cmp build|run|verify` (xem [README UI system](../README.md)).
Ảnh: `preview/build/captures/*.png` + `contexts.json` (density 1, font scale 1/2, theme hệ thống
cố định sáng): 30 ảnh nền tảng + 132 ảnh component UIS-02/03 (sáng/tối × hẹp/rộng) + 15 trạng thái/chữ
200% UIS-02 + 29 kịch bản state (sáng, hẹp) + 3 state chữ 200% = 209; cộng 7 ảnh playground sau khi
thao tác (`*--driven--*`, ghi trong `driven-contexts.json`, có lớp dialog) = 216.


EMO-01A: `NotoExpressions.kt` và `NotoExpressionTest.kt` được sinh từ
[manifest biểu cảm](../../expressions/README.md), không phải catalog viết tay thứ hai.
Module `:ui` chỉ hỗ trợ contract lookup và Unicode fallback; poster file là reference tương đối
cho host tương lai. Không có dotLottie Web/native renderer, Rust FFI hoặc product app native.
