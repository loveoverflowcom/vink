# Tailwind CSS v4 + Leptos 0.8 trên Vink

Đây là **nguồn duy nhất** cho quy ước Tailwind CSS v4 trên Web và Leptos preview của Vink. Các
skill khác dẫn về tài liệu này thay vì lặp lại luật compiler, `@layer` hay ví dụ Leptos.

- **Phiên bản pin hiện hành:** `leptos = 0.8.22` (`Cargo.lock`), `tailwindcss = 4.3.5` và
  `@tailwindcss/cli = 4.3.5` (`packages/ui-system/leptos/tailwind/package.json` +
  `package-lock.json`, cài bằng `npm ci --ignore-scripts --no-audit --no-fund`).
- **Ngưỡng tối thiểu Tailwind cho Leptos:** `>= 4.1.8` (bản sửa trích xuất cú pháp `class:` của
  Leptos, `tailwindlabs/tailwindcss#18093`, khóa cứng tại `xtask/src/tailwind.rs::MIN_VERSION`).
- **Ranh giới nền tảng:** Chỉ áp dụng cho Web (`apps/vink-web`, `packages/ui-system/leptos`,
  `packages/vink-patterns/leptos`, `packages/ui-system/leptos/preview`). Không đổi Compose
  Multiplatform (`packages/ui-system/cmp`), không chuyển sang `cargo-leptos`, không thêm CSS
  runtime hay JS framework.

## 1. Pipeline biên dịch và nguồn sự thật (SSOT)

```text
packages/ui-system/shared/tokens.json (SSOT duy nhất, 0.8.0)
  └─ cargo xtask ui-tokens (kiểm drift bằng --check trong verify quick & mọi build)
       ├─ packages/ui-system/leptos/style/tokens.css          (giá trị --vk-*, @font-face, theme/density/motion)
       ├─ packages/ui-system/leptos/style/tailwind-theme.css  (@theme inline ánh xạ --*-vk-* → var(--vk-*))
       └─ Rust / Kotlin accessors (tokens.rs, VinkTokens.kt, ...)

packages/ui-system/leptos/tailwind/web.css     ──┐
                                                  ├─ xtask/src/tailwind.rs (tailwindcss 4.3.5)
packages/ui-system/leptos/tailwind/preview.css ──┘
  ├─ Web:     target/site/pkg/vink.css                (--minify khi --release)
  └─ Preview: target/ui-preview/leptos/vink-preview.css
```

- **Một owner cho compiler:** `xtask/src/tailwind.rs` chịu trách nhiệm cài đặt có khóa
  (`npm ci --ignore-scripts`), biên dịch nguyên tử qua file tạm và xóa file đích cũ trước khi
  biên dịch để lỗi compiler không bao giờ để lại CSS stale.
- **Một nguồn token duy nhất:** Không bao giờ sửa tay `tokens.css` hay `tailwind-theme.css`. Khi
  cần vai trò màu/chữ/bán kính/khoảng cách mới, cập nhật `packages/ui-system/shared/tokens.json`
  và chạy `cargo xtask ui-tokens`.
- **Không dùng `npx` hay tải động:** `xtask/src/checks.rs` gọi `tailwind::check` trong
  `verify quick` để khóa version pin chính xác giữa `package.json`, `package-lock.json`,
  `tailwindcss` và `@tailwindcss/cli`.

## 2. Hợp đồng `@layer`, không Preflight và song hành legacy CSS

Cả `web.css` và `preview.css` dùng chung thứ tự layer:

```css
@layer theme, base, legacy, components, utilities;

@import "tailwindcss/theme.css" layer(theme);
@import "../style/tailwind-theme.css";
@import "../style/tokens.css" layer(base);
@import "../style/vink-ui.css" layer(legacy);
@import "../../../vink-patterns/leptos/patterns.css" layer(legacy);
@import "../../../../apps/vink-web/style/vink.css" layer(legacy); /* hoặc preview.css */
@import "tailwindcss/utilities.css" layer(utilities) source(none);
```

