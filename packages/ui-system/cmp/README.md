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
| `:ui` | `desktop` (JVM) luôn bật; `android`, `iosArm64`, `iosSimulatorArm64` khi bật native | `commonMain`: `VinkTokens.kt`/`VinkCatalog.kt`/`VinkIcons.kt`/`VinkIdentityVectors.kt` (**sinh ra**), `VinkValues.kt`, `VinkTheme.kt` (`VinkTheme`, `VinkScope`, `LocalVink`, `RoleOverride`, `validate`), component theo họ: `VinkLayout.kt` (`VinkText`, `VinkHeading`, `VinkSurface`, `VinkContainer`, `VinkStack`, `VinkGrid`, `VinkDivider`), `VinkActions.kt` (Button pending có MicroArc ở slot status), `VinkFields.kt`, `VinkMedia.kt`, `VinkIdentity.kt` (`initials`, `tone`, `safeHref`, `expect fun nfc`); UIS-03: `VinkState.kt` (contract thuần: trục nội dung/pha, `RegionState.layout`, `Problem`, `placementFor`, `NoticeSpec`, `LoadingGate`…; `VinkStateVectors.kt` **sinh ra**) và `VinkFeedback.kt` (`VinkContentRegion`, `VinkStatePanel`, `VinkInlineNotice`, `VinkBanner`, `VinkSkeleton`, `VinkLoadingIndicator`, `VinkEdgeProgress`, `VinkStatusBadge`, `VinkListEnd`, `VinkSnackbar`, `VinkDialog`, `VinkMotionActivity`, `rememberLoadingGate`, `rememberSnackbarTimeout`); `desktopMain`/`androidMain`/`iosMain`: `nfc` theo nền tảng |
| `:preview` | `desktop` | Workbench Compose Desktop, `PreviewFonts` (đọc WOFF2 tự host qua Skia), `StateSpecimens.kt` (họ `state.*`, playground có đồng hồ thủ công và reducer `Submission`), test và capture; `Main.kt` map cửa sổ thu nhỏ → `hostActive = false` (không vòng lặp nào chạy) |

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
desktop nạp Be Vietnam Pro 400/600 và Literata (variation `wght`) từ WOFF2 của web (Gradle sao lúc
build vào `build/generated/preview-fonts`, không commit bản sao). Android/iOS chưa có pipeline
TTF/OTF có provenance: dùng font hệ thống, ghi `UNSUPPORTED` cho tới khi có.

Lệnh: `cargo xtask ui-preview cmp build|run|verify` (xem [README UI system](../README.md)).
Ảnh: `preview/build/captures/*.png` + `contexts.json` (density 1, font scale 1/2, theme hệ thống
cố định sáng): 30 ảnh nền tảng + 132 ảnh component UIS-02/03 (sáng/tối × hẹp/rộng) + 15 trạng thái/chữ
200% UIS-02 + 29 kịch bản state (sáng, hẹp) + 3 state chữ 200% = 209; cộng 7 ảnh playground sau khi
thao tác (`*--driven--*`, ghi trong `driven-contexts.json`, có lớp dialog) = 216.
