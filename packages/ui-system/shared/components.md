# Contract component nền (UIS-02)

**Trạng thái:** UIS-02/06 ([#20](https://github.com/loveoverflowcom/vink/issues/20)) — đã triển khai ở
cả hai renderer, đăng ký trong cả hai preview với trạng thái `implemented-unverified`. Chưa có màn
nào của app dùng các component này; adoption thuộc #23 sau qualification #24. Đây là contract ngữ
nghĩa dùng chung: Leptos (`vink-ui`) và CMP (`:ui`) giữ **cùng tên, props, sự kiện, owner và trạng
thái không hỗ trợ**, không cần cùng DOM/cây Compose.

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
| `Button` | `variant: ButtonVariant` (Filled/Tonal/Outlined/Text/Danger), `state: ActionState`, `leading: IconName?`; Web: `kind: ButtonKind` (Button mặc định / Submit), `id?` | `on_press` ×1 | Fetch, retry, permission, spinner (UIS-03). CMP: không có Submit |
| `IconButton` | `icon`, `label` (bắt buộc), `state`, `pressed: bool?`; Web: `id?` | `on_press` ×1 | Tooltip hover-only |
| `Link` | `href`, `variant: LinkVariant` (Inline/Standalone); Web: `target: LinkTarget` (SameTab/NewTab); CMP: `external`, `onNavigate` | Web: trình duyệt điều hướng, giữ modifier-click; CMP: `onNavigate(href)` | Router. URL ngoài allow-list hiện như chữ ở cả hai |
| `FilterChip` | `label`, `selected`, `state`; Web: `id?` | `on_toggle(bool)` ×1 | Danh sách lọc, query |
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
`Failed`. Chỉ `Ready` hiển thị ảnh; Web lỗi tải/giải mã (kể cả AVIF) quay về initials. Web chỉ
nhận `https:`, `http:`, đường dẫn gốc `/…` và `data:image/{png,jpeg,webp,avif,gif};base64`;
URL khác (ví dụ `javascript:`, `//host`) bị coi như `Failed`. Tải mạng và xác thực thuộc adapter.

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

Kích thước (`tokens.json`, theo amendment 02/11/2026): feed 44/40, spotlight 40/38, reader 50/44
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
VinkTextField(id = "title", label = "Tiêu đề", value = title, onValueChange = { title = it }, help = "Có thể sửa sau.")
VinkButton("Lưu", onPress = ::save, variant = ButtonVariant.Filled)
```

## Ghi chú migration cho caller

- `apps/vink-web/src/appearance.rs::initials` ủy quyền cho `vink_ui::identity::initials`, giữ dấu `•`
  khi neutral; không có thay đổi DOM/route/persistence.
- Chưa thay `vk-*` của app. Khi adoption (#23): thay từng control bằng component, xóa CSS cũ cùng
  PR, chạy `verify web`; không trộn hai bản của cùng control trên một màn.