- **Không bật Tailwind Preflight:** giữ nguyên reset hiện hành của Vink, prose Markdown,
  CodeMirror 6 và native `<select>`/`<input>`/`<dialog>`.
- **Ưu tiên cascade khi song hành:** `@layer utilities` đứng sau `@layer legacy` nên utility thường
  thắng rule thường ở `@layer legacy` không cần `!important`. Ngược lại, trong CSS Cascade Layers,
  `!important` **đảo ngược thứ tự layer** (`@layer legacy !important` thắng `@layer utilities !important`),
  nên các rule reduced-motion fallback có `!important` trong `vink.css` / `preview.css` vẫn giữ độ
  ưu tiên cao nhất; nhưng các rule `@media (forced-colors: active)` **không** có `!important` trong
  `@layer legacy` sẽ thua `@layer utilities` (component chuyển sang utility phải tự mang
  `forced-colors:*` khi cần).
- **DOM bên thứ ba bơm `<style>` không layer ở runtime (CodeMirror 6 `style-mod`):** Theo chuẩn CSS
  Cascade Layers, mọi rule không nằm trong `@layer` đều thắng mọi `@layer`. Do CodeMirror 6 bơm
  `<style>` không layer vào `document.head` khi mount (ví dụ `.cm-editor.cm-focused` có viền chấm),
  các rule host `.vk-editor-body .cm-editor...` phải đặt ở phần **không layer** cuối
  `packages/ui-system/leptos/tailwind/web.css`.
- **Bẫy cascade bắt buộc phải kiểm khi migrate con trước cha:** Một selector hậu duệ ở
  `@layer legacy` (ví dụ `.vk-editor-recovery > .vk-ui-notice { padding: 0; background: transparent; }`
  hoặc `.vk-author-work__status { display: block; }`) sẽ **THUA** utility của component con trong
  `@layer utilities` bất kể specificity! Trước khi xóa rule legacy của một component, hãy grep
  toàn bộ các file CSS nguồn (`packages/ui-system/leptos/style/vink-ui.css`,
  `packages/vink-patterns/leptos/patterns.css`, `apps/vink-web/style/vink.css`,
  `packages/ui-system/leptos/preview/style/preview.css`) tìm mọi rule cha/modifier đè lên class đó
  và chuyển override sang utility của cha (ví dụ `[&>.vk-ui-notice]:p-0 [&>.vk-ui-notice]:bg-transparent [&>.vk-ui-notice]:text-inherit`)
  hoặc xóa rule xung đột cùng lát.
- **Thoát dấu gạch dưới kép (`__`) của BEM trong arbitrary variant `[...]`:** Tailwind v4 biến mọi
  dấu `_` không thoát trong `[...]` thành khoảng trắng! Khi viết selector chứa class BEM `foo__bar`
  bên trong `[...]`, bắt buộc thoát thành `\_\_` (trong chuỗi Rust thường là `\\_\\_`, ví dụ
  `"[&_.vk-ui-notice\\_\\_icon]:hidden"` để sinh đúng `.vk-ui-notice__icon`).

## 3. Quét nguồn có kiểm soát (`source(none)` + allowlist `@source`)

- Cấm `@import "tailwindcss";` quét toàn repo. Entry bắt buộc dùng
  `@import "tailwindcss/utilities.css" layer(utilities) source(none);` và chỉ khai báo `@source`
  tới các thư mục nguồn Rust UI hợp lệ:
  - Web (`web.css`): `apps/vink-web/src`, `packages/ui-system/leptos/src`,
    `packages/vink-patterns/leptos/src`.
  - Preview (`preview.css`): `packages/ui-system/leptos/src`,
    `packages/ui-system/leptos/preview/src`, `packages/ui-system/leptos/preview/examples`,
    `packages/vink-patterns/leptos/src`.
- `tailwind::check` trong `verify quick` từ chối mọi `@source` trỏ ra ngoài allowlist trên (docs,
  skills, fixtures, SQL, `target/` không bao giờ bị quét vào stylesheet).

## 4. Token ngữ nghĩa (`@theme inline`) và guard chống literal (`ui_qualification.rs`)

