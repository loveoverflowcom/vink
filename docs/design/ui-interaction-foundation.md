# UI interaction foundation — CMP

Ngày 12/11/2026 (UTC+07), base `origin/develop@7459b91b21ed37e9142f6ffb8b91bd62733884a5`.
PR skills #172 đã merge ở base này. Phạm vi là component xuất ra của UI System và diagnostic
hosts; không phải app native, không thay nghiệp vụ, token value hoặc quyết định sản phẩm.

## Findings và phép kiểm

| ID | Phân loại / nguồn | Ảnh hưởng và thay đổi | Kiểm chứng |
| --- | --- | --- | --- |
| I-01 | DEFECT: `VinkFields.kt`, clear chỉ phát empty | Kích hoạt clear bằng bàn phím bỏ focus khỏi input. Clear phát đúng một empty rồi focus input thật | `InteractionTest.searchClearReturnsFocusToTheEditableTargetAndDispatchesOnce`: FAIL trước sửa, PASS sau sửa |
| I-02 | IMPLEMENTATION_GAP: `TextControlField` cố định Next/Default/Search | Caller không cấu hình được field cuối form. Thêm IME action/callback và modifier trên input thật | `InteractionLifecycleTest.imeDoneKeepsCompositionAndDispatchesOnlyCommittedEditableInput`; native recipe Next → Search, textarea Done |
| I-03 | IMPLEMENTATION_GAP: `VinkDialog` luôn request returnFocus sau đóng | Navigate có thể cướp focus của vùng đích. Dùng `CloseReason.focusReturn`, fallback main khi trigger không attach | `navigateCloseDoesNotRestoreOldTriggerAndDismissDoes`; kiểm focus đích mới, không nhầm focus của hai window |
| I-04 | IMPLEMENTATION_GAP: finite motion chỉ có duration, consumer tự chọn curve | Thêm mapping effects/spatial nhỏ trong `VinkMotion`, consumer chip và overlay thật | Clock-driven retarget, reduction giữa chừng, ancestor inactive, disposal |
| I-05 | IMPLEMENTATION_GAP: Dialog tắt platform Back, chỉ bắt `Key.Back` | Android Back dispatcher cần adapter tại dialog window. Dùng Activity BackHandler ở đúng window; explicit vẫn chặn | Instrumentation `dialogWindowBackDismissesOnceAndExplicitPolicyRefuses` |
| I-06 | IMPLEMENTATION_GAP: callback window cũ có thể giữ callback mới | Giới hạn callback dismiss theo lifetime của opening; caller từ chối dismiss vẫn giữ panel | `rejectedOutsideDismissKeepsSheetAndDisposedCallbackCannotCloseReopenedSheet` |
| I-07 | IMPLEMENTATION_GAP: panel sheet có empty clickable, thiếu vùng cuộn khi IME giảm chiều cao | Dùng Material Surface chặn touch xuyên panel mà không thêm Button role; scroll tới action, clip theo shape | 8 ảnh narrow/large text, Android screenshot với IME và iOS XCTest cuộn → đóng → giữ chữ |

`Modifier.focusRequester` trên wrapper **không được kết luận là lỗi**: test đã đi tới input
thật ở base. `inputModifier` là seam tường minh cho caller, giữ modifier wrapper cũ.
Không có đo release performance; không tuyên bố sửa lag hay đạt 60/120fps.

## Regional LayoutSpec và interaction contract

Đã trình bày trong chat trước sửa UI; giữ Lavender, tonal hierarchy, borderless-first.

```text
Field #field
  Label → Input #input [search icon / clear] → Help/Error
Chip #chip
  Check slot cố định → Label; vùng chạm không scale, màu đổi theo state đã nhận
Overlay #overlay
  Scrim → Panel #panel [title → context → content → actions]
  Dialog giữa khung; Sheet neo đáy, nội dung cuộn, không có drag handle
```

- **Owner:** caller giữ `TextFieldValue`, selected và open; `OverlayLayer` của host giữ modal
  đang hoạt động. Animation/lifetime chỉ là trạng thái trình bày, không là bản sao authority.
- **Trigger → commit/cancel:** field phát raw value; Next/Done/Search gọi caller chỉ khi editable
  và composition đã kết thúc. Không callback thì dùng hành vi IME mặc định. Clear là ý định xóa
  tường minh (kết thúc composition), không submit. Nhấn rồi kéo ra ngoài không kích hoạt button;
  pending không nhận activation tiếp. Không có callback nghiệp vụ sau animation.
