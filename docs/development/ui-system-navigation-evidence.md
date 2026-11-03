# Bằng chứng UI System 04/06 — shell, điều hướng và bảo toàn input/focus Web/CMP

**Ngày:** 03/11/2026 (UTC+07) · **Issue:** [#22](https://github.com/loveoverflowcom/vink/issues/22) ·
**Nhánh:** `codex/ui-system-navigation`, nền `develop@3a02409` (đã gồm UIS-03 #31; snapshot của issue
là `5afb00b`, không reset). Đây là kết quả chạy trên máy dev, không phải nghiệm thu thiết kế, thiết
bị thật hay production. Contract: [navigation.md](../../packages/ui-system/shared/navigation.md);
spec shell: [design](../design/README.md#shell-thích-ứng-7).

## Môi trường

| Mục | Giá trị |
| --- | --- |
| Máy | macOS 26.5.2 (25F84), Apple Silicon arm64 |
| Rust / wasm-bindgen | 1.99.1 / 0.2.129 |
| Node / Chrome | v26.12.0 / Chrome 156.0.8078.79 headless, DPR 1 |
| JDK / Gradle / Kotlin / Compose | OpenJDK 17.0.19 / 8.14.3 / 2.2.20 / Compose Multiplatform 1.9.0, Material3 1.8.2 |
| Android / iOS | Android SDK (AGP 8.11.1) / Xcode 26.6, Kotlin/Native 2.2.20, iOS simulator arm64 |
| PostgreSQL | 17.11 (Homebrew), cluster tạm `127.0.0.1:55432` trong scratchpad |
| Nguồn | `tokens.json` 0.4.0 (SHA-256 bắt đầu `7771a63e66d5f541`), catalog 0.4.0, `fixtures/navigation.json` |

## HOW / WHY đã ghi trước khi đổi

- **HOW:** click link → `NavigationGuard` (capture, trước router) → router của Leptos → route mới
  trong `#noi-dung`; Back/Forward → `popstate` của guard (đăng ký trước router) → router. Đường lỗi:
  lưu chậm → hủy rời → ack muộn không điều hướng (`LeaveGuard` trong `desk_state.rs`). Shell nằm
  ngoài `<Routes>` nên đổi rail/theme/overlay không remount editor.
- **WHY:** thứ tự tab là chính sách điều hướng, khác launch route (`/` vẫn mặc định); utility theme
  khác điều hướng (Tôi trên web/tablet là lớp phủ, không phải route). Primitive dùng chung không
  được biết router/session để CMP và Web có cùng contract mà không kéo nav controller vào thư viện.
- **Quyết định scoped trong PR:** `NavLayout` là bảng tường minh (36 vector); ngưỡng Web giữ 760/1100
  CSS px, CMP dùng 600/840dp; một `OverlayLayer` nhỏ thay vì modal service toàn cục; Back đóng lớp
  phủ đi qua chính `NavigationGuard` (không có guard thứ hai). Icon shell đổi sang Material registry
  (`explore`, `edit`, `person`, `menu`/`menu-open`). Không chốt D/INV/S nào, không chọn auth.

## Kết quả

| Kiểm | Lệnh | Kết quả | Ghi chú |
| --- | --- | --- | --- |
| Nguồn shared hợp lệ, 13 file sinh ra khớp | `cargo xtask ui-tokens --check` | PASS | Thêm `navigation_vectors.rs`, `VinkNavigationVectors.kt`; icon `menu-open` dùng khung upstream `H0V0z` (trường `frame`) |
| Lane quick | `cargo xtask verify quick` | PASS | fmt, clippy server/hydrate/preview `csr` (wasm32), test không database (gồm `vink-ui` `navigation::tests` 8, `vink-web` `shell::tests` 3 và round-trip return context), ranh giới UI system, link, skill |
| Lane web | `cargo xtask verify web` | PASS | 343 kiểm tra `ok` ở lần chạy cuối; app thật, router/guard thật, PostgreSQL scratch |
| Lane postgres | — | NOT_RUN | Không đổi query/transaction/persistence |
| Preview Leptos | `cargo xtask ui-preview leptos verify` | PASS | 43 catalog ID, 127 link kịch bản, hàm `navigation`, tổng 291 kiểm `ok`; 428 ảnh + `contexts.json` ở `target/ui-preview/leptos/captures/`; chạy lại sau khi sửa skip link |
| Preview CMP desktop + native | `VINK_UI_NATIVE_TARGETS=1 cargo xtask ui-preview cmp verify` | PASS | JVM: `VinkNavigationTest` 5; desktop: `NavigationPreviewTest` 7, `CaptureTest` 1 (290 lần chụp, 279 file vì entry `navigation.*` mặc định trùng tên với vòng UIS-02, ở `packages/ui-system/cmp/preview/build/captures/`); `:ui:iosSimulatorArm64Test`: `VinkNavigationTest` 5/5 |
| Vectors chung | Rust `navigation::tests`, Kotlin `VinkNavigationTest` | PASS | 12 cửa sổ biên, 36 khung, 4 toggle, 4 chuỗi lớp phủ, 12 ca trả focus giống nhau ở Rust, JVM, Kotlin/Native |
| Android predictive back, iOS swipe-back, lifecycle/recreation, TalkBack/VoiceOver | — | NOT_RUN | Chưa có host native (#24); adapter CMP là host chẩn đoán, phím JVM không thay cử chỉ hệ thống |
| Cửa sổ CMP Desktop thật | `cargo xtask ui-preview cmp run` | NOT_RUN | Ảnh là `ImageComposeScene` off-screen bằng renderer thật |
| CI | — | NOT_RUN | Chưa đẩy nhánh lúc ghi |

## Kịch bản đã khẳng định

**Web, app thật** (`browser-smoke.mjs`, `adaptiveShell` và `shellWithDirtyEditor`): `/` vẫn là route
mặc định, rail Khám phá → Bàn viết, Tôi ở chân, thanh dưới Khám phá → Bàn viết → Tôi; đích là
`<a href>` thật, không `role=tab`; toggle có tên theo hành động, `aria-expanded`/`aria-controls`,
khi thu vẫn hiện, ≥44px, không bị che và trang không đổi; 220/96 mặc định theo ngữ cảnh, preference
ghi đè và qua reload/resize; tooltip hover/focus nằm ngoài rail, Escape ẩn; skip link chuyển focus
vào `#noi-dung` mà không đổi route; click có phím bổ trợ không bị guard/router chặn (để trình duyệt mở
tab mới); `/ban-vietnam`, `/tac-pham/a/b`, `/toi/khac` trả 404 từ server và không sáng đích nào; ba
lần bấm dồn dập chỉ còn đích cuối được chọn; Tôi: focus radio đang chọn, Tab giữ trong modal,
Escape/bấm ngoài/Đóng trả focus về trigger, Back khi Tôi mở chỉ đóng Tôi và giữ `/toi`, Back sau mới
về `/`, Forward trở lại; resize xuống 390px đóng Tôi và focus main; editor bẩn: rail/theme/Tôi/resize
giữ cùng `EditorView`, chữ, selection, trạng thái chưa lưu và không gửi PUT/POST; mọi đích thật
(gồm Mở trang Tôi) đi qua guard hiện có; thanh dưới không che cuối trang ở 320px/chữ 200%.

**Unit (`quick`):** `Destination::for_path` khớp nguyên segment như route (tiền tố và đường sâu hơn
là NotFound); chỉ editor đặt ngữ cảnh rail gọn + ẩn thanh; return context `?q=` đi qua
`RequestUrl` của router đọc lại đúng chuỗi (tiếng Việt, `&`, `%`, `+`, `#`, NFD), `returnTo` không
được đọc, link lạ/hỏng chỉ dựng URL nội bộ `/…#community-stream`.

**Leptos preview** (`navigation` trong `preview-smoke.mjs`): thứ tự và link thật; click thường gửi
key một lần khi host không có router, `aria-current` theo host; reader thuộc Khám phá; host cấp ít
đích thì chỉ render đích đó; ngữ cảnh editor; toggle 96 → 220 → 96 bằng chuột, Enter, Space giữ
trang, chữ và selection `5..12`, mỗi lần kích hoạt một dispatch; tooltip ngoài rail; skip link bị
clip tới khi focus; một modal (mở hộp thoại thứ hai bị từ chối), Back đóng lớp phủ trước rồi mới tới
host, Escape trả focus về trigger.

**CMP** (`NavigationPreviewTest`): deep link round-trip cho mọi đích có kiểu (query tiếng Việt và ký
tự đặc biệt), từ chối `//host`, `https://`, đường không bắt đầu bằng `/`, tiền tố/đường sâu, `%`
hỏng; ý định tới đích hiện tại không thêm entry; Back đóng lớp phủ trước, rồi pop, rồi `Exit`; điều
hướng đóng lớp phủ; chữ chưa lưu giữ Back và điều hướng tới khi chọn, ý định bị bỏ không chạy muộn,
host vẫn giữ chữ sau khi rời; đổi `Bar`/`RailCompact`/`BarHidden`/`RailExpanded` không remount nội
dung (`remember` còn nguyên), rail 220/96dp; toggle trong workbench giữ chữ và selection; thanh dưới
đúng thứ tự và điều hướng theo key; lớp phủ một modal và Back đóng trước.

## Ảnh đã mở và nhận xét

| Ảnh | Nhận xét → sửa |
| --- | --- |
| Web `1440-rail-tooltip-light` | Rail 96: logo, nút `menu`, Khám phá (sáng) và Bàn viết icon trên/nhãn dưới, Tôi ở chân; tooltip ngoài rail ngang nút. Lần chạy đầu tooltip neo theo mép nút (72px) nên nằm trong rail → neo theo mép phải của rail |
| Web `1440-overlay-dark`, `1280-editor-dirty-overlay-dark` | Rail 220 có “Vink” + `menu-open`; Tôi neo chân rail, trigger sáng khi mở; editor: Bàn viết sáng, rail 96. Không sửa |
| Web `320-me-dark-text200-bottom` | Thanh dưới ba đích, nhãn xuống dòng theo từ, footer không bị che. Không sửa |
| Leptos `navigation.scaffold--toggle--light--1440--96`/`--220` | Đúng 96/220, chữ và vùng chọn giữ. Phát hiện skip link hiện đè ghi chú trong khung preview (ẩn bằng `translateY` chỉ đúng ở đầu viewport) → ẩn bằng `clip-path` tới khi focus, thêm kiểm |
| CMP `navigation.scaffold--default--light--wide`, `navigation.scaffold--compact--light--narrow` | Rail 220dp, toggle `menu-open`, Khám phá chọn, Tôi ở chân; hẹp: top bar, nội dung, thanh dưới ba đích. Ảnh hẹp 360×2400 lần đầu chưa tới specimen (danh mục dài hơn) → tăng 5200/9000 |
| Leptos `navigation.bottom-bar--long-labels--light--320--text200` | Chỉ chụp tới danh mục: ở 320px/200% specimen nằm sau giới hạn 6000px của capture. Chưa xem được specimen bằng ảnh; reflow không tràn ngang đã được kiểm bằng assertion |

## Tự review và phản biện

Không có reviewer độc lập trong phiên. Tự review và chạy thật tìm thấy, đã sửa:

- **Sự kiện `close` của `<dialog>` đến muộn** (Chrome xếp hàng sự kiện): sau đóng bằng bấm ngoài rồi
  mở lại ngay, `close` của lần trước giải phóng lớp phủ đang mở → nút Đóng không còn tác dụng. Tái hiện
  bằng chuột thật trong trình duyệt; handler chỉ giải phóng khi dialog thật sự đang đóng. Code cũ có
  cùng lỗ hổng (`open.set(false)`).
- **Đích sáng sai trên route lạ:** `starts_with("/ban-viet")` làm `/ban-vietnam` sáng Bàn viết →
  khớp theo segment như router.
- **Skip link không chuyển focus** (router nhận fragment như điều hướng) → scaffold tự focus main.
- **Skip link ẩn bằng clip vẫn rộng:** sau khi đổi sang `clip-path`, lượt `verify web` cuối báo tràn
  ngang ở 320px/chữ 200% (chữ `nowrap` vẫn tính vào vùng cuộn) → hộp 1px tới khi focus, chạy lại.

Phản biện đã xét: thư viện không import router/session/storage (kiểm ranh giới `quick`); chỉ một
guard, lớp phủ hỏi trước trong chính guard; Back đóng lớp phủ dùng cùng cơ chế đẩy URL trang về của
guard (thay mục forward như trước, không tạo vòng lặp); preference người dùng thắng ngữ cảnh; CSS
giải khung trước hydration nên SSR không nháy bề rộng. Giới hạn còn lại: catalog vẫn
`implemented-unverified`; icon shell đổi phong cách (nét → Material filled) cần người thiết kế xem;
native back/gesture, recreation và screen reader chưa chạy; adoption màn route thuộc #23 sau #24.