- `tailwind-theme.css` dùng `@theme inline` để mọi utility (`bg-vk-surface`, `text-vk-on-surface`,
  `gap-vk-3`, `rounded-vk-md`, `font-vk-ui`, `text-vk-label`, `font-vk-role-label`,
  `shadow-vk-raised`, `ease-vk-emphasized`, `duration-vk-short`) biên dịch trực tiếp thành
  `var(--vk-*)` mà **không** sinh biến `:root` sao chép giá trị màu. Nhờ đó:
  - `<html data-theme>`, `UiRoot`, `ThemeScope`, `RoleOverride`, `data-density="compact"`,
    `data-motion="reduce"` và `@media (prefers-reduced-motion: reduce)` hoạt động đúng theo scope
    gần nhất.
  - `--default-transition-duration: var(--vk-duration-short)` và
    `--default-transition-timing-function: var(--vk-ease-emphasized)` bảo đảm `transition-*` tôn
    trọng token và reduced motion.
- **Lưu ý quan trọng khi dùng `text-vk-*` vs `text-vk-size-*`:** `text-vk-<role>` (`text-vk-label`,
  `text-vk-caption`, `text-vk-body`, …) đặt đồng thời 4 thuộc tính `font-size`, `line-height`,
  `letter-spacing` và `font-weight` của type role (nhưng không đặt `font-family` — dùng thêm
  `font-vk-ui` / `font-vk-reading` / `font-vk-code` khi cần). Nếu chỉ cần token kích thước chữ đơn
  lẻ (`text.*` trong `tokens.json`), dùng `text-vk-size-*` (`text-vk-size-draft-title`, …); nếu dùng
  `text-vk-<role>` trên phần tử kế thừa weight/leading/tracking khác (như `StatusBadge` giữ weight
  400 của body), ghép tường minh `font-vk-role-body tracking-normal` hoặc
  `leading-[var(--vk-type-body-line-height)]`. Các utility `duration-vk-short` và
  `duration-vk-medium` được sinh qua directive `@utility` ở cuối `tailwind-theme.css`.
- **Guard hai lớp (CHOICE-04 B):**
  1. `tailwind-theme.css` xóa namespace mặc định (`--color-*: initial`, `--font-*: initial`,
     `--text-*: initial`, `--font-weight-*: initial`, `--shadow-*: initial`, `--ease-*: initial`).
  2. `xtask/src/ui_qualification.rs` (chạy trong `verify quick`) quét toàn bộ 6 file CSS
     (`apps/vink-web/style/vink.css`, `packages/ui-system/leptos/style/vink-ui.css`,
     `packages/vink-patterns/leptos/patterns.css`, `packages/ui-system/leptos/preview/style/preview.css`,
     `packages/ui-system/leptos/tailwind/{web,preview}.css`) và mọi literal chuỗi / `class:<token>=`
     trong 5 thư mục nguồn Rust UI (`apps/vink-web/src`, `packages/ui-system/leptos/src`,
     `packages/vink-patterns/leptos/src`, `packages/ui-system/leptos/preview/{src,examples}`, bỏ qua
     file sinh tự động và khối `#[cfg(test)] mod tests` ở cuối file), **từ chối cứng**:
     - Bảng màu mặc định Tailwind (`bg-red-500`, `text-slate-600`, `bg-white`, `text-black`) và
       màu arbitrary (`bg-[#fff]`, `text-[rgb(...)]`, `hsl(`, `oklch(`).
     - Cỡ chữ / họ chữ / độ đậm ngoài token (`text-sm`, `text-[14px]`, `font-sans`, `font-semibold`,
       `font-[600]`). Được phép: `text-vk-*`, `text-vk-size-*`, `font-vk-*`, `font-vk-role-*`,
       `font-normal`, `font-bold`.
     - Motion/shadow literal (`duration-150`, `duration-[123ms]`, `delay-100`, `ease-in-out`,
       `shadow-md`, `shadow-[...]`). Được phép: `duration-vk-*`, `duration-0`, `delay-0`,
       `ease-vk-*`, `ease-linear`, `shadow-vk-raised`, `shadow-none`.
     - Arbitrary CSS property chứa literal (`[color:#fff]`, `[font-size:14px]`,
       `[transition-duration:123ms]`).
     - Được phép: geometry/layout (`w-[320px]`, `min-h-[48px]`, `flex-[1_1_14rem]`, `gap-[6px]`),
       biến Vink (`bg-[var(--vk-surface)]`, `text-(--vk-on-surface)`), màu hệ thống forced-colors
       (`forced-colors:border-[CanvasText]`, `forced-colors:outline-[Highlight]`), hoặc ngoại lệ có
       owner + lý do trong `packages/ui-system/shared/api-coverage.json` (`style_exceptions`, kiểm
       stale tự động). Lưu ý: dù Tailwind vẫn giữ scale số cho spacing/radius để không phá các
       utility khung như `p-0`, `m-0`, `min-w-0`, code Vink phải ưu tiên token `p-vk-*`, `gap-vk-*`,
       `rounded-vk-*` thay vì dùng `p-4` hay `rounded-md` mặc định.

