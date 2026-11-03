# CMP owners and evidence

Owner map checked against Vink `8045d45` on 03/11/2026. Re-read current files before
claiming implementation or choosing checks; this snapshot is a starting point.

| Concern | Source |
| --- | --- |
| Targets, inclusion, dependency pins | [settings](../../../../packages/ui-system/cmp/settings.gradle.kts), [root build](../../../../packages/ui-system/cmp/build.gradle.kts), [UI build](../../../../packages/ui-system/cmp/ui/build.gradle.kts) |
| Common primitives and generated values | [UI commonMain](../../../../packages/ui-system/cmp/ui/src/commonMain/kotlin/vink/ui) — `VinkTheme.kt`, `VinkValues.kt`, `VinkLayout.kt`, `VinkActions.kt`, `VinkFields.kt`, `VinkIdentity.kt`, `VinkMedia.kt` |
| State/navigation policy and components | Same directory: `VinkState.kt`, `VinkFeedback.kt`, `VinkNavigation.kt`; shared [states](../../../../packages/ui-system/shared/states.md) and [navigation](../../../../packages/ui-system/shared/navigation.md) |
| Literary compositions | [patterns CMP](../../../../packages/vink-patterns/cmp), [pattern contract](../../../../packages/vink-patterns/README.md) |
| One set of diagnostic specimens | [diagnostics commonMain](../../../../packages/ui-system/cmp/diagnostics/src/commonMain/kotlin/vink/ui/preview) — `Workbench.kt`, `Components.kt`, `StateSpecimens.kt`, `NavigationSpecimens.kt`, `ScreenRecipes.kt`, `DiagnosticHost.kt` |
| Desktop window/fonts and render tests | [Desktop preview](../../../../packages/ui-system/cmp/preview/src/desktopMain/kotlin/vink/ui/preview), [desktopTest](../../../../packages/ui-system/cmp/preview/src/desktopTest/kotlin/vink/ui/preview) |
| Android debug host and instrumentation | [Android host](../../../../packages/ui-system/cmp/androidDiagnostic); `DiagnosticActivity.kt`, `DiagnosticTest.kt`, actual manifest/build |
| iOS simulator composition and XCTest | [iOS host](../../../../packages/ui-system/cmp/iosDiagnostic); `project.yml`, `Sources/App.swift`, `Tests/DiagnosticTests.swift`; Kotlin controller in diagnostics `iosMain` |

The Gradle root includes `:patterns` from outside the CMP tree. Keep that source
owner; do not create a second patterns module. Diagnostic dependencies do not
belong in a future production consumer. Current `commonMain` import guards reject
`java.*`, `javax.*`, `android.*`, `platform.*`; platform normalization and image
decoding use their existing target adapters.

## Select checks and artifacts

Exact commands and prerequisites live in the
[development guide](../../../../docs/development/README.md#diagnostic-ui-native-uis-06)
and [CMP README](../../../../packages/ui-system/cmp/README.md). Do not infer tasks
from another Kotlin project. Current pins are Kotlin 2.2.20, CMP 1.9.0, Material3
1.8.2, Gradle 8.14.3/JDK 17+, native opt-in with Android SDK 36 and Xcode for iOS.
Manifests win when these change.

| Claim | Boundary needed | Limits/artifacts |
| --- | --- | --- |
| Pure region/overlay/identity policy | Existing `commonTest` vectors, through CMP verify | No OS or rendering claim |
| Shared Compose layout, input, semantics, callbacks | CMP verify: common + Desktop tests; open relevant `preview/build/captures` PNGs and actual context sidecar | JVM/off-screen, no OS window, native IME or screen reader |
| Web counterpart or shared-token change | Token drift/quick + Leptos preview; Web verify when app consumer changes | Leptos pixels are browser evidence |
| Android compilation | Native-enabled build with actual SDK | No installed host or interaction claim |
| Android runtime/insets/recreation/Back | Android diagnostic verify/run on a named emulator/device | Instrumentation output in `androidDiagnostic/build/outputs/androidTest-results`; screenshots from that same serial |
| iOS simulator runtime | iOS diagnostic build/verify/run with named `VINK_UI_IOS_DEVICE` | XCTest `.xcresult` under `iosDiagnostic/build/DerivedData/Logs/Test`; simulator screenshots retain UDID/OS |
| OS IME, TalkBack/VoiceOver, device lifecycle | Actual named OS input/screen-reader/device sequence | Programmatic input and accessibility tree dumps do not substitute |

Current diagnostic replay uses shared catalog IDs and bounded `entry`, `scenario`,
`theme`, `motion`, `scale` options. Native commands consume `VINK_UI_ENTRY`,
`VINK_UI_SCENARIO`, `VINK_UI_THEME`, `VINK_UI_MOTION`, `VINK_UI_SCALE`; Desktop uses
the documented launch arguments. These are sample inputs, not a native product
router. Preview `scale=200` multiplies OS font scale; record both separately.

Preserve context per image. Existing CMP sidecars differ by test; inspect their
actual fields and supplement missing provenance rather than claiming one universal
schema. Record `capture_mode=offscreen` only for the off-screen renderer. Keep raw
artifacts, focused captures and inspected flags distinct. Drive upload is a
separate state and requires readback through its existing workflow.

## Selective Cantos provenance

Studied `cantos-cmp-mobile` at Cantos commit
`ce8ac58be662342e68f3d9e171be307efae44d9c` (local checkout), particularly owner/source-set
mapping, controlled inputs, injected preferences/lifecycle and evidence by host.
Adapted these methods to Vink's libraries and diagnostic hosts. No Cantos player,
media session, downloads/outbox, auth, localization catalog, Compose Preview CLI,
module names or verification commands are inherited. Those are separate products.
