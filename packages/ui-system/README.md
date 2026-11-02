# Vink UI System

**Trạng thái:** UIS-01/06 ([#19](https://github.com/loveoverflowcom/vink/issues/19)) — nền tảng token,
theme và hai preview; UIS-02/06 ([#20](https://github.com/loveoverflowcom/vink/issues/20)) — component
nền theo họ (chữ/bố cục, hành động, nhập/chọn, danh tính/hình ảnh) ở cả hai renderer, contract ở
[shared/components.md](shared/components.md); UIS-03/06 ([#21](https://github.com/loveoverflowcom/vink/issues/21)) —
trạng thái, phản hồi, loading và motion ở cả hai renderer, contract ở [shared/states.md](shared/states.md).
Chuỗi tiếp theo: #22 shell/navigation → #23 patterns/adoption → #24 qualification. Chưa màn nào của
app dùng component UIS-02/03. Đây là thư viện renderer, không phải ứng dụng thứ hai
và không giữ session, API, route hay nghiệp vụ. Tổ chức học từ Cantos UI system (`7649017`) nhưng
không chép thương hiệu, phiên bản, palette hay build; Vink giữ Lavender, Be Vietnam Pro/Literata,
Enso và Leptos 0.8.

## Owner map

| Owner | Nội dung | Trạng thái |
| --- | --- | --- |
| [`shared`](shared/README.md) | `tokens.json` (nguồn token duy nhất), `tokens.schema.json`, `preview-catalog.json`, `fixtures/` (gồm golden vectors identity và states), `icons.json` + license, [contract component](shared/components.md), [contract trạng thái](shared/states.md) | Đã triển khai |
| [`leptos`](leptos/README.md) | crate `vink-ui`: token/catalog/icon/vector Rust sinh ra, `tokens.css` sinh ra, `UiRoot`/`ThemeScope`, component theo họ (`text`, `layout`, `action`, `field`, `media`, `identity`, `feedback`), contract thuần `state`, CSS renderer có scope; crate `vink-ui-preview` | Đã triển khai; preview kiểm bằng Chrome headless |
| [`cmp`](cmp/README.md) | Gradle: `:ui` (commonMain, `VinkTheme`/`VinkScope`, component `Vink*` theo họ, `VinkState`/`VinkFeedback`, token/catalog/icon/vector Kotlin sinh ra; NFC theo nền tảng), `:preview` (Compose Desktop) | Đã triển khai; JVM desktop có test và ảnh; Android/iOS chỉ compile khi bật |
| `xtask/src/ui_tokens.rs`, `xtask/src/ui_states.rs` | Validate + generate + `--check` (token, catalog, fixture, icon registry, identity vectors, state vectors) | Đã triển khai |
| `xtask/src/ui_preview.rs` | `cargo xtask ui-preview leptos\|cmp build\|verify\|run` | Đã triển khai |
| App (`apps/vink-web`) | Key `vink.ui.theme`, bootstrap trước CSS, khi nào ghi storage, route/copy/dữ liệu | Giữ nguyên owner |

Một role chỉ có một owner. `apps/vink-web/style/vink.css` không còn khai báo giá trị token: build
phục vụ `tokens.css` (sinh) rồi `vink.css` (component web) thành một `/pkg/vink.css`.

```text
shared/tokens.json + preview-catalog.json + fixtures/ + icons.json   (không runtime, không backend)
  ├─ cargo xtask ui-tokens → leptos/style/tokens.css, leptos/src/{tokens,catalog,icons,identity_vectors}.rs
  │                            → vink-ui (UiRoot, ThemeScope, component) → vink-web pilot, preview Leptos
  └─ cargo xtask ui-tokens → cmp/ui/src/commonMain/.../{VinkTokens,VinkCatalog,VinkIcons,VinkIdentityVectors}.kt
                               → :ui (VinkTheme, VinkScope, Vink*) → :preview (Compose Desktop)
preview = public API của renderer; không clone component, không DOM chung với Compose, không WebView
```

## Lệnh

Nguồn lệnh duy nhất vẫn là [docs/development](../../docs/development/README.md); tóm tắt:

| Lệnh | Việc |
| --- | --- |
| `cargo xtask ui-tokens` | Validate các nguồn shared rồi ghi 11 file sinh ra (chỉ ghi file thay đổi) |
| `cargo xtask ui-tokens --check` | Không ghi; lỗi khi thiếu hoặc lệch. Chạy trong `verify quick` và trước mọi build web/preview |
| `cargo xtask ui-preview leptos build\|run\|verify` | Bundle wasm (`csr`) vào `target/ui-preview/leptos`; `run` phục vụ ở `VINK_UI_PREVIEW_BIND` (mặc định `127.0.0.1:3019`); `verify` chạy Chrome headless và ghi ảnh vào `target/ui-preview/leptos/captures/` |
| `cargo xtask ui-preview cmp build\|run\|verify` | Gradle wrapper 8.14.3; `run` mở cửa sổ desktop; `verify` chạy test common + desktop và ghi ảnh vào `cmp/preview/build/captures/` |
| `VINK_UI_NATIVE_TARGETS=1 cargo xtask ui-preview cmp build\|verify` | Thêm Android (`ANDROID_HOME`) và iOS arm64/simulator arm64 (Xcode, tải Kotlin/Native lần đầu) |

Link kịch bản chính xác của preview Leptos: `http://127.0.0.1:3019/#<catalog-id>/<scenario>`, ví dụ
`#foundation.typography/combining-marks` hoặc `#action.button/pending`. Preview CMP chọn cùng ID
trong danh mục. Thao tác trên specimen ghi vào nhật ký sự kiện của inspector, không gọi backend.

## LayoutSpec preview (r1, đã trình bày trước khi viết UI)

```text
Screen #preview — kiểm specimen foundation qua renderer thật
  Column #page
    Region #preview-header — tên, phiên bản token/catalog, "Không kết nối dịch vụ", đồng hồ cố định
    Grid #workbench — rộng: nav | stage; hẹp hoặc chữ lớn: stage xuống dưới nav
      Region #catalog-nav — ô tìm (không dấu, cả nhãn kịch bản) → danh sách catalog ID
      Column #stage
        Region #preview-controls — Giao diện Sáng/Tối/Hệ thống · Khung · Cỡ chữ 100/200% · Chuyển động
        Region #specimen — UiRoot/ThemeScope (Leptos) | VinkTheme/VinkScope (CMP) với kịch bản
        Region #inspector — catalog ID, nguồn @ phiên bản, trạng thái, giá trị hiệu lực, nhật ký
```

Leptos đọc giá trị hiệu lực bằng `getComputedStyle` của scope specimen; CMP đọc `LocalVink` của
scope có cùng tham số. Preview không đọc storage, không gọi service; preference chỉ ở bộ nhớ.

## API và precedence

| Khía cạnh | Leptos (`vink-ui`) | CMP (`:ui`) |
| --- | --- | --- |
| Gốc | `UiRoot(preferences: Signal<Preferences>, overrides?)` → `.vk-ui.vk-scope[data-*]` | `VinkTheme(preferences, systemDark, systemReducedMotion, fonts, overrides)` |
| Lồng | `ThemeScope(theme?, density?, reduce_motion, overrides)` | `VinkScope(theme?, density?, reduceMotion, overrides)` |
| Theme | Scope gần nhất có theme tường minh > scope ngoài > `<html data-theme>` (app) > OS | Scope gần nhất > `VinkTheme` > `systemDark` (mặc định `isSystemInDarkTheme()`) |
| Mật độ | Giá trị khai báo gần nhất | Như Leptos |
| Giảm chuyển động | Cộng dồn: OS, gốc hoặc scope ngoài giảm thì bên trong không bật lại | Như Leptos; `systemReducedMotion` do host inject (CMP 1.9 không có API common) |
| Override | `RoleOverride { role, to }`: role → role, không hex; tự tham chiếu/trùng/vòng bị từ chối, render `data-override-error` | `RoleOverride(role, to)`, `validate()`; scope có theme tường minh bỏ override kế thừa (giống CSS khai báo lại role) |
| Đơn vị | rem (spacing, chữ), px (radius, hit target), chữ đọc fluid `clamp()` | dp, sp; line-height = size × ratio theo sp (font scale áp một lần); chữ fluid dùng giá trị min |

Thư viện không đọc storage/tài khoản. App giữ persistence, key storage và bootstrap trước CSS:
`apps/vink-web/src/appearance.rs` dùng `vink_ui::ThemePreference` cho giá trị/parse, còn lại giữ
nguyên. CSS component có scope `.vk-ui`/`vkp-`; không reset tag toàn cục, không chạm CodeMirror.

## Component UIS-02

API, anatomy (LayoutSpec), sự kiện, owner và trạng thái không hỗ trợ: [shared/components.md](shared/components.md).
Tóm tắt khác biệt có chủ đích giữa hai renderer:

| Khía cạnh | Leptos | CMP |
| --- | --- | --- |
| Giá trị chữ | `Signal<String>` vào, `TextInput { value, composing }` ra; DOM chỉ ghi khi giá trị đổi | `TextFieldValue` vào/ra (giữ selection + composition), không qua `String` |
| Nhãn field | `<label for>` phía trên control, help/error qua `aria-describedby` | Slot `label` của Material (gộp vào semantics của field), lỗi qua `error()`; help là chữ kế bên |
| Disabled / Pending | `aria-disabled` (+`aria-busy`), vẫn focus được, click bị chặn | `enabled = false` / node giữ focus + `stateDescription`, click bị bỏ qua |
| Submit | `ButtonKind::Submit` tường minh; mặc định `type="button"` | Không có form native: N/A |
| Link | `<a href>` thật, `NewTab` báo trước, URL lạ hiện như chữ | `LinkAnnotation` → `onNavigate(href)` của host; tab/modifier-click N/A |
| Heading | `h1`–`h6` độc lập kiểu chữ | `heading()` không cấp: cấp chỉ chọn kiểu mặc định |
| Select | `<select>` native | `ExposedDropdownMenuBox` của Material |
| Ảnh | `ImageState::Ready { src }` (allow-list URL), lỗi tải/giải mã → initials | `ImageState.Ready(painter)` do host giải mã; lỗi giải mã là `Failed` |
| NFC (initials/màu) | `unicode-normalization` | `expect fun nfc`: JVM `java.text.Normalizer`, iOS Foundation |
| Hover | CSS state layer 8%/12% | Material ripple/indication; capture off-screen không có hover (N/A) |
| Kích thước | 48px, mật độ gọn 44px (sàn đã công bố) | ≥48dp |

## Kế hoạch migration

1. **UIS-01 (PR này):** nguồn token duy nhất; Web giữ nguyên toàn bộ tên `--vk-*` (đã so 59 + 37 + 36
   giá trị cũ với CSS sinh ra, không lệch); `vink.css` bỏ khối token; reduced motion giới hạn
   animation một lần; `appearance.rs` dùng type theme của thư viện. Không đổi route, DOM, editor
   hay session.
2. **UIS-02 (#20):** 25 component đăng ký trong catalog và render ở **cả hai** preview; token
   thêm avatar tone/kích thước, `density.control-height`, `measure-wide`, `size.grid-min-*` (0.2.0,
   minor). `appearance.rs::initials` ủy quyền cho `vink_ui::identity::initials` (giữ `•`), không đổi
   DOM/route/storage.
3. **UIS-03 (#21):** 8 entry `state.*` (37 kịch bản) ở cả hai preview, token 0.3.0 thêm thời lượng
   loading/snackbar và kích thước chỉ báo (minor); Button pending lấp slot `status` bằng
   `LoadingIndicator`. Không màn app nào đổi. **#22:** shell/navigation và coordinator overlay.
   Component CSS dần dùng role chữ `--vk-type-*` thay literal trong `vink.css`.
4. **#23–#24:** màn Web/CMP dùng pattern; qualification, Android/iOS diagnostic host và versioning.
   Đổi `version` trong `tokens.json` theo semver khi đổi/xóa role (major), thêm role (minor), sửa
   giá trị không đổi nghĩa (patch); generator in phiên bản vào mọi file sinh ra.

Thêm hoặc đổi token: sửa `shared/tokens.json` → `cargo xtask ui-tokens` → xem diff 5 file sinh ra
→ `cargo xtask verify quick` → hai preview. Không sửa tay file có dòng `GENERATED`.

## Bằng chứng và giới hạn

Kết quả chạy, môi trường, SHA và PASS/FAIL/NOT_RUN/UNSUPPORTED nằm ở
[ui-system-foundation-evidence](../../docs/development/ui-system-foundation-evidence.md) (UIS-01) và
[ui-system-primitives-evidence](../../docs/development/ui-system-primitives-evidence.md) (UIS-02),
[ui-system-states-evidence](../../docs/development/ui-system-states-evidence.md) (UIS-03). Trạng thái
catalog của mọi entry vẫn là `implemented-unverified`: compile, test và ảnh của JVM/Chrome không
thành nghiệm thu thiết kế, Android/iOS, IME hay screen reader.