## 5. Cú pháp Leptos 0.8 đã kiểm chứng với Tailwind 4.3.5

1. **Class tĩnh đầy đủ:** đặt trực tiếp trong `class="..."` hoặc `const` chuỗi tĩnh.
   ```rust
   <p class="vk-ui-list-end flex items-center gap-vk-3 m-0 font-vk-ui text-vk-caption tracking-normal text-vk-on-surface-variant">
   ```
2. **Bật/tắt một token bằng `class:` (không cần khoảng trắng giả ở Tailwind >= 4.1.8):**
   ```rust
   <div
       class=base_class
       class:vk-ui-notice--banner=banner
       class:rounded-none=banner
       class:rounded-vk-md=!banner
       class:px-vk-6=banner
       class:px-vk-4=!banner
   >
   ```
   *Mẹo:* dùng cặp loại trừ nhau (`rounded-none=banner` / `rounded-vk-md=!banner`) để tránh phụ
   thuộc thứ tự khai báo trong `@layer utilities`. Cú pháp `class:<ident>=cond` chỉ dùng được cho
   tên class gồm chữ/số/gạch ngang; khi tên class chứa `:`, `[`, `]`, `/`, `.` (như
   `forced-colors:border` hay `pt-[0.125em]`), phải dùng tuple `class=("...", cond)`.
3. **Tuple `class=("...", cond)` chỉ nhận ĐÚNG MỘT class token:** trong Leptos 0.8, cú pháp tuple
   gọi `DOMTokenList` (`classList.add/remove`), sẽ ném `InvalidCharacterError` ở trình duyệt nếu
   chuỗi chứa khoảng trắng!
   - Đúng: `class=("forced-colors:border", true) class=("forced-colors:border-[CanvasText]", true)`
   - Sai: `class=("px-vk-6 rounded-none", banner)` (lỗi runtime DOM!)
4. **Biến thể hữu hạn bằng `match` trả về chuỗi class đầy đủ:**
   ```rust
   fn notice_tone_classes(tone: Tone) -> &'static str {
       match tone {
           Tone::Neutral => "bg-vk-surface-container-high text-vk-on-surface",
           Tone::Error => "bg-vk-error-container text-vk-on-error-container",
           Tone::Warning => "bg-vk-warning-container text-vk-on-warning-container",
           Tone::Info => "bg-vk-info-container text-vk-on-info-container",
           Tone::Success => "bg-vk-success-container text-vk-on-success-container",
       }
   }
   ```
   Có thể ghép chuỗi tĩnh + kết quả `match` qua `format!("vk-ui-notice ... {}", notice_tone_classes(tone))`
   vì mọi tên utility đã xuất hiện nguyên vẹn dưới dạng literal tĩnh trong nguồn.
5. **Phân biệt `format!` cho semantic hook với cấm ghép động tên utility:**
   - **Được phép:** ghép chuỗi với kết quả `match` chứa literal tĩnh đầy đủ, hoặc giữ semantic hook
     hiện hữu (ví dụ `format!("vk-ui-button vk-ui-button--{}", variant.id())` khi rule legacy của
     nút chưa migrate).
   - **Cấm tuyệt đối:** ghép mảnh tên utility Tailwind như `format!("bg-vk-{tone}")` hay
     `format!("gap-vk-{n}")` — Tailwind scanner chỉ đọc văn bản tĩnh và sẽ bỏ qua class đó mà không
     báo lỗi biên dịch Rust.
