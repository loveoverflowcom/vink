# UI system — Leptos renderer (`vink-ui`)

Leptos **0.8** (workspace pin, không hạ về 0.7 của Cantos). Crate không tự bật feature: consumer chọn
`ssr`/`hydrate`/`csr` cho `leptos`. Không phụ thuộc service, router, transport, storage hay database;
`verify quick` kiểm manifest và source.

| Đường dẫn | Nội dung |
| --- | --- |
| `src/tokens.rs` | **Sinh ra.** `ColorRole` (token, `css_var`, light/dark đã giải, alias), `CONTRAST`, `font`, `dim`, `number`, `text`, `motion`, `FONT_FACES` |
| `src/catalog.rs` | **Sinh ra.** `ENTRIES`, `STATUSES`, `fixture::*` |
| `src/values.rs` | Kiểu giá trị (`Rgba::contrast`, `Dimension::var`, `TextRole::style`…) |
| `src/theme.rs` | `ThemePreference`, `Density`, `MotionPreference`, `Preferences`, `RoleOverride`, precedence |
| `src/root.rs` | `UiRoot`, `ThemeScope` |
| `style/tokens.css` | **Sinh ra.** `@font-face`, `:root`, theme/scope, recipe theo scope, density, motion |
| `style/vink-ui.css` | CSS renderer, mọi selector dưới `.vk-ui` |
| `preview/` | crate `vink-ui-preview` (CSR, feature `csr`), `index.html`, `style/preview.css`, `e2e/preview-smoke.mjs` |

`tokens.rs`/`catalog.rs` được `#[rustfmt::skip]` ở khai báo module: `--check` so byte với generator
nên formatter không được đổi chúng.

```rust
use vink_ui::{Preferences, ThemePreference, ThemeScope, UiRoot};

view! {
    <UiRoot preferences=Signal::derive(move || Preferences { theme: theme.get(), ..Default::default() })>
        <ThemeScope theme=Some(ThemePreference::Dark)>"…"</ThemeScope>
    </UiRoot>
}
```

`UiRoot` không thay bootstrap `<html data-theme>` của app: Web vẫn đặt theme trước CSS để không
nháy khi tải; scope chỉ đè cho cây con. Theme scope có `data-theme` tự tô nền `--vk-surface`.
