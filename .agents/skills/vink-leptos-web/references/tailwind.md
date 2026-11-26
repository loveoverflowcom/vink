# Tailwind CSS v4 + Leptos 0.8 trên Vink

Đây là **nguồn duy nhất** cho quy ước Tailwind CSS v4 trên Web và Leptos preview của Vink. Các
skill khác dẫn về tài liệu này thay vì lặp lại luật compiler, `@layer` hay ví dụ Leptos.

- **Phiên bản pin hiện hành:** `leptos = 0.8.22` (`Cargo.lock`), `tailwindcss = 4.3.5` và
  `@tailwindcss/cli = 4.3.5` (`packages/ui-system/leptos/tailwind/package.json` +
  `package-lock.json`, cài bằng `npm ci --ignore-scripts --no-audit --no-fund`).
- **Ngưỡng tối thiểu Tailwind cho Leptos:** `>= 4.1.8` (bản sửa trích xuất cú pháp `class:` của
  Leptos, [Tailwind #18093](https://github.com/tailwindlabs/tailwindcss/pull/18093), phát hành ở
  [v4.1.8](https://github.com/tailwindlabs/tailwindcss/releases/tag/v4.1.8), khóa tại `xtask/src/tailwind.rs::MIN_VERSION`).
- **Ranh giới nền tảng:** Chỉ áp dụng cho Web (`apps/vink-web`, `packages/ui-system/leptos`,
  `packages/vink-patterns/leptos`, `apps/vink-preview/leptos`). Không đổi Compose
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
  ├─ CSR:     target/spa/_v/<git-sha>/pkg/vink.css
  ├─ SSR:     target/site/pkg/vink.css                (--minify khi --release)
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
- **Ưu tiên cascade khi song hành.** Trong cùng origin và context, declaration thường ở
  `@layer utilities` thắng declaration thường ở `@layer legacy`. Kiểm `forced-colors:*` khi
  chuyển một thuộc tính đang có fallback `forced-colors` thường ở legacy. Với `!important`,
  thứ tự layer đảo ngược. Fallback reduced-motion `!important` trong legacy thắng utility
  thường và utility `!important` trong layer đứng sau. Declaration thường ngoài layer thắng
  declaration thường trong layer. Declaration `!important` trong layer thắng declaration
  `!important` ngoài layer. [CSS Cascade Level 5](https://www.w3.org/TR/css-cascade-5/#cascade-sort)
- **DOM CodeMirror 6.** `style-mod` đưa `<style>` ngoài layer vào `document.head` khi mount.
  Các override thường của Vink cho focus, scroller, line và Markdown Live Preview nằm cùng
  mức ngoài layer ở cuối [entry Web](../../../../packages/ui-system/leptos/tailwind/web.css).
  Với cùng mức cascade, kiểm specificity và thứ tự nguồn để xác nhận selector thắng.
- **Bẫy cascade bắt buộc phải kiểm khi migrate con trước cha:** Một selector hậu duệ ở
  `@layer legacy` (ví dụ `.vk-editor-recovery > .vk-ui-notice { padding: 0; background: transparent; }`
  hoặc flex/gap của `.vk-author-work__status`) sẽ **THUA** utility của component con trong
  `@layer utilities` bất kể specificity! Trước khi xóa rule legacy của một component, hãy grep
  toàn bộ các file CSS nguồn (`packages/ui-system/leptos/style/vink-ui.css`,
  `packages/vink-patterns/leptos/patterns.css`, `apps/vink-web/style/vink.css`,
  `apps/vink-preview/leptos/style/preview.css`) tìm mọi rule cha/modifier đè lên class đó
  và chuyển override sang utility của cha (ví dụ `[&>.vk-ui-notice]:p-0 [&>.vk-ui-notice]:bg-transparent [&>.vk-ui-notice]:text-inherit`)
  hoặc xóa rule xung đột cùng lát.
- **Thoát dấu gạch dưới kép của BEM trong arbitrary variant:** Tailwind đọc nguyên văn nguồn.
  Viết raw string Rust `class=r"[&_.vk-ui-notice\_\_icon]:hidden"` để extractor và Leptos
  nhận cùng token. Dấu gạch chéo trong chuỗi Rust thường phải thoát lần nữa, khiến class mà
  Tailwind nhìn thấy khác class Leptos render. Kiểm selector lớp cha và selector hậu duệ từ CSS đã sinh.

## 3. Quét nguồn có kiểm soát (`source(none)` + allowlist `@source`)

- Cấm `@import "tailwindcss";` quét toàn repo. Entry bắt buộc dùng
  `@import "tailwindcss/utilities.css" layer(utilities) source(none);` và chỉ khai báo `@source`
  tới các thư mục nguồn Rust UI hợp lệ:
  - Web (`web.css`): `apps/vink-web/src`, `packages/ui-system/leptos/src`,
    `packages/vink-patterns/leptos/src`.
  - Preview (`preview.css`): `packages/ui-system/leptos/src`,
    `apps/vink-preview/leptos/src`, `apps/vink-preview/leptos/examples`,
    `packages/vink-patterns/leptos/src`.
- `tailwind::check` trong `verify quick` yêu cầu đúng tập `@source` của từng entry (docs,
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
  lẻ (`text.*` trong `tokens.json`), dùng `text-vk-size-*` (`text-vk-size-draft-title`, …). Khi
  migrate từng thuộc tính, giữ đúng các giá trị kế thừa của selector cũ. Ví dụ, `StatusBadge` dùng
  `text-[length:var(--vk-type-label-size)]` và `leading-[var(--vk-type-label-line-height)]` để đặt
  size và leading, đồng thời giữ weight/tracking kế thừa. Các utility `duration-vk-short` và
  `duration-vk-medium` được sinh qua directive `@utility` ở cuối `tailwind-theme.css`.
- **Guard hai lớp (CHOICE-04 B):**
  1. `tailwind-theme.css` xóa namespace mặc định (`--color-*: initial`, `--font-*: initial`,
     `--text-*: initial`, `--font-weight-*: initial`, `--shadow-*: initial`, `--ease-*: initial`).
  2. `xtask/src/ui_qualification.rs` (chạy trong `verify quick`) quét toàn bộ 6 file CSS
     (`apps/vink-web/style/vink.css`, `packages/ui-system/leptos/style/vink-ui.css`,
     `packages/vink-patterns/leptos/patterns.css`, `apps/vink-preview/leptos/style/preview.css`,
     `packages/ui-system/leptos/tailwind/{web,preview}.css`) và mọi literal chuỗi / `class:<token>=`
     trong 5 thư mục nguồn Rust UI (`apps/vink-web/src`, `packages/ui-system/leptos/src`,
     `packages/vink-patterns/leptos/src`, `apps/vink-preview/leptos/{src,examples}`, bỏ qua
     file sinh tự động và thân module `#[cfg(test)]`), **từ chối cứng**:
     - Bảng màu mặc định Tailwind (`bg-red-500`, `text-slate-600`, `bg-white`, `text-black`) và
       màu arbitrary (`bg-[#fff]`, `text-[rgb(...)]`, `hsl(`, `oklch(`).
     - Cỡ chữ / họ chữ / độ đậm ngoài token (`text-sm`, `text-[14px]`, `font-sans`, `font-semibold`,
       `font-[600]`). Được phép: `text-vk-*`, `text-vk-size-*`, `font-vk-*`, `font-vk-role-*`,
       `font-normal`, `font-bold`.
     - Motion/shadow literal (`duration-150`, `duration-[123ms]`, `delay-100`, `ease-in-out`,
       `shadow-md`, `shadow-[...]`). Được phép: `duration-vk-*`, `duration-0`, `delay-0`,
       `ease-vk-*`, `ease-linear`, `shadow-vk-raised`, `shadow-none`.
     - Arbitrary CSS property chứa literal (`[color:#fff]`, `[font-size:14px]`,
       `[transition-duration:123ms]`). Màu có tên trong shorthand `background`/`border`, như
       `[background:red]`, cũng bị chặn. Cùng guard kiểm các declaration CSS giữ lại.
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
   <p class="vk-ui-list-end flex items-center gap-vk-3 m-0 [font-family:var(--vk-type-caption-font)] text-[length:var(--vk-type-caption-size)] leading-[var(--vk-type-caption-line-height)] text-vk-on-surface-variant">
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
   thuộc thứ tự khai báo trong `@layer utilities`. Ví dụ từ [DraftResume](../../../../packages/vink-patterns/leptos/src/lib.rs)
   dùng `class:vk-draft-resume__title=true`. Với utility có variant hoặc arbitrary value như
   `forced-colors:border` và `pt-[0.125em]`, dùng tuple `class=("...", cond)`.
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
     server (trong `vink-web`, dùng `#[cfg(feature = "browser")]` cho DOM dùng chung ở CSR và hydration), và giữ nguyên cấu
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
  - Rule cho DOM ngoài Leptos: CodeMirror 6 (`.cm-*`, cuối `tailwind/web.css`), nội dung Markdown render bởi `pulldown-cmark`
    (`.vk-reader__body`), `:root` / `data-theme` / `data-rail` / `data-text` bootstrap.
  - `@keyframes` animation phức tạp, container query hoặc `::backdrop` đặc thù khi utility khó đọc
    hoặc giảm rõ ràng.
- **Giữ semantic hook & dọn rule cũ:**
  - Giữ các class hook mà E2E/smoke (`apps/vink-web/e2e/`, `apps/vink-preview/leptos/e2e/`)
    đang truy vấn (`vk-ui-notice`, `vk-ui-status`, `vk-ui-list-end`, `vk-draft-meta`, …) trên phần
    tử đã migrate, đồng thời **xóa rule tạo kiểu cũ** trong file CSS legacy tương ứng để không tồn
    tại hai nguồn sự thật cạnh tranh.

## 7. Ranh giới bằng chứng (không suy diễn)

- `cargo xtask verify quick` chứng minh token match, pin package/lockfile, `source(none)` allowlist,
  unit test compiler/guard và clippy server/hydrate/preview — **không** chứng minh giao diện trên
  trình duyệt hay kích thước/hiệu năng thực.
- `cargo xtask verify preview` kiểm Leptos preview render phía client, tương tác và fixture
  media hydration riêng trong headless Chrome (`target/ui-preview/leptos/captures/`).
  `cargo xtask verify web --scenario NAME` kiểm Web theo kịch bản đã chọn. Chọn `standalone` cho CSR,
  chọn kịch bản của hành trình cho SSR/hydration và tương tác (`target/verify-web/<run-id>/`).
  Xem [tooling local](../../../../docs/development/feature-map.md#tooling-local-có-phạm-vi-184).
  Kết luận thị giác cần mở ảnh PNG trước/sau cùng kịch bản. Input tổng hợp trong Chrome
  và phép đo geometry có phạm vi riêng với IME hệ điều hành và thiết bị thật.
- Mọi nhận xét về dung lượng CSS/WASM/SSR HTML hay thời gian build phải trích số đo thực từ cùng
  máy/cấu hình (xem [docs/development/tailwind-web-foundation.md](../../../../docs/development/tailwind-web-foundation.md));
  không đưa ra tuyên bố định lượng chưa đo (ví dụ lợi ích hiệu suất AI chưa có thí nghiệm đo).

## 8. Walkthrough một variant dùng chung Web và preview

1. Từ [source map](../../vink-ui-design/references/source-map.md), tìm `Notice` trong
   [feedback.rs](../../../../packages/ui-system/leptos/src/feedback.rs). `InlineNotice` truyền
   `banner=false` và `Banner` truyền `banner=true`. Giữ `vk-ui-notice`, `data-tone` và
   `role=live_role(announce)` để CSS, trạng thái và smoke dùng chung hợp đồng.
2. Trong worktree thử, thêm `class=("sm:gap-x-vk-6", banner)` cạnh các class điều kiện.
   Chỉ `Banner` nhận class khi `banner=true`. Hai entry cùng quét owner này. Token
   `--vk-space-6` vẫn lấy từ SSOT.
3. Kiểm selector `.sm\:gap-x-vk-6`, media query 40rem và `column-gap: var(--vk-space-6)`
   trong CSS cả Web và preview. `xtask` là owner build. Khi có môi trường phù hợp, dùng
   `cargo xtask build web --release` và `cargo xtask build preview` để đối chiếu hai artifact.
4. Muốn xác nhận hành vi, mở cả hai trạng thái ở `state.notice/default` và
   `state.notice/banner`. Kiểm `Banner` ở dưới/trên breakpoint, `InlineNotice` giữ gap cũ,
   role và tone vẫn đúng. Ghi riêng kết quả browser, CSS build và phép đo hình học.

Walkthrough trên `develop@2712145b` đã kiểm parser và lifecycle sinh CSS. Chi tiết SHA,
fixture và phần browser còn chờ nằm trong [bảng đối chiếu](../../../../docs/development/tailwind-web-foundation.md#9-đối-chiếu-sau-squash-431).
