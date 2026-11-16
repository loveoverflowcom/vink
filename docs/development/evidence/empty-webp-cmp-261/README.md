# CMP empty artwork #261 · local evidence

Base đã fetch/reconcile: `develop@57737a788869fbab556971d0b820cbb0fef64099` (sau #260).
Branch: `codex/cmp-empty-webp-states`. Implementation source/asset hashes trong
[source-integrity.json](source-integrity.json); PR head/merge pin được ghi trong GitHub handoff.
Tự chạy 16/11/2026 UTC, macOS 15.6 arm64, JBR 21.0.11, Gradle 8.14.3, Kotlin 2.2.20,
CMP 1.9.0/Skiko 0.9.22.2. Cwd: root checkout Vink của task #261.
Review: **self-review**, `fidelity: NO_APPROVED_REFERENCE`; không tự duyệt golden.

Implementation là **cmp diagnostic**, không app native sản phẩm. [LayoutSpec R1](../../../design/empty-artwork-cmp-261.md).
CHOICE-01 A dùng exact alpha WebP qua closed diagnostic port; CHOICE-02 A giữ composition
ở diagnostics, không mở API ui/patterns. B (opaque derivatives theo theme) không cần cho
Desktop vì decoder thật giữ alpha; native qualification của A còn pending. Không tự suy ra
native PASS hoặc đổi sang derivative chỉ để che môi trường thiếu thiết bị.

| Check thực chạy | Kết quả | Evidence / giới hạn |
| --- | --- | --- |
| `./tools/local-budget run --timeout 600 -- cargo build --locked -p xtask` | PASS | [log](xtask-build.log), 40,78s, 2 Cargo jobs; chỉ bootstrap runner |
| `cargo xtask ui-preview cmp verify` | PASS | [log](cmp-verify.log), 3m57s; token drift + 110 tests: ui 43, patterns 5, diagnostics 8, preview 54, không skip; [summary](desktop-summary.json) |
| Real alpha decoder, both source/display sizes, opaque-port rejection, cancellation | PASS Desktop/JVM | [pixel output](alpha-pixels.xml); không dùng opaque fixture làm bằng chứng alpha |
| `node apps/vink-web/assets/empty/verify.mjs` | PASS | [asset log](assets.log), [SHA/byte readback](asset-source-check.json); không phải pixel decode |
| `node --test apps/vink-web/assets/empty/verify.test.mjs` | PASS 3/3 | [negative validator tests](asset-validator-tests.log) |
| `cargo xtask ui-preview android build` | PASS | [log](android-build.log); Android SDK 36, minSdk24, APK thật. Không chứng minh Android runtime |
| `cargo xtask ui-preview android verify` | **FAIL prerequisite** | [log](android-verify.log): test APK compile/package xong, `connectedDebugAndroidTest`: **No connected devices!**; instrumentation execution NOT_RUN |
| Isolated Android35 emulator boot | FAIL prerequisite | Mặc định cần 7372,8MB, ổ đĩa còn khoảng 5846MB. Thử data ext4 1GiB riêng + metadata template: init thiếu first-stage mount devices, boot loop/offline. [disk failure](android-boot-fail.log), [kernel log](android-key-fail.log.gz). Chỉ emulator task `vink261-api35`, port5562 được tạo/dừng; không dùng AVD/dữ liệu có sẵn |
| Android API24–27 BitmapFactory runtime | NOT_RUN | Không có image/device API24; allowlist content vẫn hai exact fixtures, artwork là closed enum riêng |
| Android API28+ ImageDecoder runtime | NOT_RUN | API35 emulator không boot; build không thay pixel proof |
| `xcrun simctl list devices` | FAIL prerequisite | [log](ios-prerequisite.log): không có simctl/Xcode; iOS common/host build/runtime NOT_RUN |
| `swiftc -frontend -parse …/DiagnosticTests.swift` | PASS syntax only | [log rỗng](ios-swift-parse.log); không typecheck XCTest hay build framework |
| Real OS fontScale/Dynamic Type, OS IME, TalkBack/VoiceOver, physical device | NOT_RUN | Preview multiplier 1/2 không thay OS setting |
| RSS/heap/latency benchmark | NOT_RUN | Output size cap là arithmetic, không là memory measurement |
| Quint/Kani/backend/Web product lanes | NOT_RUN, không cần cho delta này | Không đổi domain/RegionState/API/persistence/authority, chỉ diagnostic pixels và callback local; coroutine cancellation được kiểm tại seam thật |
| GitHub CI / VPS deploy | NOT_RUN | Không đọc/chờ CI, không deploy |

Các lệnh Java/Gradle dùng `JAVA_HOME=/Users/manhblue/Library/Java/JavaVirtualMachines/jbr-21.0.11/Contents/Home`.
Native thêm `ANDROID_HOME=/Users/manhblue/Library/Android/sdk`, `GRADLE_OPTS=-Dorg.gradle.workers.max=2`.
CMP/native không được local-budget launcher hỗ trợ; chạy riêng sau bootstrap có admission,
không chạy chồng workload nặng khác. Emulator riêng tối đa 2 core/2GiB, đã dừng trước native build.

## Real pixels / asset boundary

Dùng đúng ba tệp canonical của #258/#259, 512×427, VP8X+ALPH+VP8, một frame,
tổng **55.704 bytes**, mỗi tệp <30KiB. Không ảnh mới, không runtime Drive/network.
SHA đầy đủ tại [asset-source-check.json](asset-source-check.json). Gradle kiểm bytes/dimensions/hash
rồi sinh data trong build output; không check-in bộ tranh/manifest thứ hai.

Desktop `toPixelMap` ở 192×160:

| Asset | Transparent pixels | Translucent pixels | Max alpha |
| --- | ---: | ---: | ---: |
| no-results | 22.176 | 8.544 | 0,99607843 |
| no-public-works | 22.740 | 7.980 | 0,99607843 |
| first-draft | 22.348 | 8.372 | 0,99607843 |

Source-size decode 512×427 cũng PASS, có alpha 0, alpha bán trong suốt và alpha 1.
Opaque validator/`decodeContentImage` từ chối chính các bytes alpha này. Android giữ nguyên
validator + allowlist hai opaque fixtures; helper BitmapFactory dùng chung thuật toán scaling
nhưng artwork chỉ đi vào qua enum sinh từ bytes được kiểm hash. Skia dùng eager codec.
Cancel sau real decode/trước consumer delivery không publish stale result.

## Captures và review

[Before](before/) tại base + capture test mới; [after](after/) cùng UI source cuối (hash ở
source-integrity), 48 PNG mỗi phase: 2 screens × 2 states × 320/390/834dp × Light/Dark × 100/200%.
`ImageComposeScene`, density1, 1000dp viewport, padding16dp, vertical scroll, insets0,
pinned TTF PreviewFonts; không phải native viewport/window. Context JSON đi cùng mỗi phase.
[Capture SHA](capture-integrity.json) giữ originals. [12 review sheets](review-sheets/) đã mở đủ;
chỉ ghép bản gốc để review (834dp hiển thị ở 50%), không thay ảnh raw hay dùng sheet đo geometry.
[Script ghép](review-sheets.swift.txt). Mở thêm raw Desk/320/Dark/200 và Explore/390/Light/100.

| Chiều | Desktop result | Claim / giới hạn |
| --- | --- | --- |
| Functional | PASS | empty vs no-results đúng copy/asset; clear query và sample editor intent thật; non-empty/load/error/auth/forbidden/pending/retained không có artwork empty |
| Geometry | PASS | 176/240dp, aspect512:427 ±1px rounding; actual 320/390/834dp, text1/2; không đo từ thumbnail |
| Visual | PASS theo R1 | Đã mở before/after cả ma trận; hình nhỏ, không crop/halo/card, một câu + CTA rõ; artwork candidate, không có approved CMP reference |
| Accessibility/Responsive | PASS subtree kiểm được | Decorative không ContentDescription, text/nút tồn tại khi image null, 200% reflow/scroll tới CTA; native safe-area/AT/Dynamic Type NOT_RUN |
| Interaction | PASS JVM/Compose | query, selection và focus giữ qua region/theme/text-scale; CTA một callback; fallback vẫn click được. OS IME/screen reader/native feel NOT_RUN |

`EmptyArtworkPreviewTest` kiểm cả partial/stale/refresh/uncertain và disabled/auth scenarios bằng
cùng ScreenRecipe thật. Không đổi shared RegionState hay mô hình nghiệp vụ; không gửi server.
Không có fix UI sau lượt render after; các sửa tiếp là oracle bounds/test API và docs/native tests.

## Failures được giữ

[Initial alpha FAIL](focused-initial-fail.log), [XML](alpha-initial-fail.xml): oracle từng đòi
pixel alpha đúng1 ở ảnh thu nhỏ. Resampling thật cho max254/255; oracle cuối kiểm alpha0,
alpha bán trong suốt và nét max>0,8, source-size có alpha1. Không đổi decoder để làm test xanh.
[Geometry FAIL](geometry-clipped-fail.log), [XML](geometry-clipped-fail.xml): đo clipped viewport
bounds khi nội dung cao ở 200%; sửa test sang **unclipped** Dp bounds, giữ assertion aspect/size.
Các compile FAIL do gọi Compose waitUntil/DpRect test API sai được giữ ở `*-compile-fail.log`.
Final focused interaction và full CMP lane đều PASS, không che FAIL native prerequisite.

## AC mapping và resume

| AC | Kết luận | Còn thiếu |
| --- | --- | --- |
| AC-01 | PASS | Hash/bytes/provenance đúng nguồn #258, một asset owner |
| AC-02 | PASS tại diagnostic Desktop | Đúng 4 states và callback/semantics thật |
| AC-03 | PASS | Non-fresh states không bị tráo empty; không service/save/auth giả |
| AC-04 | PARTIAL | Desktop alpha thật + native compile xong; **Android24/28+ và iOS pixel runtime NOT_RUN** |
| AC-05 | PASS subtree Desktop/source | Decorative/Fit/text/CTA/input identity được kiểm; OS AT thuộc AC-07 |
| AC-06 | PARTIAL | Desktop ma trận before/after opened + scroll PASS; **native fontScale/safe-area/nav occlusion NOT_RUN** |
| AC-07 | PARTIAL | Boundary/commands/gaps tách rõ; thiếu native runtime như trên |
| AC-08 | PASS | Docs/spec/catalog IDs hiện hữu và source/evidence khớp; một CMP PR, không đổi Web source/token/theme |

**Giữ #261 OPEN**, AC-04/06/07 unchecked; merge implementation không thay native qualification.
Không đóng #116 hoặc suy thành native product acceptance.

Để tiếp tục: có Android emulator/device API24 và API28+ boot hoàn chỉnh, đủ disk; chạy tuần tự
`ANDROID_SERIAL=<serial> cargo xtask ui-preview android verify` (test mới đọc pixel alpha cả ba,
CTA/text200/theme), mở capture trên từng serial, kiểm thêm real OS fontScale1/2 và insets.
Có Xcode + simulator + XcodeGen: `VINK_UI_NATIVE_TARGETS=1 cargo xtask ui-preview cmp verify`
(bao gồm real `EmptyArtworkTest` iosSimulatorArm64), rồi
`VINK_UI_IOS_DEVICE=<UDID> cargo xtask ui-preview ios verify` (XCTest mới 8 ca empty/recovery
ở text200, light/dark). Test source đã có; chưa chạy không thành PASS.