6. **Biến CSS cho giá trị động và an toàn SSR/hydration:**
   - Khi cần truyền tọa độ/kích thước đo runtime (ví dụ `--vk-ui-tooltip-left` hoặc
     `--vk-ui-bar-height`), chỉ đặt biến CSS qua owner/validation hiện hữu; **không bao giờ** biến
     văn bản bản thảo (manuscript) của người dùng thành CSS inline, class name hay nguồn quét
     Tailwind.
   - Giữ nguyên ranh giới SSR / `LocalResource` cho dữ liệu riêng tư, không gọi browser DOM API trên
     server (`#[cfg(target_arch = "wasm32")]` / `#[cfg(feature = "hydrate")]`), và giữ nguyên cấu
     trúc DOM/focus khi cập nhật class reactive để không làm mất IME composition hoặc vị trí cuộn.

## 6. Nguyên tắc tái sử dụng, CSS giữ lại và test hook

- **Tái sử dụng trước khi lặp utility:**
  - Dùng component chia sẻ của `vink-ui` (`packages/ui-system/leptos`) và composition của
    `vink-patterns` (`packages/vink-patterns/leptos`) trước khi ghép utility mới trong
    `apps/vink-web`.
  - Khi cùng một tổ hợp utility lặp ở nhiều vị trí trong cùng crate, gom thành `const` hoặc
    component/helper (như `DRAFT_META_CLASS` trong `packages/vink-patterns/leptos/src/lib.rs`)
    thay vì sao chép chuỗi dài hay lạm dụng `@apply`.
  - Không dùng `@apply` trừ trường hợp bất khả kháng cho third-party DOM không gắn class Rust được.
- **Phạm vi CSS nên giữ lại (không ép chuyển sang utility):**
  - Token sinh ra (`tokens.css`, `tailwind-theme.css`) và `@font-face`.
  - Rule cho DOM ngoài Leptos: CodeMirror 6 (`.cm-*`), nội dung Markdown render bởi `pulldown-cmark`
    (`.vk-reader__body`), `:root` / `data-theme` / `data-rail` / `data-text` bootstrap.
  - `@keyframes` animation phức tạp, container query hoặc `::backdrop` đặc thù khi utility khó đọc
    hoặc giảm rõ ràng.
- **Giữ semantic hook & dọn rule cũ:**
  - Giữ các class hook mà E2E/smoke (`apps/vink-web/e2e/`, `packages/ui-system/leptos/preview/e2e/`)
    đang truy vấn (`vk-ui-notice`, `vk-ui-status`, `vk-ui-list-end`, `vk-draft-meta`, …) trên phần
    tử đã migrate, đồng thời **xóa rule tạo kiểu cũ** trong file CSS legacy tương ứng để không tồn
    tại hai nguồn sự thật cạnh tranh.

## 7. Ranh giới bằng chứng (không suy diễn)

- `cargo xtask verify quick` chứng minh token match, pin package/lockfile, `source(none)` allowlist,
  unit test compiler/guard và clippy server/hydrate/preview — **không** chứng minh giao diện trên
  trình duyệt hay kích thước/hiệu năng thực.
- `cargo xtask verify preview` và `cargo xtask verify web --scenario ...` chứng minh render/hydration/
  tương tác trên headless Chrome; kết luận thị giác đòi hỏi **mở trực tiếp ảnh PNG** trước/sau trên
  cùng kịch bản.
- Mọi nhận xét về dung lượng CSS/WASM/SSR HTML hay thời gian build phải trích số đo thực từ cùng
  máy/cấu hình (xem [docs/development/tailwind-web-foundation.md](../../../../docs/development/tailwind-web-foundation.md));
  không đưa ra tuyên bố định lượng chưa đo (ví dụ lợi ích hiệu suất AI chưa có thí nghiệm đo).
