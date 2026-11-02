# Bằng chứng UI System 03/06 — trạng thái, phản hồi, loading và motion Web/CMP

**Ngày:** 02/11/2026 (UTC+07) · **Issue:** [#21](https://github.com/loveoverflowcom/vink/issues/21) ·
**Nhánh:** `codex/ui-system-states-motion`, nền `develop@f3ca042` (đã gồm UIS-02 #30; snapshot của
issue là `5afb00b`, không reset). Đây là kết quả chạy trên máy dev, không phải nghiệm thu thiết kế,
thiết bị thật hay production. Contract: [states.md](../../packages/ui-system/shared/states.md);
hướng dẫn thiết kế: [state-and-feedback](../design/state-and-feedback.md).

## Môi trường

| Mục | Giá trị |
| --- | --- |
| Máy | macOS 26.5.2 (25F84), Apple Silicon arm64 |
| Rust / wasm-bindgen | 1.99.1 / 0.2.129 |
| Node / Chrome | v26.12.0 / Chrome 156.0.8078.79 headless (`--headless=new`), DPR 1 |
| JDK / Gradle / Kotlin / Compose | OpenJDK 17.0.19 / 8.14.3 / 2.2.20 / Compose Multiplatform 1.9.0, Material3 1.8.2 |
| Android / iOS | Android SDK (AGP 8.11.1) / Xcode 26.6, Kotlin/Native 2.2.20, iOS simulator arm64 |
| Nguồn | `tokens.json` 0.3.0 (SHA-256 bắt đầu `a99c2e195ecd1ede`), catalog 0.3.0, `fixtures/states.json` |

## HOW / WHY đã ghi trước khi đổi

- **HOW:** nguồn trạng thái ở app (`LoadError`/`ApiError::from_response` cho GET, `CreateState`
  cho create không chắc chắn, `EditorSession`/`SaveStatus` cho lưu) → adapter của caller chiếu vào
  các trục `Content` × `Phase`, `ActionPhase`, `Problem` → component chỉ trình bày và báo
  `on_recover(intent)`. Lỗi GET (đọc lại an toàn) khác lỗi create/save (không biết đã ghi chưa →
  `Uncertain` + `Reconcile`, không `Retry`).
- **WHY:** giữ dữ liệu và đường phục hồi đúng quan trọng hơn làm mọi lỗi giống nhau; một enum phẳng
  sẽ cho phép tổ hợp sai (lỗi quyền mà vẫn hiện dữ liệu, refresh xóa trang). Cantos `7649017` chỉ
  được học phương pháp ba purpose loading và token motion, không chép code.
- **Quyết định scoped trong PR:** tổ hợp vùng là bảng tường minh (112 vector) thay vì suy luận;
  `LoadingGate` thuần với đồng hồ tiêm (300/500/8000ms từ token); activity do host tiêm và cộng dồn
  như giảm chuyển động; 429 chỉ là fixture. Không có Quint model: các bất biến ở đây là bảng hữu
  hạn được kiểm hết bằng vectors, không phải race. Không chốt D/INV/S nào, không chọn auth.

## Kết quả

| Kiểm | Lệnh | Kết quả | Ghi chú |
| --- | --- | --- | --- |
| Nguồn shared hợp lệ, 11 file sinh ra khớp | `cargo xtask ui-tokens --check` | PASS | Thêm `state_vectors.rs`, `VinkStateVectors.kt`; ca `LoadingGate` phải khớp chính sách token |
| Lane quick | `cargo xtask verify quick` | PASS | fmt, clippy server/hydrate/preview `csr` (wasm32), test (`vink-ui` 36, `vink-ui-preview` 8, `xtask` 28), ranh giới UI system, link, skill |
| Lane web | `cargo xtask verify web` | PASS | `vink-web` phụ thuộc `vink-ui` (web-sys chỉ cho wasm32, `Cargo.lock` đổi); SSR, hydration, editor, lưu/xung đột, shell, restart; không assertion web mới vì chưa màn nào dùng component. PostgreSQL 17.11 scratch `127.0.0.1:55432` |
| Lane postgres | — | NOT_RUN | Không đổi query/transaction/persistence |
| Preview Leptos | `cargo xtask ui-preview leptos verify` | PASS | 37 catalog ID, 103 link kịch bản, 245 kiểm tra `ok` (gồm hàm `states`), tab ẩn thật đã chạy (không NOT_RUN); 355 ảnh + `contexts.json` ở `target/ui-preview/leptos/captures/`; chạy lại sau khi sửa `Dialog` |
| Preview CMP desktop + native | `VINK_UI_NATIVE_TARGETS=1 cargo xtask ui-preview cmp verify` | PASS | JVM: `VinkStateTest` 7, `VinkThemeTest` 10, `VinkScopeTest` 3, `VinkIdentityTest` 6; desktop: `StatePreviewTest` 11, `PreviewTest` 10, `CaptureTest` 1, `DrivenCaptureTest` 1 (216 ảnh ở `packages/ui-system/cmp/preview/build/captures/`); `:ui:iosSimulatorArm64Test` (`VinkStateTest` 7, `VinkThemeTest` 10, `VinkIdentityTest` 6) |
| Vectors chung | Rust `state::tests`, Kotlin `VinkStateTest` (JVM + iOS simulator) | PASS | 112 tổ hợp vùng, intent của 12 `ProblemKind`, 48 placement, 20 notice, 7 ca gate giống nhau ở Rust, JVM, Kotlin/Native |
| Lifecycle Android/iOS, giảm chuyển động của OS thật, TalkBack/VoiceOver | — | NOT_RUN | Chưa có host native (#24); CMP preview map cửa sổ thu nhỏ → không hoạt động |
| Cửa sổ CMP Desktop thật | `cargo xtask ui-preview cmp run` | NOT_RUN | Ảnh là `ImageComposeScene` off-screen bằng renderer thật |
| CI | — | NOT_RUN | `ui-preview` vẫn là lệnh cục bộ |

## Kịch bản đã khẳng định

Leptos (`preview-smoke.mjs`, hàm `states`, đồng hồ tiêm nên mọi mốc thời gian xác định; animation
đọc bằng Web Animations API, không suy từ pixel): skeleton trang trí và vùng `aria-busy`; tải chậm
là ghi chú info, không tone lỗi; no-results là panel empty có Xóa bộ lọc; low-content chỉ hiện
một mục thật; auth-required không có nút đăng nhập giả; lỗi chưa rõ không hứa thất bại chắc chắn;
mã chẩn đoán ổn định; lỗi một phần giữ nội dung; notice inline là `status` polite, chỉ banner
session-expired assertive; snackbar không action đóng sau 6000ms trên đồng hồ, có action thì chờ;
link tóm tắt lỗi focus field, field giữ chữ thô. Playground: loading → ready không chỉ báo ở
+0/+200ms, skeleton ở +300ms, tải xong trước 300ms không bao giờ hiện skeleton, ghi chú chậm sau
8000ms mà vẫn đang tải; error → Retry gọi intent đúng 1 lần, chữ còn; refresh giữ nội dung, focus và
vị trí cuộn; pending có `aria-busy` + MicroArc ở slot `status`, giữ bề rộng, lần kích hoạt thứ hai
không gửi; huỷ trung tính, ack muộn của vé đã huỷ không báo "đã lưu"; uncertain chặn gửi lại tới khi
đối chiếu, không có Retry, không tự replay; conflict giữ chữ và không bao giờ hiện saved; hết phiên
lúc viết mở alertdialog, Escape không đóng, không nút đăng nhập, ở lại giữ chữ. Dialog: phá hủy
focus lựa chọn an toàn trước, Tab ở trong modal, Escape đóng `EscapeOnly`, đóng trả focus về nút mở,
bấm ngoài không đóng `EscapeOnly` mà đóng sheet `EscapeAndOutside`. Motion: MicroArc/EdgeProgress
chạy mặc định, MicroArc không che nhãn; giảm → không vòng lặp, EdgeProgress hiện nhãn chữ; host
không hoạt động → không vòng lặp; cài đặt giảm của hệ thống dừng mọi vòng; determinate báo giá trị
thật, không lặp; tab ẩn thật (`visibilitychange`) → không vòng lặp, hiện lại → chạy lại.

CMP (`StatePreviewTest`): reducer `Submission` bỏ lần bắt đầu thứ hai và ack muộn; chỉ báo chờ đủ
độ trễ gate trên đồng hồ thủ công; retry gọi một lần và giữ input; conflict giữ chữ, không saved;
uncertain cần đối chiếu trước khi gửi lại có chủ ý; giảm/không hoạt động/ẩn không chạy vòng lặp;
dialog phá hủy focus lựa chọn an toàn và trả focus; hết phiên cần lựa chọn và giữ nháp; snackbar
không action đóng sau thời lượng, có action thì chờ; vùng sai tổ hợp hiện chẩn đoán thay vì nội
dung; mỗi kịch bản lỗi chỉ có hành động hợp lệ. `DrivenCaptureTest` thao tác 7 recipe rồi chụp.

## Ảnh đã mở và nhận xét

| Ảnh | Nhận xét → sửa |
| --- | --- |
| Leptos `state.loading--default--light--834`, `state.error--default--light--390` | Skeleton cùng hình thẻ thật (tiêu đề → byline → đoạn); panel lỗi: icon → tiêu đề → giải thích → Thử lại/Về Khám phá → "Mã: works.list.network". Không sửa |
| Leptos `state.notice--default--dark--390` | Bốn tone đọc rõ ở Tối, mỗi tone có icon + chữ. Không sửa |
| Leptos `state.playground--save-conflict--after-step--light--390`, `state.dialog--session-expired--light--390` | Conflict: badge "Xung đột phiên bản — chưa lưu" + cảnh báo giữ chữ + Đối chiếu/Tải bản sao; hết phiên: chữ nháp còn trong textarea. Không sửa |
| CMP `state.playground--save-conflict--driven--light--834`, `state.error--default--light--narrow`, `state.loading--default--dark--narrow` | Đối ứng Leptos (badge CMP rút gọn "Xung đột: chưa lưu"); skeleton tối không chói |
| CMP `state.dialog--session-expired--light--narrow` | Scrim + hộp thoại giữa màn, không nút đăng nhập; nhãn "Ở lại trang" xuống hai dòng ở 360dp — chấp nhận, ghi lại cho #24 |

## Tự review và phản biện

Không có reviewer độc lập trong phiên. Tự review tìm thấy và đã sửa:

- **Leptos context rò giữa kịch bản:** `provide_context` trong Leptos 0.8 gắn vào owner của render
  effect cha nên `MotionActivity` của kịch bản trước còn trỏ vào signal đã dispose → bọc bằng
  `Provider` có owner riêng (`MotionActivityScope`); smoke mở lại kịch bản cho sạch state.
- **Chrome đóng `<dialog>` sau Escape lặp lại** dù `cancel` đã bị chặn → dialog cần quyết định mở
  lại modal và đặt lại focus ban đầu; các policy khác báo `Escape`.
- **Lane quick tìm thấy:** hai `if` lồng nhau và một content slot `{()}` thừa (clippy `-D warnings`)
  → `Dialog` nhận content slot tùy chọn.

Phản biện đã xét: tổ hợp vùng sai bị từ chối thay vì render (vd. dữ liệu + `Forbidden`); không có
"saved" giả (chỉ `Confirmed` sau ack của vé hiện hành); không blind replay POST (`Uncertain` không
có `Retry`); không rò lỗi thô (`Problem` không có trường payload, mã ngoài mẫu bị bỏ). Giới hạn còn
lại: catalog vẫn `implemented-unverified`; lifecycle Android/iOS chưa có host; announce chỉ kiểm
thuộc tính live region, chưa nghe bằng screen reader; coordinator overlay toàn app thuộc #22;
adoption trên màn app thuộc #23 sau qualification #24.
