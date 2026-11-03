# Contract shell, điều hướng và lớp phủ (UIS-04)

**Trạng thái:** UIS-04/06 ([#22](https://github.com/loveoverflowcom/vink/issues/22)) — NEW. Đã triển khai ở
cả hai renderer, đăng ký trong cả hai preview (`navigation.*`) với trạng thái `implemented-unverified`;
`navigation.adapter` trên Leptos là `unsupported` vì Web kiểm router/guard bằng app thật. Khác UIS-02/03,
shell Web (`apps/vink-web/src/shell.rs`) đã dùng các primitive này; màn route vẫn chưa (thuộc #23).

Thư viện **trình bày** khung và **phát intent**. Nó không import router, không đọc/ghi storage, không
biết session, quyền hay bản nháp, không tự điều hướng và không có guard thứ hai. Host (app Web, adapter
CMP) sở hữu: danh sách đích có kiểu và quyền thật, route ↔ URL/back stack, `NavigationGuard`, preference
lưu ở đâu, focus/cuộn sau khi đổi route.

## LayoutSpec (đã trình bày trước code)

```text
AdaptiveScaffold #shell                   — .vk-ui-scaffold[data-layout][data-compact-rail][data-hide-bar]
  SkipLink #skip                          — "Bỏ qua tới nội dung" → #<main id> (host cấp id)
  NavigationRail #navigation              — <aside aria-label>; rộng/hẹp/ẩn theo NavLayout
    Slot #brand                           — host (logo, link về /)
    RailToggle #rail-toggle               — disclosure thường trực: aria-expanded, aria-controls, tên + tooltip
    NavList #nav-primary                  — <nav aria-label>: NavItem* theo thứ tự host
    Slot #account                         — chân rail: NavUtility (mở lớp phủ) hoặc NavItem
  Column #content
    TopBar #top-bar                       — chỉ khi Bar: brand + hành động của route
    Slot #main                            — <main id tabindex=-1> của host; giữ instance khi đổi khung
  BottomNavigation #bottom-navigation     — <nav aria-label>: NavItem* (Web: fixed, đo chiều cao)
  Tooltip #rail-tooltip                   — ngoài vùng rail cuộn (không bị containment cắt)
  Slot #overlay-host                      — dialog/sheet của host, điều phối bằng OverlayLayer
```

Thứ tự Vink do host cấp: rail Khám phá → Bàn viết, Tôi ở chân; thanh dưới Khám phá → Bàn viết → Tôi.
Thư viện không thêm tab (Thư viện/Thảo luận/Kiểm duyệt) và không ẩn/hiện đích theo vai trò: host truyền
danh sách đã lọc theo capability thật. Điều hướng route **không** dùng `role=tablist`; tab chỉ dành cho
panel trong một trang.

## Chọn khung (`NavLayout::resolve`)

| Kiểu | Giá trị | Owner |
| --- | --- | --- |
| `WindowClass` | `Compact` · `Medium` · `Expanded` | Host đo bề rộng **khả dụng**: Web `from_css_px` (760/1100, shell #7), CMP `fromDp` (600/840, Material) |
| `NavContext` | `{ compact_rail, hide_bar }` | Route của host (Web: editor `/ban-viet/:id` bật cả hai) |
| `RailPreference` | `Expanded` · `Compact` · không có | Người dùng; host lưu (`vink.ui.rail`) và bootstrap trước CSS |
| `NavLayout` | `RailExpanded` · `RailCompact` · `Bar` · `BarHidden` | Thư viện, hàm thuần |

1. `Compact` → `Bar`, hoặc `BarHidden` khi `hide_bar` (editor không bị thanh che). Preference rail
   không áp cho thanh.
2. Có preference → rail theo preference (**người dùng > mặc định theo ngữ cảnh**).
3. Không có → `RailExpanded` chỉ khi `Expanded` và không `compact_rail`; còn lại `RailCompact`.

`next_preference(layout_đang_thấy)` đảo rail đang **thấy** (không phải giá trị reactive có thể trễ
sau resize); thanh dưới không có toggle. Breakpoint nội dung 1200 không đổi ngưỡng rail 1100.
Native chọn theo window size class/dp của nền tảng, không chép px. Bảng đầy đủ:
[fixtures/navigation.json](fixtures/navigation.json) (`layouts`, 36 tổ hợp).

**Web, trước hydration:** CSS (`vink-ui.css`) giải cùng bảng bằng media query và `:root[data-rail]`
(host đặt trước CSS như `data-theme`), nên HTML SSR đúng bề rộng không chờ JS; `data-layout="auto"`.
Preview và CMP đặt `NavLayout` tường minh (`data-layout="rail:compact"`…).

## Primitive

| Component | Leptos | CMP | Ghi chú |
| --- | --- | --- | --- |
| `NavDestination` | `{ key, label, icon, href }` | `NavDestination(key, label, icon)` | `key` ổn định cho test/active; Web có `href` thật |
| `NavItem` | `<a href aria-current=page>` | `onNavigate(key)` | Web: không chặn click — router/guard của host nhận link; modifier/middle click, copy link, tab mới giữ nguyên |
| `NavUtility` | `<button aria-haspopup=dialog aria-expanded aria-current>` | `onOpen()` | Tiện ích ở chân rail (Tôi): mở lớp phủ, không phải route |
| `NavigationRail` | `expanded: Signal<bool>`, `on_toggle` | `expanded`, `onToggle` | Toggle luôn hiện (cả khi thu), ≥44 CSS px / 48dp, nhãn + tooltip hover/focus, Escape ẩn tooltip; icon `menu` ↔ `menu-open` |
| `BottomNavigation` | ≥`size.bottom-bar`, safe-area, đo chiều cao → `--vk-ui-bar-height` | `NavigationBar` | Đích chia đều, nhãn xuống dòng, không cắt chữ 200% |
| `TopBar` | `<header>` brand + actions | `VinkTopBar` | Không chứa đích chính |
| `AdaptiveScaffold` | `ScaffoldLayout::Auto { context }` \| `Fixed(NavLayout)` | `layout: NavLayout` | Đổi khung chỉ đổi khung: `#main` và editor không remount |

Kích thước: rail `size.rail-expanded` 220 / `size.rail-compact` 96 (px Web, dp CMP), đích
`size.nav-item` 52. Rail thu đặt icon trên/nhãn dưới; rail cuộn khi thấp/ngang nên toggle và Tôi luôn tới
được. Lớp z: rail/thanh 5 < tooltip/skip-link 10 < dialog native (top layer).

## Lớp phủ (`OverlayLayer`)

Một chủ nhỏ, không phải modal service toàn cục: **một modal hoạt động** tại một thời điểm.

| Thao tác | Kết quả |
| --- | --- |
| `open(id)` | `ok` (cả khi chính `id` đang mở) · `busy(active)` khi modal khác đang mở |
| `close(id, reason)` | `closed(id, reason)` · `stale` khi `id` không còn là modal hoạt động (đóng muộn bị bỏ) |
| `back()` / `escape()` | Đóng modal hoạt động với lý do `back`/`escape`; không có modal → `pass` (Back đi tiếp tới router/back stack, Escape không làm gì) |

`CloseReason`: `dismiss` (nút Đóng) · `escape` · `back` · `outside` · `resize` · `navigate`.
`focus_return(reason, trigger_visible)`: `navigate` → `leave` (route mới tự quyết focus); còn lại →
`trigger` nếu trigger còn hiển thị, ngược lại `main`. Web: ngoài dialog inert (`showModal`), Tab giữ
bên trong, resize xuống mobile đóng và trả focus main, cuộn trang được giữ.

**Back trên Web:** `NavigationGuard` của app (một guard duy nhất) hỏi lớp phủ trước: `popstate` khi lớp
phủ mở → đóng (`back`), chặn router và đẩy URL trang về như khi giữ bản nháp; lần Back sau mới tới
guard editor. Link/điều hướng bằng code đóng lớp phủ (`navigate`) rồi mới hỏi guard editor. Thư viện
không nghe `popstate`. **CMP:** adapter của host gọi `back()` trước khi pop back stack; Android
predictive back/iOS gesture chưa được kiểm (mục verification riêng, không giả bằng phím JVM).

## Bảo toàn

- Đổi theme/rail/thanh, mở/đóng lớp phủ, resize không lưu, không gọi guard, không remount `#main`.
  Web giữ cùng `EditorView` (caret/selection/undo/IME); CMP giữ `TextFieldValue` ở state của host.
- Rời thật đi qua guard hiện có; lưu-rồi-rời chỉ sau ack đúng phiên; ack muộn không điều hướng.
- Lối thoát hiện có (xuất `.txt`, rời không lưu có xác nhận) không bị vô hiệu hóa.
- URL/back stack không chứa bản thảo, session hay token; return context chỉ là đích nội bộ đã kiểm.

## Ngoài phạm vi

Đăng nhập/capability thật (D-05), tab Thư viện/Thảo luận, menu/tooltip chung cho mọi control, sheet
native, Android predictive back, iOS swipe-back, TalkBack/VoiceOver, editor CMP production. Preview
CMP có adapter chẩn đoán (đích có kiểu, back stack, deep link, chữ chưa lưu) — không phải app native.
