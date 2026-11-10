# Contract component nền (UIS-02)

**Trạng thái:** UIS-02/06 ([#20](https://github.com/loveoverflowcom/vink/issues/20)) — đã triển khai ở
cả hai renderer, đăng ký trong cả hai preview với trạng thái `implemented-unverified`. UIS-05 (#23)
đã đưa các component vào năm màn Web và composition CMP mẫu; qualification rộng hơn thuộc #24. Đây là contract ngữ
nghĩa dùng chung: Leptos (`vink-ui`) và CMP (`:ui`) giữ **cùng tên, props, sự kiện, owner và trạng
thái không hỗ trợ**, không cần cùng DOM/cây Compose.

UIS-05 bổ sung seam cho consumer thật, không đổi owner dữ liệu: Web `Link.href` nhận
`Signal<String>` (chuỗi tĩnh vẫn dùng được), URL không hợp lệ thành anchor không có href;
`TextField` có `name`, `max_length`, `TextKind::Search`, `input_ref` và `on_focus` để form GET
SSR và editor dùng cùng primitive mà giữ focus/giới hạn 200. `Button.button_ref` là điểm trả
focus của host; vẫn một target và chặn activation khi disabled/pending.

## Nguyên tắc chung

- **Controlled:** giá trị đến từ caller (`Signal`/tham số), component chỉ phát intent qua callback.
  Không HTTP, storage, route, session, profile lookup hay validation nghiệp vụ trong primitive.
- **Một target tương tác, một lần dispatch:** mỗi component có đúng một phần tử/node nhận tương tác;
  mỗi kích hoạt gọi callback đúng một lần; `Disabled`/`Pending` gọi **không** lần nào.
- **Typed variant + slot:** biến thể là enum (`ButtonVariant`, `AvatarSize`…), không nhận class/style
  tự do. Chỉ có biến thể đã có consumer/kịch bản; không có `UniversalComponent`.
- **ID do caller cấp** cho field và action có ghi chú (`id`); ID con suy ra cố định (`{id}-label`,
  `{id}-help`, `{id}-error`, `{id}-note`) nên SSR/hydration không lệch. Trùng ID là lỗi của caller;
  preview kiểm không có ID trùng.
- **Không chỉ bằng màu:** bắt buộc có chữ "(bắt buộc)", lỗi có icon + chữ, chọn có dấu/trạng thái.
- **Kích thước chạm:** Web control thường 48px (`density.control-height`), mật độ gọn 44px (= sàn
  `size.hit-target` đã công bố). CMP luôn ≥48dp (Material `minimumInteractiveComponentSize`).
  Avatar/icon trang trí nhỏ hơn target được; không phóng mọi glyph thành 48.
- **CSS có scope:** class `vk-ui-*`, không selector tag toàn cục, không chạm CodeMirror hay `vk-*` của app.

## LayoutSpec (anatomy, ID vùng ổn định)

```text
Text #text                     — role (TypeRole) độc lập với thẻ; tone (TextTone)
Heading #heading               — level h1–h6 độc lập với role hiển thị
Surface #surface               — tone nền + radius + padding (Space); slot #content
Container #container           — measure: Reading (--vk-measure) | Wide (--vk-measure-wide) | Full
Stack #stack                   — Column|Row, gap (Space), align, wrap
Grid #grid                     — cột tự động ≥ GridMin (sm|md), gap; tự xuống một cột ở màn hẹp
Divider #divider               — <hr> có nghĩa | trang trí (aria-hidden)

Button #action
  Control #control             — <button type=button|submit> / Material Button; một target
    Slot #leading              — Icon trang trí (tùy chọn)
    Label #label               — chữ luôn đọc được, không đổi khi pending
    Slot #status               — chỗ dành cho chỉ báo của UIS-03 (rỗng trong UIS-02)
  Note #note                   — lý do disabled / chữ trạng thái pending (id `{id}-note`)
IconButton #action             — Icon + label bắt buộc (accessible name), pressed tùy chọn
Link #link                     — <a href> thật; NewTab thêm icon + "(mở tab mới)"
FilterChip #chip               — toggle (aria-pressed / selected), dấu ✓ khi chọn

Field #field
  Label #label                 — Web: <label for> phía trên; CMP: slot `label` của Material (nổi lên,
                                 không biến mất như placeholder); "(bắt buộc)" bằng chữ
  Control #control             — input | textarea | search | select (native trước)
    Slot #leading / #trailing  — icon tìm, nút xóa (SearchField)
  Help #help                   — gợi ý (id `{id}-help`)
  Error #error                 — icon + chữ khi error khác rỗng (id `{id}-error`)
Choice (Checkbox | Switch) #choice — <label> bọc input + chữ: một target, một handler
RadioGroup #group              — <fieldset><legend> + các Radio; help/error như Field
FormSummary #summary           — tóm tắt lỗi do caller cấp, link tới `#{field-id}`

Icon #icon                     — SVG/vector từ registry; trang trí (ẩn) | có nhãn
Avatar #avatar                 — Image (khi Ready và tải được) trên Initials | Neutral
AvatarGroup #group             — tối đa N avatar + "+K" chỉ khi biết tổng; một nhãn nhóm
AuthorByline #byline           — Avatar (ẩn với AT) · Name (đậm, link chỉ khi href đã xác minh) · Meta
ImageFrame #frame              — tỉ lệ cố định; Image | Fallback trung tính (không spinner)
```

Màn hẹp (<760px, cùng ngưỡng shell): Stack Row được `wrap`, Grid về một cột, Avatar dùng kích thước
`-narrow`, byline xuống dòng nhưng tên vẫn trước metadata. Chữ 200%: rem/sp tăng theo, không cắt;
control giữ chiều cao tối thiểu nhưng được cao thêm. Sáng/Tối: mọi màu là role, resolve theo scope.

## Quiet Lavender R1 — Shared Controls (PR-01)

`TextField` / `VinkTextField` thêm `variant: FieldVariant`:
`ExternalLabel` (mặc định) và `FloatingTonal`. `FloatingTonal` là short field một dòng;
PR-02 dùng `variant=FieldVariant::FloatingTonal` cho `draft-title` và giữ nguyên `id`, `name`,
`max_length`, `input_ref`, `on_focus`, callback và value. `TextArea` Web thêm `variant` opt-in cho recipe multiline Nhận xét R3.1 (#134), tái dùng native field/composition owner; mặc định và CMP giữ nhãn ngoài;
`SearchField` mặc định giữ anatomy nhãn ngoài. Search R3 (#90) bổ sung trên Web
`SearchVariant::Bar`, optional `name`, `clear_href`, `clear_id`, `on_clear`, `input_ref`.
Bar dùng cùng native `text_control`, một kính lúp `type=submit`, nhãn thật intrinsic và
clear trong một tonal shell; caller sở hữu form/URL/query. `clear_href` là fallback link thật
khi không JS, hydrated clear phát raw empty + callback rồi focus input; modifier click giữ
nghĩa link. Không có name thì không serialize query riêng vào form. Enter đang composition
luôn chặn native submit (kể cả Safari keyCode229). CMP giữ API/hình học hiện có;
Bar là Web-only, chưa tuyên bố parity/native qualification. [LayoutSpec scoped](../../../docs/design/search-r3.md).

Surface R5.1 (#90) thay phần label của Web Bar: native label visually-hidden, placeholder
và query cùng một dòng; chỉ kính lúp/input/conditional clear trong shell. Scope ngắn ở
help ngoài shell. ExternalLabel mặc định và các field khác không đổi. WorkCard/DraftRow
dùng tonal surface và gap thay rules; ListEnd bỏ pseudo rules. [Delta hiện hành](../../../docs/design/search-r5.md).

Web support API chung: `PlainTooltip { id, text, children }` chứa text thuần; trigger
giữ tên riêng và tham chiếu ID bằng `aria-describedby`. Hover/focus duy trì (focus tự cuộn được đặt lại vị trí; Escape không tự mở lại), tooltip chỉ nhận pointer sau khi đã đặt đúng vị trí để không chặn click action. Pointer
có thể đi vào tooltip, Escape đóng tooltip mà không chuyển focus hoặc đóng panel cha.
`TooltipIconButton { id, icon, label, tooltip?, state?, pressed?, on_press? }` ghép
IconButton cùng tooltip `{id}-tooltip`; disabled/pending vẫn chặn action và giữ note.
`SupportingDetails { id, summary, compact?, children }` dùng native details/summary
trong flow; compact chỉ ẩn chữ summary bằng visually-hidden. Native toggle, keyboard,
focus và no-JS giữ nguyên; không áp Escape/focus-return của overlay. Essential warning,
blocker và unsaved state không được đưa vào detail tùy chọn. Rail giữ exception tooltip
ngoài scroll containment. Leptos preview và Desk refresh dùng owner này; CMP chưa có
support API/parity, không có native qualification từ Web checks.

Cây floating Web: `Field → Control [Input + Label] → Help/Error`. Input và label dùng hai hàng
intrinsic để nhãn dài tiếng Việt reflow ở 200%; empty blur đặt nhãn giữa shell, focus/filled/
restored/autofill đặt nhãn trên hàng chữ. CSS đọc giá trị DOM (`:placeholder-shown`, `:autofill`),
không remount hoặc thay input khi label đổi. Label thật có `for`, helper/error có
`aria-describedby`. CMP giữ Material3 `TextField` và `TextFieldValue`; `FloatingTonal` dùng slot
label native; nhãn ngoài là text hiện hữu, được đưa vào semantics text của cùng field, không
thay label bằng contentDescription. Field disabled thiếu help được giải thích “Tính năng chưa
khả dụng.”; caller có thể cung cấp lý do cụ thể hơn.

Idle tonal không outline trang trí. Floating shell Web sở hữu một outline inset; field nhãn ngoài,
search, Select và textarea để input sở hữu. Focus 2px, error 1px, error+focus 2px màu error;
không shadow vẽ viền. Nút trailing chỉ vẽ focus của chính nó. Actions/chips dùng một focus 2px
flush; Outlined vẫn có API riêng. CMP tắt cả bốn indicator Material, một border trên native
TextField là perimeter duy nhất. Chỉ focus của text control kích hoạt perimeter, không dùng
focus-within của parent cho trailing action. Forced-colors được thêm stroke hệ thống và giữ
focus; không có chuyển động nhãn custom.

Chip idle dùng SurfaceContainer, selected dùng SecondaryContainer + check/pressed; slot check
được giữ để toggle không dịch hàng. Filled/Tonal/Text/Outlined/Danger và action guards giữ nguyên;
status slot của cả Text button được chừa trước pending. Tonal dành cho hành động phụ, Filled cho
hành động chính; không đổi default API của Button.

Quyết định CHOICE-01: Material3 TextField + indicator transparent + border có điều kiện, giữ
native IME/caret/label và không xây BasicTextField mới. CHOICE-02: floating shell / input nhãn ngoài
sở hữu perimeter theo đúng painted target; không dùng parent focus-within. CHOICE-03: giữ
RadioGroup generic native; inventory chỉ có appearance cần icon/label/check, recipe app thuộc PR-03.
Không thay token vì role màu/spacing/radius hiện có đủ; catalog thêm scenario `external-label`
và sinh lại bằng `cargo xtask ui-tokens`.

## ToggleAction — hành động bật/tắt đã xác nhận (LVE #73)

**Trạng thái:** token 0.5.0, catalog 0.6.0 (`action.toggle-action`), vectors
`toggle-contract-v1` ở [fixtures/toggle.json](fixtures/toggle.json); đã triển khai ở Leptos
(`vink_ui::toggle`) và CMP (`VinkToggle.kt`), `implemented-unverified`. Consumer đầu tiên là
`EndorsementAction` của [patterns](../../vink-patterns/README.md); màn app đầu tiên là reader Web
(tác phẩm + từng bình luận, qua adapter LVE của `vink-web`).
Primitive chung cho mọi tín hiệu bật/tắt có owner xác nhận (ghi nhận, sau này lưu/reaction nếu
được chấp nhận), không biết endpoint, actor, `command_key` hay `expected_order`.

```text
ToggleAction #toggle
  Control #control             — một <button>/node clickable; tên truy cập theo giá trị đã xác nhận
    Slot #glyph                — icon registry tùy chọn, giữ nguyên ở mọi trạng thái (EndorsementAction dùng Pen Flight R2 A)
    Label #label               — chữ hiển thị cố định ở mọi trạng thái
    Count #count               — số (nhóm “1.234”) | “–” khi không có số liệu | không render khi Hidden
    Slot #mark                 — ô 20px cuối nút giữ sẵn như status slot của Button: MicroArc khi
                                 pending · dấu ✓ khi owner xác nhận Bật; nhãn không dịch
  Feedback #feedback           — live region polite, luôn mount, không chiếm chỗ khi rỗng
    Note #note                 — đang gửi / chưa xác nhận được / chưa rõ / thất bại / lý do chặn (`{id}-note`)
    Recover #recover           — Text button “Kiểm tra lại” (`RecoveryIntent::Reconcile`) khi cần
```

Ba trục độc lập, adapter của caller chiếu từ nguồn của nó:

| Trục | Kiểu | Ghi chú |
| --- | --- | --- |
| Giá trị đã xác nhận | `ToggleValue`: `Off` · `On` · `Unknown` | Không optimistic. `Unknown` (khách, chưa đọc, đọc lỗi, đích không còn) không bao giờ mặc định thành `Off` |
| Ý định mới nhất | `TogglePhase`: `Idle` · `Pending { desired }` · `Uncertain { desired }` · `Failed` | Reducer có ticket/order của caller. `Failed` = biết chắc không đổi; mất phản hồi/timeout = `Uncertain` |
| Khả dụng | `blocked: Option<String>` | `Some(lý do)` chặn bấm và hiện lý do bằng chữ (không chỉ hover); chuỗi trắng = chặn không lý do |
| Số đếm | `CountFact`: `Hidden` · `Unavailable` · `Known(n)` | Chỉ số của chính đích này; `Unavailable` hiện “–” + “chưa có số liệu”, không thành 0 |

`ToggleView::resolve` (thứ tự ưu tiên pending → uncertain → chặn → chưa rõ → thất bại → idle) trả
`pressed`, `busy`, `dispatch` (trạng thái mong muốn phát khi bấm), `note`, `reconcile`. 36 tổ hợp
value × phase × availability nằm trong vectors; generator từ chối hàng phát lệnh khi busy/uncertain/
chặn/chưa rõ, phát lại đúng trạng thái đang hiện, hoặc uncertain không mời Kiểm tra lại; và từ chối
`unavailable` hiện thành chữ số. Hai renderer test với cùng vectors.

- **Desired state, không toggle mù.** Bấm phát `on_set(!confirmed)` đúng một lần; pending/uncertain/
  chặn/chưa rõ không phát. `on_set` phải đưa phase sang `Pending` trước khi trả về để click thứ hai
  cùng frame bị bỏ qua. Bỏ phản hồi cũ (order/ticket/session) là việc của reducer, không phải view.
- **Uncertain → Kiểm tra lại**, không “Thử lại”: đọc lại trạng thái hiện hành hoặc replay đúng
  key/payload là quyết định của adapter. `Failed` cho bấm lại như một ý định mới có chủ ý.
- **Dấu chọn chỉ sau xác nhận.** Không có chữ “Đã ghi nhận” sau click; ✓ + container tonal chỉ khi
  owner trả `On`. Hai cue (hình dấu + màu), forced-colors giữ viền và dấu.
- **Tên truy cập theo hành động** (theo copy #73): `label` khi chưa bật/chưa rõ, `selected_name`
  (ví dụ “Bỏ đánh giá cao”) khi đã xác nhận Bật; tên phải chứa chữ của nhãn (WCAG 2.5.3). Không
  dùng thêm `aria-pressed` để tránh đọc “Bỏ đánh giá cao, đã nhấn”. Web: `aria-label` + `aria-describedby`
  tới `{id}-count {id}-note`; `aria-disabled`/`aria-busy` giữ focus. CMP: một node `Role.Button`,
  `contentDescription` = tên + số đếm, `stateDescription` = ghi chú; chặn dùng `enabled = false`.
- **Quiet, borderless:** recipe `action.toggle.content` trên nền trong suốt, `selected-container`/
  `selected-content` khi Bật; không viền kép/halo/glow, không cạnh tranh CTA. Hit target 48 (Web gọn
  44), glyph 20. Chữ 200% xuống dòng trong nút.
- **Glyph** là slot đầu `icon: Option<IconName>` từ registry, không đổi theo trạng thái (✓ ở slot
  cuối). Ghi nhận văn học dùng Pen Flight R2 A · Mềm (CHOICE-07), glyph Vink-owned
  có provenance/hash riêng trong `icons.json`, 20px trang trí. Gửi giữ mũi tên ngang. Không có glyph thì chữ bắt đầu sau padding 12, thẳng với Text button.

Tương quan CHOICE-10: primitive chỉ trình bày hướng A (pending + xác nhận sau ACK/readback). Hướng
B (optimistic có nhãn pending) cần một phase mới và `toggle-contract-v2`, không giả bằng cách
truyền `On` trước khi owner xác nhận. Lát adapter Web đã chốt CHOICE-10 **A** cho sản phẩm
([decision record](../../../docs/architecture/literary-endorsement.md#lát-web-adapter-wp-04-06112026)).

## Props, sự kiện và trạng thái

Tên dùng chung; Leptos viết `snake_case` và component không tiền tố (`Button`, `on_press`), CMP viết
`camelCase` với tiền tố `Vink` (`VinkButton`, `onPress`). Cột cuối ghi cả khác biệt có chủ đích.

| Component | Props chính | Sự kiện | Không sở hữu / khác biệt nền tảng |
| --- | --- | --- | --- |
| `Text` | `role: TypeRole`, `tone: TextTone` (Default kế thừa màu nền chứa nó), `tag: TextTag` (Web: P/Span/Div/Strong) | — | Copy, dịch |
| `Heading` | `level: HeadingLevel`, `role: TypeRole?` | — | Mục lục, anchor. CMP: `heading()` không cấp |
| `Surface` | `tone: SurfaceTone`, `radius: Radius`, `padding: Space`; Web: `tag`, `label?` | — | Card tương tác (bọc link riêng) |
| `Container` | `measure: Measure` (Reading/Wide/Full) | — | Breakpoint của màn |
| `Stack` | `direction: Direction`, `gap: Space`, `align: Align`, `wrap: bool` | — | Thứ tự đọc khác thứ tự nguồn. CMP: `Stretch` = `Start` (con tự quyết bề rộng) |
| `Grid` | `min: GridMin`, `gap: Space` | — | Masonry, cột cố định |
| `Divider` | `decorative: bool` | — | — |
| `Button` | `variant: ButtonVariant` (Filled/Tonal/Outlined/Text/Danger), `state: ActionState`, `leading: IconName?`; Web: `kind: ButtonKind` (Button mặc định / Submit), `id?`, `described_by?` liên kết mô tả vào native target và nối note của state | `on_press` ×1 | Fetch, retry, permission, spinner (UIS-03). CMP: không có Submit |
| `IconButton` | `icon`, `label` (bắt buộc), `state`, `pressed: bool?`; Web: `id?` | `on_press` ×1 | Tooltip hover-only |
| `Link` | `href`, `variant: LinkVariant` (Inline/Standalone); Web: `target: LinkTarget` (SameTab/NewTab); CMP: `external`, `onNavigate` | Web: trình duyệt điều hướng, giữ modifier-click; CMP: `onNavigate(href)` | Router. URL ngoài allow-list hiện như chữ ở cả hai |
| `FilterChip` | `label`, `selected`, `state`; Web: `id?` | `on_toggle(bool)` ×1 | Danh sách lọc, query |
| `ToggleAction` | `id` (bắt buộc), `copy: ToggleCopy` (label, selected_name, count_unit, ghi chú, “Kiểm tra lại”), `value: ToggleValue`, `phase: TogglePhase`, `count: CountFact`, `blocked: Option<String>`, `icon: IconName?`; Web: `blocked_description: Option<String>` opt-in tham chiếu lý do chung đang hiển thị, chỉ khi blocked/Idle (không đổi gate hoặc count) | `on_set(desired: bool)` ×1 khi được phép; `on_recover(Reconcile)` | Request, order/ticket, session, đếm lại. CMP: `blocked` dùng `enabled = false`, tên + số trong `contentDescription` |
| `TextField` / `TextArea` | `label`, `value`, `help?`, `error?`, `required`, `mode: FieldMode` (Editable/ReadOnly/Disabled), `kind: TextKind` (Text/Email/Url/Tel), `rows` (TextArea); Web: `id`, `autocomplete?` | Web: `on_input(TextInput { value, composing })`, phát thêm khi `compositionend`; CMP: `onValueChange(TextFieldValue)` | Trim/normalize/parse, validate, save |
| `SearchField` | như TextField + nút xóa khi có chữ | `on_input`; `on_submit(String)` (Web: Enter ngoài composition; CMP: hành động Search của IME) | Tìm kiếm, debounce |
| `Checkbox` / `Switch` | `label`, `checked`, `help?`, `error?` (Checkbox); Web: `id`, `disabled`; CMP: `enabled` | `on_change(bool)` ×1 | Read-only (không có trên choice native) |
| `RadioGroup` | `legend`, `options: [ChoiceOption]`, `value: String?`, `help?`, `error?`, `required`; Web: `id`, `disabled`; CMP: `enabled` | `on_change(String)` ×1 | Read-only |
| `Select` | `label`, `options`, `value: String?`, `placeholder`, `help?`, `error?`, `required`; Web: `id`, `disabled`; CMP: `enabled` | `on_change(String?)` | Tìm trong danh sách, chọn nhiều. Web `<select>` native, CMP menu Material |
| `FormSummary` | `title`, `items: [(field_id, message)]` | Web: link `#field_id` rồi focus field; CMP: `onSelect(field_id)` để host focus | Tự gom lỗi |
| `Icon` | `name: IconName`, `label: String?`, `size: IconSize` | — | Icon tự vẽ cho action đã có icon |
| `Avatar` | `name?`, `identity_key?`, `image: ImageState`, `size: AvatarSize`, `kind: AvatarKind` (Author/Account), `decorative`; CMP: `narrow` (host chọn theo bề rộng cửa sổ) | — | Gravatar, tra email, ảnh có xác thực, suy account từ tên |
| `AvatarGroup` | `people: [Person]`, `max_visible`, `total: usize?`, `label`, `size` | — | Đếm, tải thêm |
| `AuthorByline` | `name`, `identity_key?`, `image`, `meta: [String]`, `size`; Web: `href?` (đã xác minh); CMP: `onOpenProfile?`, `narrow` | Web: link; CMP: `onOpenProfile()` | Route profile |
| `ImageFrame` | `image: ImageState`, `alt` (rỗng = trang trí), `ratio: AspectRatio` | — | Tải/giải mã ảnh, auth |

`ActionState`: `Enabled` · `Disabled { reason }` · `Pending { status }`. Web dùng `aria-disabled`
(+`aria-busy` khi pending) để control vẫn nhận focus và đọc được lý do; click bị chặn và
`preventDefault` nên Submit không gửi form. CMP: `Disabled` dùng `enabled = false` (TalkBack vẫn đọc),
`Pending` giữ node có focus, bỏ qua click và đặt `stateDescription`. Bề rộng không đổi khi pending
(label giữ nguyên, slot status có kích thước cố định).

`FieldMode`: `Editable` · `ReadOnly` (chọn/copy được, không sửa; Web `readonly`, CMP `readOnly`)
· `Disabled` (không focus). Error rỗng hoặc chỉ khoảng trắng **không** tạo invalid.

`ImageState`: `Absent` · `Loading` · `Ready { src }` (Web: URL; CMP: `Painter` đã giải mã) ·
`Failed`. Chỉ `Ready` hiển thị ảnh. Khi tải hoặc giải mã lỗi, Web hiện initials hoặc
fallback của khung ảnh. Bộ lọc nguồn nhận `https://`, `http://`, đường dẫn gốc `/…`,
`blob:` do adapter tạo và `data:image/{png,jpeg,webp,gif};base64`. Nguồn bị từ chối
được xử lý như `Failed`. Producer và adapter chịu trách nhiệm về định dạng bytes,
MIME và quyền tải theo [quy ước ảnh nội dung](content-images.md).

## Identity: initials và màu avatar (`avatar-identity-v1`)

Initials (Rust `vink_ui::identity::initials`, Kotlin `vink.ui.initials`):

1. Chuẩn hóa NFC tên hiển thị; tách từ theo khoảng trắng Unicode.
2. Bỏ từ không có chữ cái (BMP) hoặc chữ số ASCII — emoji, dấu câu đứng riêng.
3. Lấy ký tự chữ/số đầu tiên của **từ đầu** và **từ cuối** (một từ → một initial), kèm dấu tổ hợp
   ngay sau nó nếu NFC chưa gộp; viết hoa theo Unicode.
4. Không còn từ nào → `None` (neutral).

An Nhiên → AN; Nguyễn Hoài Thương → NT; Trần Linh → TL; Nguyễn → N; blank → neutral. **Khác bản cũ
của `appearance.rs`** (lấy ký tự đầu bất kỳ): `(Minh)` → M thay vì `(`, `🌸 Hoa` → H, chuỗi chỉ
emoji → neutral thay vì nửa grapheme. App chỉ gọi với `None` nên không đổi hiển thị hiện tại.

Màu: `tone = 1 + FNV-1a-32(UTF-8(NFC(identity_key))) mod 6`, key do caller cấp (ID ổn định, không
phải email, không phải vị trí trong list). Thiếu/rỗng key → tone trung tính. Tone giống nhau giữa
Sáng/Tối/đổi thứ tự; mỗi tone có cặp `color.avatar-N`/`color.on-avatar-N` đạt 4.5:1 cả hai theme.
Golden vectors ở [fixtures/identity.json](fixtures/identity.json), sinh vào hai renderer và được test
ở cả hai. Giới hạn đã biết: chữ thuộc ngoài BMP, Nl và Other_Alphabetic không được hứa giống nhau.

Kích thước (`tokens.json`, R1.2 #100 ngày 09/11/2026; spotlight giữ amendment 02/11): feed 32/32, spotlight 40/38, reader 44/44
(rộng/hẹp <760px); `Small` 32 và `Medium` 40 giữ token UIS-01.

## Ví dụ

```rust
use vink_ui::{ActionState, Button, ButtonVariant, TextField, TextInput};

let title = RwSignal::new(String::new());
view! {
    <TextField id="title" label="Tiêu đề" value=title on_input=move |i: TextInput| title.set(i.value)
        help="Có thể sửa sau." />
    <Button variant=ButtonVariant::Filled state=ActionState::Enabled on_press=move |_| save()>"Lưu"</Button>
}
```

```kotlin
var title by remember { mutableStateOf(TextFieldValue("")) }
VinkTextField(label = "Tiêu đề", value = title, onValueChange = { title = it }, help = "Có thể sửa sau.")
VinkButton("Lưu", onPress = ::save, variant = ButtonVariant.Filled)
```

## Ghi chú migration cho caller

- `apps/vink-web/src/appearance.rs::initials` ủy quyền cho `vink_ui::identity::initials`, giữ dấu `•`
  khi neutral; không có thay đổi DOM/route/persistence.
- Chưa thay `vk-*` của app. Khi adoption (#23): thay từng control bằng component, xóa CSS cũ cùng
  PR, chạy `verify web`; không trộn hai bản của cùng control trên một màn.

Native HTML forms: Checkbox nhận name/value tùy chọn; HiddenField chỉ serialize context do app validate, không hiển thị/focus và không có counterpart CMP. Preview field.form + native_get_fields_preserve_names_values_and_selected_state kiểm phần này.