- **Focus:** `inputModifier` gắn vào TextField; không request focus mỗi recomposition. Mở overlay
  focus safe target một lần khi dialog window có focus. Đóng bình thường chờ window nền có focus,
  thử trigger còn attach rồi main; Navigate để caller
  của vùng đích quyết focus. Keyboard focus không đại diện accessibility focus. Caller cần bỏ
  `returnFocus` khi trigger bị ẩn nhưng vẫn attach. Tham số mặc định giữ callsite cũ.
- **Interruption:** đổi theme/state không remount input. Effects retarget từ giá trị đang vẽ.
  Đóng overlay tháo ngay, không exit timer. Callback của window đã dispose không được đóng
  opening mới; nếu caller không đóng, nội dung/focus không bị component tự tháo.
- **Motion:** effects = duration.short + ease.emphasized; spatial = duration.medium + ease.spatial,
  đều từ generated tokens. Chip đổi màu; dialog fade, sheet thêm dịch chuyển nhỏ bằng Space4.
  Reduced/inactive snap tới đích; loop gate cũ giữ nguyên, không infinite animation duration 0.
- **Insets/cuộn:** host quản lý màn nền; sheet window quản lý safeDrawing/IME của lớp phủ riêng
  bằng consumption của Compose, không cộng lại inset đã consume. Panel có scroll state ổn định
  để tới field/action khi chiều cao giảm. Không delay chờ keyboard, không global keyboard manager.
- **Narrow/large text:** minimum hit target giữ 48dp, không cố định chiều cao field. Kiểm
  360×800dp, light/dark, font 100/200%; title/help và multiline giữ dấu, đoạn và khổ thơ.
  Action RowScope hiện hữu giữ nguyên; caller tránh hàng nhiều action dài không xuống dòng.
- **Back/gesture:** adapter Android cài trong Dialog, không thay router hoặc handler ở host.
  Escape Desktop giữ DismissPolicy. Có nút đóng trong recipe. Sheet cuộn dọc, không kéo để đóng,
  không thêm swipe/outside-dismiss mặc định. Predictive Back và iOS swipe navigation chưa được
  triển khai/qualification trong lát này.

## API, parity và dependency

API thêm optional ở CMP: `inputModifier` trên ba text controls; `imeAction`/`onImeAction` trên
TextField/TextArea; `closeReason`/`mainFocus` trên Dialog. `onSubmit` Search giữ nguyên.
Không round-trip TextFieldValue qua String; không normalize/trim trong renderer.
`VinkMotion.shortMs/mediumMs/loopMs/loopsRun` giữ API; thêm `effects()`/`spatial()`.

Catalog 0.8.0 thêm `composition.interaction/{default,sheet,explicit}` độc lập trong REGISTERED.
Admission map 0.4.0 gắn các component bị sửa với recipe thật. Tokens vẫn 0.5.0.
Recipe native dùng exported components; không có renderer giả khác production.

Leptos vẫn có field DOM controlled, composition guard, clear-focus, CSS effects/spatial và
focus policy của host. **UNSUPPORTED có chủ đích** cho recipe IME/FocusRequester/Android Back:
không mô phỏng native APIs bằng một form Web. Leptos Dialog chưa có optional closeReason như
CMP; route/overlay focus của Web tiếp tục thuộc NavigationGuard/shell. Không tuyên bố sửa
Web Dialog hoặc parity OS IME. Leptos preview vẫn kiểm mọi entry khác và hiển thị lý do tại entry
native này. Đây khác với native test chưa chạy (`NOT_RUN`).

Đã đối chiếu [Android Material3](https://developer.android.com/develop/ui/compose/designsystems/material3)
và [JetBrains versioning](https://kotlinlang.org/docs/multiplatform/compose-compatibility-and-versioning.html)
ngày 12/11/2026, cùng JAR `org.jetbrains.compose.material3:material3-desktop:1.8.2` thực tế:
không có MotionScheme; có ModalBottomSheet. Không nâng Kotlin 2.2.20, CMP 1.9.0, Material3
1.8.2 hay Gradle 8.14.3. Không dùng ModalBottomSheet vì default drag/hide lifecycle khác contract
controlled hiện hữu; không tự chốt drag-dismiss. Không thêm thư viện animation/gesture ngoài.

Android `:ui` dùng `androidx.activity:activity-compose:1.10.1` (cùng pin đã có ở diagnostic host,
Apache-2.0), chỉ androidMain để nhận Back ở Dialog window. Không đưa vào common/iOS/Desktop;
không đổi APK runtime version đã resolve. Consumer: DialogBackHandler; phép kiểm: instrumentation
Back/Explicit. Nếu thay adapter, phải giữ gate và test này; không fallback sang Key.Back để
nhận là platform Back. Không đo binary size delta.

## Evidence

Kết quả thực tế và giới hạn được ghi trong
[qualification registry](../development/ui-system-qualification-evidence.md#interaction-foundation-12112026).
