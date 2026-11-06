# UI system — Leptos renderer (`vink-ui`)

Leptos **0.8** (workspace pin, không hạ về 0.7 của Cantos). Crate không tự bật feature: consumer chọn
`ssr`/`hydrate`/`csr` cho `leptos`. Không phụ thuộc service, router, transport, storage hay database;
`verify quick` kiểm manifest và source.

| Đường dẫn | Nội dung |
| --- | --- |
| `src/tokens.rs` | **Sinh ra.** `ColorRole` (token, `css_var`, light/dark đã giải, alias), `CONTRAST`, `font`, `dim`, `number`, `text`, `motion`, `FONT_FACES` |
| `src/catalog.rs` | **Sinh ra.** `ENTRIES`, `STATUSES`, `fixture::*` |
| `src/icons.rs`, `src/identity_vectors.rs` | **Sinh ra.** `IconName` (path, upstream), golden vectors + người/ảnh `[Mẫu]` |
| `src/text.rs`, `src/layout.rs` | `Text`, `Heading`, `Surface`, `Container`, `Stack`, `Grid`, `Divider` |
| `src/action.rs` | `Button`, `IconButton`, `Link` (`safe_href`), `FilterChip`, `ActionState` |
| `src/field.rs` | `TextField`, `TextArea`, `SearchField`, `Checkbox`, `Switch`, `RadioGroup`, `Select`, `FormSummary` |
| `src/media.rs`, `src/identity.rs` | `Icon`, `Avatar`, `AvatarGroup`, `AuthorByline`, `ImageFrame` (`safe_image_src`); `initials`, `tone` thuần |
| `src/state.rs`, `src/state_vectors.rs` | Contract thuần UIS-03 (`RegionState::layout`, `Problem`, `placement_for`, `NoticeSpec`, `LoadingGate`…); vectors **sinh ra** |
| `src/feedback.rs` | `ContentRegion`, `StatePanel`, `InlineNotice`, `Banner`, `Skeleton`, `LoadingIndicator`, `EdgeProgress`, `StatusBadge`, `ListEnd`, `Snackbar`, `Dialog`; `MotionActivity`/`provide_motion_activity`, `document_visibility()`, `use_loading_gate`, `use_snackbar_timeout` |
| `src/toggle.rs`, `src/toggle_vectors.rs` | LVE #73: `ToggleValue`/`TogglePhase`/`CountFact`, `ToggleView::resolve` thuần, `ToggleCopy`; `ToggleAction` (một `<button>`, tên theo giá trị đã xác nhận, số đếm + ghi chú qua `aria-describedby`, live region, “Kiểm tra lại”); vectors `toggle-contract-v1` **sinh ra** |
| `src/navigation.rs`, `src/navigation_vectors.rs` | UIS-04: `WindowClass`, `NavLayout::resolve`, `RailPreference`, `OverlayLayer`/`CloseReason`/`FocusReturn` thuần; `NavItem`, `NavUtility`, `NavigationRail` (toggle thường trực + tooltip ngoài rail), `BottomNavigation` (đo chiều cao → `--vk-ui-bar-height`), `TopBar`, `AdaptiveScaffold` (`data-layout` auto bằng CSS hoặc cố định); vectors **sinh ra** |
| `src/values.rs` | Kiểu giá trị (`Rgba::contrast`, `Dimension::var`, `TextRole::style`…) |
| `src/theme.rs` | `ThemePreference`, `Density`, `MotionPreference`, `Preferences`, `RoleOverride`, precedence |
| `src/root.rs` | `UiRoot`, `ThemeScope` |
| `style/tokens.css` | **Sinh ra.** `@font-face`, `:root`, theme/scope, recipe theo scope, density, motion |
| `style/vink-ui.css` | CSS renderer: gốc `.vk-ui` và class `vk-ui-*` của component; không selector tag toàn cục. Phần shell/điều hướng tự mang kiểu chữ/focus ring nên app dùng được dưới token cấp document (`<html data-theme>`, `:root[data-rail]`) trước khi có `UiRoot`; app gộp file này vào `/pkg/vink.css` |
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

Component phải nằm trong một `UiRoot` (focus ring và chữ gốc đến từ `.vk-ui`); field cần `id` ổn
định do caller cấp vì SSR/hydration không tự sinh ID. Contract đầy đủ: [components.md](../shared/components.md).

`UiRoot` không thay bootstrap `<html data-theme>` của app: Web vẫn đặt theme trước CSS để không
nháy khi tải; scope chỉ đè cho cây con. Theme scope có `data-theme` tự tô nền `--vk-surface`.
