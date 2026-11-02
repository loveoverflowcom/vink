# Bằng chứng UI System 01/06 — nền tảng token, theme và hai preview

**Ngày:** 02/11/2026 (UTC+07) · **Issue:** [#19](https://github.com/loveoverflowcom/vink/issues/19) ·
**Nhánh:** `codex/ui-system-foundation`, nền `develop@b358794` (snapshot của issue là `5afb00b`; code
nền tảng #4–#10, #14 và PostgreSQL/Identity #26–#27 đã có trên nền này, không reset). Đây là kết quả
chạy trên máy dev; không phải nghiệm thu thiết kế, thiết bị thật hay production. Owner map, API và
migration: [packages/ui-system](../../packages/ui-system/README.md).

## Môi trường

| Mục | Giá trị |
| --- | --- |
| Máy | macOS 26.5.2 (25F84), Apple Silicon arm64 |
| Rust / wasm-bindgen | 1.99.1 / 0.2.129 |
| Node / Chrome | v26.12.0 / Chrome 156.0.8078.79 headless (`--headless=new`), DPR 1 |
| JDK / Gradle / Kotlin / Compose | Microsoft OpenJDK 17.0.19 / 8.14.3 (wrapper) / 2.2.20 / Compose Multiplatform 1.9.0, Material3 1.8.2 |
| Android / iOS toolchain | Android SDK platform 36 (AGP 8.11.1) / Xcode 26.6, Kotlin/Native 2.2.20 (tải lần đầu trong phiên) |
| PostgreSQL | 17.11 cluster scratch trên `127.0.0.1:55432` (chỉ cho `verify web`/`postgres`) |
| Token | `tokens.json` 0.1.0, catalog 0.1.0; SHA-256 `tokens.json` bắt đầu `a5684b15dab9f633` |

## HOW / WHY đã ghi trước khi đổi

- **HOW (luồng đổi theme + storage lỗi):** `BOOTSTRAP` trong `appearance.rs` đọc `vink.ui.theme` trước
  CSS và đặt `<html data-theme>`; CSS có `:root` (sáng), `:root[data-theme=dark]` và `@media dark`
  lặp lại cùng giá trị. `ThemePicker` → `Appearance::set_theme` đổi attribute rồi ghi
  `localStorage`; ghi lỗi thì lựa chọn vẫn hiệu lực trong phiên, không giữ sau reload; Hệ thống xóa
  attribute để CSS theo OS. UIS-01 giữ nguyên luồng này, chỉ đổi nơi sinh CSS và type theme.
- **WHY có nguồn:** #5/#7 chọn Lavender, Be Vietnam Pro/Literata và `vink.css` làm owner; #10 Enso.
  **Đề xuất mới của #19:** thêm CMP cần contract chung để không có hai bản màu/chữ sửa tay và để thư
  viện UI không giữ session/API. Quyết định scoped trong PR: generator viết bằng Rust trong `xtask`
  (không thêm Python), giữ mọi tên `--vk-*`, target native opt-in. Không chốt native release,
  identity/Discourse hay D/INV/S nào.

## Kết quả

| Kiểm | Lệnh | Kết quả | Ghi chú |
| --- | --- | --- | --- |
| Nguồn token hợp lệ, file sinh ra khớp | `cargo xtask ui-tokens --check` | PASS | 98 token, 68 primitive, 28 cặp tương phản đạt cả hai theme |
| Generator: schema/alias/vòng/thiếu theme/đơn vị sai/field lạ/tương phản thấp/sinh hai lần | `cargo test -p xtask` (trong `verify quick`) | PASS | `ui_tokens::tests` |
| Giá trị Web không đổi | so sánh CSS cũ (`HEAD:vink.css`) với `tokens.css` | PASS | 59 `:root` + 37 dark + 36 system-dark, 0 lệch (font stack chỉ khác dấu ngoặc kép) |
| Lane quick | `cargo xtask verify quick` | PASS | fmt, clippy server, clippy hydrate, clippy preview `csr`, test, ranh giới UI system |
| Lane web (SSR, hydration, editor, theme, contrast, reflow) | `cargo xtask verify web` | PASS | Chạy trên `/pkg/vink.css` mới; contrast shell/desk sáng ≥5.79, tối ≥7.87; ảnh mở: Khám phá 1440 sáng, editor + overlay Tôi 1280 tối |
| Lane postgres | `cargo xtask verify postgres` | PASS | Không đổi code PostgreSQL; chạy để bắt hồi quy |
| Preview Leptos | `cargo xtask ui-preview leptos verify` | PASS | 38 ảnh + `contexts.json` ở `target/ui-preview/leptos/captures/` |
| Preview CMP desktop | `cargo xtask ui-preview cmp verify` | PASS | `VinkThemeTest` 10, `VinkScopeTest` 3, `PreviewTest` 6, `CaptureTest` 1 (30 ảnh ở `packages/ui-system/cmp/preview/build/captures/`) |
| CMP compile Android + iOS | `VINK_UI_NATIVE_TARGETS=1 cargo xtask ui-preview cmp build` | PASS | `:ui:compileDebugKotlinAndroid`, `compileKotlinIosArm64`, `compileKotlinIosSimulatorArm64` |
| CMP common test trên iOS simulator | `VINK_UI_NATIVE_TARGETS=1 cargo xtask ui-preview cmp verify` | PASS | `:ui:iosSimulatorArm64Test`: `VinkThemeTest` 10/10 trên simulator arm64 (logic common; không phải UI runtime) |
| Cửa sổ CMP Desktop thật | `cargo xtask ui-preview cmp run` | NOT_RUN | Ảnh CMP là render off-screen bằng renderer thật (`ImageComposeScene`), không phải chụp cửa sổ |
| Android/iOS runtime, TalkBack/VoiceOver, IME | — | NOT_RUN | Chưa có diagnostic host (#24) |
| Font thương hiệu trên Android/iOS | — | UNSUPPORTED | Chưa có pipeline TTF/OTF có provenance; dùng font hệ thống |
| Tự phát hiện reduced motion trên CMP | — | UNSUPPORTED | CMP 1.9 không có API common; host inject `systemReducedMotion` |

## Kịch bản preview Leptos đã khẳng định

`preview-smoke.mjs`: danh mục đúng 4 ID theo thứ tự catalog; mỗi `#<entry>/<scenario>` (8 link) render
đúng scope và inspector; Sáng/Tối đè hệ thống cả hai chiều, Hệ thống theo đổi OS không reload, inspector
đọc lại giá trị computed sau khi OS đổi (lỗi tìm thấy và sửa trong phiên: inspector giữ giá trị cũ khi
chỉ OS đổi); scope lồng nhau Sáng/Tối/Hệ thống-trong-Tối đúng, recipe `--vk-action-primary-container`
giải theo scope lồng; mật độ gọn 0.5rem nhưng control vẫn cao 44px; vòng lặp chạy mặc định, dừng
(`animation-iteration-count: 0`, không animation đang chạy) khi scope giảm, OS giảm hoặc preference Giảm,
chuyển tiếp 320ms → 0ms; tìm "chuyen dong", "DẤU TỔ HỢP", không khớp; không tràn ngang ở 320/390/834/1440px
với chữ gốc 100% và 200% ở mọi kịch bản (lỗi tìm thấy và sửa: fieldset/input min-content, grid ngầm
`auto`, nav sticky đè nội dung khi xếp chồng); Be Vietnam Pro 400/600 và Literata tải được; không lỗi console.

## Ảnh đã mở và nhận xét

| Ảnh | Nhận xét → sửa |
| --- | --- |
| Leptos `foundation.color-roles--default--light--1440` | Swatch, bảng tương phản, inspector đọc được; thứ tự vai trò chữ theo bảng chữ cái → đổi sang phân cấp display→code |
| Leptos `foundation.typography--default--dark--390` | Chữ tiếng Việt đủ dấu, khổ thơ giữ dòng trống; pill "Không kết nối dịch vụ" phình khi xuống dòng → bo `radius.md` |
| Leptos `…combining-marks--light--320--text200` | Dấu tổ hợp NFD hiển thị đúng, không cắt; padding ăn chỗ ở 320px/200% → padding `min(token, vw)` |
| Leptos `foundation.color-roles--nested-theme--light--1440`, `motion--reduced--light--834`, `shape-spacing--default--dark--1440` | Lồng theme đúng; vòng lặp tĩnh khi giảm; không có vấn đề mới |
| CMP `color-roles--default--light--wide`, `typography--default--dark--narrow` | Font tự host đúng; vùng dưới nội dung không được tô (đen) → workbench `fillMaxSize`; radio sát nhãn → khoảng 4dp; swatch lệch rộng → cột đều theo bề rộng |
| CMP `color-roles--nested-theme--light--1280` | Hai pane xếp dọc ở 1280dp → chia đôi khi ≥640dp |
| CMP `motion--reduced--light--834` | Chỉ báo vòng lặp như khối màu → vẽ cung tròn bằng `Canvas` |
| CMP `typography--combining-marks--light--narrow--text200`, `shape-spacing--compact--light--834` | Dấu tổ hợp và chữ 200% không cắt; padding gọn 8dp, cao ≥44dp |
| Web `explore-1440-100-light`, `1280-editor-dirty-overlay-dark` (từ `verify web`) | Giống baseline: palette, font, overlay Tôi, editor không đổi |

## Tự review và giới hạn

Không có reviewer độc lập trong phiên; đây là tự review từ final diff và caller/callee. Phản ví dụ đã
thử: alias vòng/treo/sai type, thiếu theme, đơn vị sai, file sinh ra lệch hoặc thiếu (build web từ chối
phục vụ CSS cũ), override tự tham chiếu/trùng/vòng, vòng đi qua recipe alias (`primary → action.primary.container`,
tìm thấy trong tự review và sửa ở cả hai renderer), override lồng giữa scope (CMP từng gộp map, lệch CSS —
sửa để giải theo giá trị kế thừa như custom property), đường dẫn `..` ở static server preview.

- Tương phản trong generator/preview là số học token WCAG 2.x, không phải chứng nhận màn hình.
  Disabled/`outline-variant` được loại khỏi ngưỡng có chủ đích.
- Catalog vẫn `implemented-unverified` cho cả bốn entry ở cả hai renderer: không tick UX-01/WR-02.
- `ui-preview` chưa chạy trong CI; `verify quick` trong CI vẫn kiểm generator, drift, clippy preview và test.
- Ảnh preview là bằng chứng cho người mở, không phải visual baseline tự bless.
