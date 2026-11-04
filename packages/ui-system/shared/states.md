# Contract trạng thái, phản hồi và chuyển động (UIS-03)

**Trạng thái:** UIS-03/06 ([#21](https://github.com/loveoverflowcom/vink/issues/21)) — NEW. Đã triển khai ở cả
hai renderer, đăng ký trong cả hai preview với trạng thái `implemented-unverified`. UIS-05 (#23)
đã dùng notice/panel/dialog ở các màn Web; qualification rộng hơn thuộc #24. Coordinator overlay toàn app (một dialog
mở tại một thời điểm, Back đóng lớp phủ trước, trả focus) là `OverlayLayer` của UIS-04 (#22), contract ở
[navigation.md](navigation.md).

UIS-05: Web `Dialog.title` là signal để bước xác nhận của `LeaveGuard` đổi tiêu đề mà không
remount; `focus_title` dùng cho preview dài. Trả focus dùng prevent-scroll. Sự kiện `close`
xếp hàng từ lần đóng trước không dismiss dialog đang mở lại: kiểm trạng thái DOM hiện tại
trước khi phát `on_dismiss`. App vẫn sở hữu chữ, scroll, quyết định lưu/rời và request identity.

Thư viện **trình bày** trạng thái có chủ nguồn; nó không phân loại lỗi nghiệp vụ, không fetch,
không lên lịch retry, không logout, không điều hướng, không xóa chữ và không quyết định quyền.
`EditorSession` (`apps/vink-web/src/desk_state.rs`) vẫn là authority của editor; `CreateState`
(`draft_list.rs`) vẫn quyết định create không chắc chắn; `ApiError::from_response` (`contract.rs`)
và `LoadError` (`api.rs`) vẫn phân loại phản hồi HTTP. Adapter của app chiếu các nguồn đó vào
kiểu dưới đây.

## Các trục, không một enum phẳng

| Trục | Kiểu | Owner của giá trị | Ghi chú |
| --- | --- | --- | --- |
| Nội dung đang có | `Content`: `Absent` · `Empty(EmptyKind)` · `Present { low, partial }` | Resource/list của caller | `low`: ít nội dung thật; `partial`: một phần không tải được |
| Pha yêu cầu | `Phase`: `Idle` · `Loading { slow }` · `Failed(ProblemKind)` · `Cancelled` | Resource/action của caller | `slow` đến từ `LoadingGate`, không phải timeout |
| Khả dụng hành động | `ActionState` (UIS-02): `Enabled` · `Disabled { reason }` · `Pending { status }` | Caller | Nút pending giữ nhãn + vùng chạm, slot `status` có `LoadingIndicator` |
| Kiểm hợp lệ | `FormSummary` + `error` của field (UIS-02) | Validator của caller | Giữ chữ thô; summary link tới field |
| Tương tác | `ActionPhase`: `Idle` · `Dirty` · `Pending` · `Confirmed` · `Failed(ProblemKind)` · `Cancelled` · `Uncertain` | Reducer có ticket của caller (vd. `EditorSession`) | `Confirmed` chỉ sau ack của ticket hiện hành |
| Thông báo | `NoticeSpec { placement, tone, announce, dismiss }` | Chính sách dưới đây | Vị trí theo tác động |

`EmptyKind`: `FirstUse` (chưa có gì, mời bắt đầu) · `NoResults` (bộ lọc/tìm không khớp, có
`ClearFilter`). End-of-list không phải empty: `ListEnd` hiển thị “Đã hết danh sách” dưới nội dung.

## Tổ hợp hợp lệ của vùng nội dung (`RegionState::layout`)

`RegionState { content, phase }` → `RegionLayout { body, progress, notice }` hoặc `Unlawful`.
`body`: `Skeleton` · `Panel(PanelKind)` · `Content`. `progress`: `EdgeProgress` trên vùng.
`notice`: `Slow` · `Refresh(kind)` · `Partial` · `LowContent`.

| content \ phase | `Idle` | `Loading { slow }` | `Failed(k)` | `Cancelled` |
| --- | --- | --- | --- | --- |
| `Absent` | ✗ chưa yêu cầu thì caller phải bắt đầu tải | `Skeleton` (+ `Slow`) | `Panel(Error(k))` | `Panel(Cancelled)` trung tính |
| `Empty(e)` | `Panel(Empty(e))` | `Panel(Empty(e))` + progress (+ `Slow`) | `Panel(Empty(e))` + `Refresh(k)` | `Panel(Empty(e))` |
| `Present { low, partial }` | `Content` (+ `Partial` > `LowContent`) | `Content` + progress (+ `Slow`) | `Content` + `Refresh(k)` | `Content` (+ `Partial`/`LowContent`) |

Ngoại lệ ✗ (`Unlawful`):

- `Failed(Validation | Conflict | Uncertain)` ở vùng tải: đó là lỗi của tác vụ ghi, trình bày cạnh
  field/hành động, không thay nội dung.
- `Empty`/`Present` + `Failed(Forbidden | NotFound | AuthRequired)`: dữ liệu đó không còn được phép
  hiển thị; caller bỏ dữ liệu rồi chiếu `Absent` + lỗi. `SessionExpired` có bằng chứng thì giữ dữ
  liệu (đã hiển thị cho chính người đó) và thêm banner.
- `Present { low: true }` chỉ khi có nội dung thật; không nhân bản spotlight/feed để lấp chỗ.

Refresh giữ nội dung, focus và vị trí cuộn: `Content` vẫn mount, chỉ thêm progress và
`aria-busy`; lỗi refresh là notice, không xóa trang. Huỷ không thành lỗi đỏ.

## Vấn đề (Problem) và hành động phục hồi

`Problem { kind, title, message, code, actions }` là **phép chiếu an toàn**: tiêu đề/thông điệp đã
bản địa hóa do caller cấp, `DiagnosticCode` ổn định (`[a-z0-9][a-z0-9._-]{0,63}`; chuỗi khác bị bỏ,
không hiển thị), danh sách `RecoveryAction { intent, label }`. Không có trường payload: stack trace,
token, phản hồi thô của backend không có chỗ để đi vào. Chữ render như text, không HTML.

| `ProblemKind` | Ví dụ nguồn (app) | Intent cho phép | Ghi chú copy |
| --- | --- | --- | --- |
| `Validation` | 422 `Invalid { field }` | — (sửa field) | Nói field nào, giữ chữ |
| `Conflict` | 409 `version_conflict` | `Reconcile`, `Export`, `Return` | Không tự ghi đè |
| `RateLimited` | 429 (**fixture-only**: API chưa trả 429) | `Retry`, `Return` | Retry do caller có chính sách |
| `Timeout` | client hết thời gian | `Retry`, `Export`, `Return` | Với ghi: dùng `Uncertain` |
| `Network` | `ApiError::Network` khi đọc | `Retry`, `Export`, `Return` | Không suy “offline” từ `navigator.onLine` |
| `Unavailable` | 503, `LoadError::Unavailable` | `Retry`, `Export`, `Return` | |
| `Uncertain` | create/save không biết đã ghi chưa (`CreateState::Uncertain`) | `Reconcile`, `Export`, `Return` | **Không** `Retry`: kiểm lại trước khi gửi lại có chủ ý |
| `Unknown` | `Unexpected { status }` khi đọc | `Retry`, `Export`, `Return` | Không hứa “đã thất bại chắc chắn” |
| `AuthRequired` | chưa đăng nhập, nội dung riêng | `Reauthenticate`, `Return` | Không login wall cho nội dung public; không có auth → caller **không** cấp `Reauthenticate` |
| `SessionExpired` | phiên hết hạn **có bằng chứng** (không suy từ 401 đơn lẻ) | `Reauthenticate`, `Export`, `Return` | Giữ chữ; không tự reload |
| `Forbidden` | 403 | `Return` | Không lộ tên/nội dung nháp |
| `NotFound` | 404, `LoadError::NotFound` | `Return` | Gồm “không còn khả dụng” |

`ClearFilter` chỉ thuộc `Empty(NoResults)`, `Start` (bắt đầu viết/khám phá) chỉ thuộc `Empty(FirstUse)`;
`PanelContent::empty` từ chối intent khác (`PanelError`), `PanelContent::cancelled` chỉ có `Retry`. `Problem::with_action` từ chối intent không cho phép
(`IntentNotAllowed`) thay vì hiển thị nút sai. Intent là callback của caller: thư viện chỉ báo
`on_recover(intent)` một lần mỗi kích hoạt.

## Chính sách vị trí và thông báo

| Placement | Khi | Announce | Dismiss |
| --- | --- | --- | --- |
| `Field` | `Validation` của một field | `Off` (đã liên kết `aria-describedby`; summary nhận focus khi gửi) | Khi caller sửa |
| `Inline` | lỗi/ghi chú cạnh vùng có dữ liệu, `Conflict`, `Uncertain`, slow | `Polite` | `Manual` hoặc khi trạng thái đổi |
| `Region` | vùng không có dữ liệu: lỗi, truy cập, empty | `Polite` | Khi trạng thái đổi |
| `Banner` | ảnh hưởng cả trang: `SessionExpired` khi đọc | `Assertive` chỉ cho `SessionExpired`; còn lại `Polite` | `Manual` |
| `Snackbar` | xác nhận ngắn không cần quyết định (Đã lưu, Đã dừng) | `Polite` | `Auto` chỉ khi không có action; có action thì `Manual` |
| `Dialog` | cần quyết định: xác nhận phá hủy, `SessionExpired` lúc đang viết | `Off` (focus vào dialog đọc tiêu đề) | Theo `DismissPolicy` |

`placement_for(kind, has_data, writing)` cài bảng trên; `NoticeSpec::new` từ chối tổ hợp sai:
`Snackbar` không mang tone `Error`/`Warning`, `Auto` chỉ cho `Snackbar`, `Assertive` không dùng cho
`Snackbar`/`Field`/`Dialog`. Blocker (lỗi editor, conflict, session-expired, uncertain) không bao giờ
chỉ nằm trong snackbar tự biến mất. Live region chỉ đổi chữ khi thông điệp đổi (không announce mỗi
frame/dấu chấm tải, không lặp sau recomposition). Lỗi/retry không tự chuyển focus khi người dùng còn
đang nhập, trừ: gửi form lỗi → focus summary; mở dialog → focus vào dialog.

## Chuyển động và tải

| Purpose | Component | Khi | Giảm chuyển động | Không hoạt động |
| --- | --- | --- | --- | --- |
| SurfacePulse | `Skeleton` | Nội dung lần đầu (`Absent` + `Loading`) | Khối tĩnh | Dừng pulse |
| MicroArc | `LoadingIndicator` | Hành động pending (slot `status` của Button), giữ nhãn và vùng chạm | Cung tĩnh + nhãn | Dừng quay |
| EdgeProgress | `EdgeProgress` | Refresh/tìm kiếm đang bay; `Determinate(v)` chỉ khi caller có tiến độ thật | Thanh tĩnh + nhãn | Dừng trượt |

- Vòng lặp (`duration.pulse`, `duration.loop`) dừng bằng `loop.iterations: 0`, **không** chạy với
  duration 0. Chuyển tiếp hữu hạn (`duration.short/medium`) về 0ms khi giảm chuyển động.
- `LoadingGate` (thuần, đồng hồ do adapter tiêm): chỉ hiện chỉ báo sau `duration.loading-delay`
  (300ms), đã hiện thì giữ tối thiểu `duration.loading-min-visible` (500ms) để không nháy; báo `slow`
  sau `duration.slow-notice` (8000ms) — đó là gợi ý UX “vẫn đang tải”, không tuyên bố timeout/lỗi.
  View không tự đặt timer mạng; adapter (Leptos `use_loading_gate`, CMP `rememberLoadingGate`) hỏi
  `next_change` rồi hẹn một lần.
- **Activity** do host tiêm và cộng dồn như giảm chuyển động (vùng ngoài không hoạt động thì bên
  trong không chạy lại): Leptos `MotionActivity { active: Signal<bool> }` (`provide_motion_activity`),
  adapter `document_visibility()` (Page Visibility API); CMP `VinkMotionActivity(active) { … }`
  (`LocalMotionActive`), host map lifecycle (desktop preview: cửa sổ thu nhỏ/ẩn; Android/iOS chưa có
  host nên NOT_IMPLEMENTED). Không hoạt động (tab ẩn, nền, offscreen) → không
  vòng lặp nào chạy. Unmount/dispose → animation mất cùng node. Component con không tự chạy timer:
  Web dùng CSS animation, CMP `rememberInfiniteTransition` chỉ khi `loopsRun && active`.
- Shared role định nghĩa ý nghĩa; easing/hình dạng theo nền tảng, không ép giống pixel.

## LayoutSpec (anatomy, ID vùng ổn định)

```text
StatePanel #state-panel        — Symbol(icon, ẩn với AT) → Heading → Explanation → Primary recovery
                                 → Secondary actions → Code ("Mã: …", chữ nhỏ); căn trái, measure đọc.
                                 Primary = typed action đầu (Filled); không có typed action thì Web
                                 tô Primary cho link Standalone đứng đầu slot (GET chạy trước hydrate),
                                 CMP caller truyền VinkButton Filled
InlineNotice #inline-notice    — Icon + Title? + Message (không cắt, xuống dòng) + Action?; tone bằng icon + chữ
Banner #banner                 — như InlineNotice, rộng hết vùng chứa, đầu trang/vùng; không che control
Skeleton #skeleton             — cùng geometry/thứ tự đọc nội dung thật (dòng tiêu đề, meta, đoạn);
                                 trang trí aria-hidden; nhãn "Đang tải …" nằm ở ContentRegion
ContentRegion #region          — EdgeProgress? → Notice? → Body (Skeleton | StatePanel | Content giữ mount)
                                 aria-busy khi tải; một nhãn trạng thái cho cả vùng
LoadingIndicator #micro-arc    — vòng 20/24 (Small/Medium); trong Button: aria-hidden, nhãn ở note
EdgeProgress #edge-progress    — thanh 4px ở mép trên vùng; role=progressbar, valuenow chỉ khi determinate
StatusBadge #status            — Icon/arc + Label; tone từ ActionPhase, không chỉ bằng màu
ListEnd #list-end              — Divider + "Đã hết danh sách"; không phải CTA, không announce
Snackbar #toast                — một dòng ngắn + tối đa một action + nút đóng; đáy vùng, không che focus
Dialog #dialog                 — Title → Context → Content slot → Actions (an toàn trước khi phá hủy);
                                 Sheet: cùng anatomy, neo đáy ở màn hẹp
FormSummary (UIS-02)           — dùng làm FieldErrorSummary; không tạo bản thứ hai
```

Mỗi state: hẹp (<760px) — panel/notice xếp dọc, action xuống dòng, sheet neo đáy; Sáng/Tối — mọi
màu là role (`error-container`, `warning-container`, `info-container`, `success-container`,
`inverse-surface` cho snackbar); chữ 200% — không cắt, control cao thêm; focus — vòng focus UIS-01,
dialog trap + trả focus. Quyền phục hồi luôn từ caller.

## Props, sự kiện và khác biệt nền tảng

Tên dùng chung; Leptos không tiền tố (`StatePanel`), CMP tiền tố `Vink` (`VinkStatePanel`).

| Component | Props chính | Sự kiện | Không sở hữu / khác biệt nền tảng |
| --- | --- | --- | --- |
| `ContentRegion` | `state: RegionState`, `label` (vd. "Danh sách tác phẩm"), `panel: PanelContent?`, `notices: RegionNotices` (copy), `skeleton: SkeletonShape`, `content` slot | `on_recover(intent)` | Fetch, cache, scroll restore khi đổi route |
| `StatePanel` | `content: PanelContent` (`from_problem`, `empty`, `cancelled`), `heading: HeadingLevel`; optional caller action slot (`children` Web / `actions` CMP) | `on_recover(intent)` ×1; slot giữ callback/link của caller | Phân loại lỗi |
| `InlineNotice` / `Banner` | `tone: Tone`, `title?`, `message`, `action: RecoveryAction?`, `announce: Announce`; dismiss tùy chọn | `on_recover`, `on_dismiss` | Tự ẩn |
| `Skeleton` | `shape: SkeletonShape` (`Text { lines }`, `Card`, `Byline`) | — | Nhãn (ở `ContentRegion`) |
| `LoadingIndicator` | `label?` (rỗng = trang trí), `size: IndicatorSize` | — | Tiến độ giả |
| `EdgeProgress` | `progress: Progress` (`Indeterminate` / `Determinate(0..=1)`), `label` | — | Ước lượng phần trăm |
| `StatusBadge` | `phase: ActionPhase`, `label` | — | Tự đổi phase |
| `ListEnd` | `label` (mặc định "Đã hết danh sách") | — | Tải thêm |
| `Snackbar` | `message: Option<SnackbarMessage { text, action? }>` (controlled) | `on_action`, `on_dismiss` | Hẹn giờ: adapter `use_snackbar_timeout` / `rememberSnackbarTimeout` |
| `Dialog` | `open`, `title`, `context?`, `role: DialogRole` (Dialog/AlertDialog), `kind: DialogKind` (Dialog/Sheet), `dismiss: DismissPolicy` (Explicit/EscapeOnly/EscapeAndOutside), content + `actions` slot; Web `id` | `on_dismiss(DismissReason)` | Mở/đóng (controlled), coordinator (UIS-04). Web: `<dialog>` + `showModal()` (inert ngoài, Escape → `cancel`), lưu/trả focus; CMP: `Dialog` window + `DialogProperties`, `FocusRequester` |

Nút phá hủy trong `AlertDialog`: focus ban đầu vào lựa chọn an toàn (`initial_focus` = an toàn, mặc
định phần tử đầu của actions). `DismissPolicy::Explicit` chặn Escape và bấm ngoài (dialog cần
quyết định); `EscapeOnly` mặc định; `EscapeAndOutside` cho thông tin.

## Kịch bản preview (playground)

Cả hai preview có `state.playground` với đồng hồ cố định (bắt đầu ở `fixture::CLOCK`, chỉ tiến khi
bấm), nút bước, input giữ chữ và nhật ký sự kiện. Reducer có ticket của preview kiểm double-click và
ack muộn (unit test ở đúng preview), không thay `EditorSession`.

| Recipe | Chuỗi | Phải giữ |
| --- | --- | --- |
| `loading-ready` | Absent+Loading → (300ms) skeleton → Present | Không nháy chỉ báo cho tải nhanh |
| `loading-empty` | → Empty(FirstUse) | Không nhầm lỗi với empty |
| `loading-error-retry` | → Failed(Network) → Retry → Present | Retry là intent của caller |
| `refresh-partial` | Present → refresh → Partial | Nội dung, focus, cuộn còn nguyên |
| `pending-cancel` | Pending → Cancelled | Không lỗi đỏ, nhãn nút không đổi |
| `submit-uncertain` | Pending → Uncertain → Reconcile → gửi lại có chủ ý | Không tự replay POST, chữ còn |
| `save-conflict` | Dirty → Pending → Conflict | Chữ còn, không báo saved |
| `session-expired` | Đang viết → SessionExpired dialog | Chữ còn, không nút đăng nhập giả, không reload |

Vectors chung (lawful combinations, intent, placement, `LoadingGate`) ở
[fixtures/states.json](fixtures/states.json), sinh vào hai renderer và test ở cả hai.

## Ghi chú cho caller (adoption #23)

- Chiếu `LoadError::{NotFound, Unavailable}` → `Absent + Failed(NotFound | Unavailable)`; `ApiError`
  của GET → `Unavailable`/`Network`/`Unknown`; 401 chỉ thành `SessionExpired` khi authority xác nhận
  (hiện không có auth: dùng `AuthRequired` cho vùng riêng, không có nút đăng nhập).
- `CreateState::Uncertain` → `ActionPhase::Uncertain` + `Problem(Uncertain)` với `Reconcile`; chỉ
  `Reconciled` mới cho gửi lại có chủ ý. `SaveStatus` của `EditorSession` → `ActionPhase`
  (`Saved` → `Confirmed`, `Conflict` → `Failed(Conflict)`).
- Pilot nhỏ trong UIS-03: `Button` pending của thư viện hiện `LoadingIndicator` trong slot `status`.
  Không màn nào của app đổi sang component mới trong PR này.
