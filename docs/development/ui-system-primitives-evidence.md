# Bằng chứng UI System 02/06 — component nền Web/CMP

**Ngày:** 02/11/2026 (UTC+07) · **Issue:** [#20](https://github.com/loveoverflowcom/vink/issues/20) ·
**Nhánh:** `codex/ui-system-primitives`, nền `develop@43455a0` (đã gồm UIS-01 #28 và #29; snapshot
của issue là `5afb00b`, không reset). Đây là kết quả chạy trên máy dev, không phải nghiệm thu thiết
kế, thiết bị thật hay production. Contract: [components.md](../../packages/ui-system/shared/components.md);
owner map và khác biệt nền tảng: [packages/ui-system](../../packages/ui-system/README.md).

## Môi trường

| Mục | Giá trị |
| --- | --- |
| Máy | macOS 26.5.2 (25F84), Apple Silicon arm64 |
| Rust / wasm-bindgen | 1.99.1 / 0.2.129 |
| Node / Chrome | v26.12.0 / Chrome 156.0.8078.79 headless (`--headless=new`), DPR 1 |
| JDK / Gradle / Kotlin / Compose | Microsoft OpenJDK 17.0.19 / 8.14.3 / 2.2.20 / Compose Multiplatform 1.9.0, Material3 1.8.2 |
| Android / iOS | Android SDK (`ANDROID_HOME=~/Library/Android/sdk`, AGP 8.11.1) / Xcode 26.6, Kotlin/Native 2.2.20, iOS simulator arm64 |
| PostgreSQL | 17.11 cluster scratch trên `127.0.0.1:55432` (chỉ cho `verify web`) |
| Nguồn | `tokens.json` 0.2.0 (SHA-256 bắt đầu `818e650218752898`), catalog 0.2.0, icon Material Icons `10ecc14` (Apache-2.0), `avatar-identity-v1` |

## HOW / WHY đã ghi trước khi đổi

- **HOW:** text/value → event → caller: field nhận `Signal<String>`/`TextFieldValue`, phát
  `TextInput { value, composing }`/`TextFieldValue` nguyên văn; caller giữ giá trị và validate.
  Avatar: tên → NFC → từ đầu/cuối → initials; key ổn định → FNV-1a → tông; ảnh `Ready` đè lên
  initials, lỗi tải/giải mã quay về initials. Không component nào đọc storage, route hay service.
- **WHY:** controlled state + semantic role giúp hai renderer cùng hành vi mà không cần cùng DOM;
  Cantos `7649017` (Leptos 0.7.8/CMP) chỉ được học phương pháp, không chép code hay phiên bản.
  `appearance.rs` (#7) đã có initials từ đầu + từ cuối; giữ quy tắc đó làm `avatar-identity-v1`.
- **Quyết định scoped trong PR:** icon từ Material Icons (Apache-2.0) vì đủ action hiện có và có
  provenance; path giữ nguyên văn, generator dựng lại SVG upstream để so SHA-256. Avatar tone là role
  riêng (không mượn role trạng thái). CMP field dùng slot `label` của Material thay vì
  `contentDescription` (xem tự review). Không chốt D/INV/S nào, không tạo route profile hay auth.

## Kết quả

| Kiểm | Lệnh | Kết quả | Ghi chú |
| --- | --- | --- | --- |
| Nguồn shared hợp lệ, 9 file sinh ra khớp | `cargo xtask ui-tokens --check` | PASS | Tương phản 36 cặp đạt cả hai theme (thêm 7 cặp avatar, 1 viền field); icon 13/13 khớp SHA-256 upstream |
| Generator, icon/identity | `cargo test -p xtask` (trong `verify quick`) | PASS | Thêm: path sửa 1 ký tự bị từ chối (SHA lệch), thiếu license bị từ chối, tone ngoài khoảng bị từ chối, NFD/ZWJ giữ escape |
| Lane quick | `cargo xtask verify quick` | PASS | fmt, clippy server/hydrate/preview `csr`, test (`vink-ui` 25, `vink-ui-preview` 7), ranh giới UI system, link, skill |
| Lane web (app dùng `initials` ủy quyền) | `cargo xtask verify web` | PASS | SSR, hydration, editor, shell, theme, restart; không assertion web mới vì chưa màn nào dùng component |
| Lane postgres | — | NOT_RUN | Không đổi code PostgreSQL/App persistence |
| Preview Leptos | `cargo xtask ui-preview leptos verify` | PASS | 66 link kịch bản, assertion component (dưới), 253 ảnh + `contexts.json` ở `target/ui-preview/leptos/captures/` |
| Preview CMP desktop + native | `VINK_UI_NATIVE_TARGETS=1 cargo xtask ui-preview cmp verify` | PASS | `VinkThemeTest` 10, `VinkScopeTest` 3, `VinkIdentityTest` 6, `PreviewTest` 10, `CaptureTest` 1 (145 ảnh ở `packages/ui-system/cmp/preview/build/captures/`); `:ui:iosSimulatorArm64Test` (`VinkThemeTest` 10, `VinkIdentityTest` 6) và `compileDebugKotlinAndroid` |
| CMP compile Android + iOS | `VINK_UI_NATIVE_TARGETS=1 cargo xtask ui-preview cmp build` | PASS | `actual fun nfc` cho JVM (desktop/Android) và Foundation (iOS) |
| Golden vectors chung | Rust `identity::tests`, Kotlin `VinkIdentityTest` (JVM + iOS simulator), smoke Chrome | PASS | 21 initials + 10 tone giống nhau ở Rust, JVM, Kotlin/Native; tông render trong Chrome bằng vector |
| IME hệ điều hành thật, TalkBack/VoiceOver, Android/iOS UI runtime | — | NOT_RUN | Chưa có diagnostic host (#24); composition chỉ mô phỏng bằng CDP `Input.imeSetComposition` |
| Hover CMP | — | N/A | Ảnh CMP là render off-screen không con trỏ; trên thiết bị cảm ứng không có hover. Material indication có sẵn khi có con trỏ, chưa quan sát |
| Cửa sổ CMP Desktop thật | `cargo xtask ui-preview cmp run` | NOT_RUN | Ảnh là `ImageComposeScene` off-screen bằng renderer thật |
| CI | — | NOT_RUN | `ui-preview` vẫn là lệnh cục bộ; CI chạy `quick` (gồm generator, test `vink-ui`/preview) |

## Kịch bản đã khẳng định

Leptos (`preview-smoke.mjs`, hàm `structure` và `components`): mọi kịch bản không có ID trùng và
mọi `aria-describedby` trỏ tới phần tử có thật; Button: click, Enter, Space mỗi lần gọi đúng 1;
focus bàn phím có vòng 3px; Button mặc định trong form không gửi form; target ≥48px; disabled không
gọi, vẫn focus được và lý do được liên kết; pending không nhận kích hoạt thứ hai, giữ bề rộng
(143.97 → 143.97px) và focus; Link tab mới có `noopener noreferrer` + "(mở tab mới)", URL
`javascript:` không thành link; FilterChip Space đổi `aria-pressed`, Tab sang chip kế; TextField giữ
caret giữa chữ qua controlled value ("Bànx viết", caret 4), composition được gắn cờ đang soạn rồi chữ
đã chốt đến không cờ; nhãn/gợi ý/bắt buộc liên kết, lỗi rỗng không invalid, lỗi có thật thì
`aria-invalid` + `title-help title-error`; read-only focus được, disabled không; giá trị `0` giữ
nguyên; SearchField Enter gửi, nút xóa làm rỗng và trả focus; Checkbox nhãn + ô một target một
lần gọi; RadioGroup phím mũi tên; mọi input/textarea/select có `<label for>`; field/choice ≥48px;
avatar AVIF hỏng/URL script/Failed/Loading hiện initials, không `<img src="javascript:…">`; đảo thứ
tự giữ tông theo key và khớp golden vectors; AvatarGroup chưa biết tổng không có "+N"/"người khác",
không điểm dừng focus, biết tổng hiện "+9"; byline tên 600/16px trên meta 13px, avatar ẩn, tên đọc
một lần, avatar reader 50px; tương phản chữ đã resolve ≥4.5 (thấp nhất 5.86 sáng, 6.72 tối) trên
button, chip, field + lỗi, tóm tắt lỗi, avatar, byline, composition; không tràn ngang 320–1440px ở
chữ 100/200% mọi kịch bản; không lỗi console.

CMP (`PreviewTest`): mọi kịch bản của 29 entry render, không còn "Chưa đăng ký"; nút gọi 1 lần,
disabled không gọi (`assertIsNotEnabled`), pending giữ node enabled + `stateDescription` và bỏ qua
lần 2; text field giữ selection qua `TextFieldValue` ("Bànx viết" · 4..4), nhãn nằm trong semantics
field, lỗi qua `error()`, giá trị `0`; checkbox và chip một lần; avatar có nhãn, byline không đọc
tên hai lần, nhóm không bịa số; PNG giải mã được, AVIF hỏng → `Failed`.

## Ảnh đã mở và nhận xét

| Ảnh | Nhận xét → sửa |
| --- | --- |
| Leptos `action.button`, `action.icon-button`, `action.filter-chip` (1440 sáng/tối) | Năm biến thể đọc rõ cả hai theme; chip dành chỗ dấu ✓ nên không nhảy bề rộng khi chọn. Không sửa |
| Leptos `field.text-field`, `field.select`, `field.radio` (1440 sáng/tối), `field.form--invalid--light--390` | Nhãn trên control, "(bắt buộc)" bằng chữ, lỗi có icon + chữ, tóm tắt lỗi có link; lựa chọn "Kịch (chưa hỗ trợ)" mờ theo disabled |
| Leptos `identity.avatar` (1440 sáng/tối), `identity.avatar--image-error--light--390`, `identity.author-byline`, `identity.avatar-group` | Sáu tông + trung tính đọc rõ; ảnh PNG đè initials, các ca lỗi về initials; tên đậm trước meta; "+9" |
| Leptos `composition.nested--narrow--light--390`, `field.form--default--light--320--text200` | Sáng trong Tối đúng; 200% xuống dòng, không cắt |
| CMP `action.button` (rộng sáng/tối), `field.text-field`, `field.form--invalid`, `identity.avatar--image-error`, `composition.nested--narrow`, `identity.author-byline` (hẹp tối) | Đối ứng Leptos; tên byline có link dùng màu primary → đổi sang màu chữ chính + gạch chân như Web |
| CMP `field.form--invalid--light--narrow`, `field.text-field--default--dark--wide` (sau khi đổi nhãn) | Nhãn Material nằm trong viền khi trống, nổi lên khi có giá trị (Select); "(bắt buộc)" trong nhãn; lỗi icon + chữ dưới field |

## Tự review và phản biện

Không có reviewer độc lập trong phiên. Tự review final diff tìm thấy và đã sửa:

- **CMP nhãn field giả bằng `contentDescription`:** TalkBack có thể đọc mô tả thay cho giá trị đang
  nhập — tức giả ARIA trong native. Đổi sang slot `label` của Material (nhãn gộp vào semantics field,
  luôn hiển thị); test tìm field theo chữ nhãn + `SetText`.
- **FilterChip bỏ mất lý do `Disabled`** ở cả hai renderer → ghi chú dưới chip như Button.
- **Smoke tìm thấy:** tiêu đề specimen một từ dài tràn 8px ở 320px/200% (CSS preview); Chrome bỏ
  điều hướng hash khi quá nhiều lần liên tiếp ("Throttling navigation") → mỗi kịch bản chỉ mở một
  lần rồi đổi viewport, có thử lại. `assert()` của Kotlin là API thử nghiệm trên Native → `assertTrue`.
- **Tương phản:** lựa chọn disabled 3.16:1 là màu disabled 38% của M3, được WCAG 1.4.3 miễn cho UI
  không hoạt động và token đã ghi lý do; phép đo loại control disabled, lý do vẫn là chữ.

Giới hạn còn lại: catalog vẫn `implemented-unverified` cho mọi entry ở cả hai renderer; tương phản
là màu resolve trên Chrome/Skia, không phải quét thiết bị; Compose không có `aria-describedby` nên
help là chữ kế bên; Select CMP dựa vào bàn phím của menu Material (chưa kiểm trên Android/iOS);
quy tắc initials hứa giống nhau cho chữ Latin/Việt/số ASCII/emoji/CJK BMP, không cho chữ ngoài BMP,
Nl hay Other_Alphabetic. Adoption trên màn app thuộc #23 sau qualification #24.
