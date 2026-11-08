# Surface R5.1 — Khám phá và Bàn viết

Delta của [#90](https://github.com/loveoverflowcom/vink/issues/90), trên
`develop@6c97917b6a7aa4c51f3b2fe816462c8a0a47fc64` sau PR #93.
[Ảnh R5.1](https://drive.google.com/file/d/1c27wRtdzE7VBk3_AlnMvZ8Nl9MYgwA7w/view)
là concept; không dùng màu, gradient, pixel hoặc dữ liệu tạo sinh làm contract.
R5.1 thay anatomy search R3/R4; giữ placement và hành vi trong
[Search R3](search-r3.md). Không thay theme authority, backend, editor hay Tôi.

## DecisionMap

| Quyết định | Chọn và lý do | Đánh đổi |
| --- | --- | --- |
| R5.1 cố định | Input một dòng: placeholder/query, label native visually-hidden, kính lúp và clear có điều kiện. Scope ngắn ngoài shell, không info/help trigger. | Nhãn không còn nổi khi gõ; accessible name vẫn tồn tại khi có query. |
| CHOICE-08 | Hàng tonal riêng, gap nhỏ. Tên dài và single-link có ranh giới rõ mà không cần divider hoặc card lồng. | Nhiều mảng nền hơn nhóm chung; giữ padding/title measure của #91. |
| CHOICE-09 | Ánh xạ trực tiếp tier semantic tại component/pattern owner; không thêm alias hoặc palette. | Search dùng field-container, public/draft link dùng surface-container-low, hover container; resume vẫn primary-container. |
| CHOICE-03 | Primitive plain-tooltip nhỏ dùng chung, `TooltipIconButton` tại refresh và preview. | Rail giữ tooltip chuyên biệt nằm ngoài scroll owner; không migrate rail gây regression không cần thiết. |
| CHOICE-04 | `SupportingDetails` native details/summary cho consumer cần detail như #101/#67. | Mở thêm chiều cao trong flow. Không đặt ở search; không yêu cầu Escape/focus-return của overlay. |
| CHOICE-05 | Giữ một inventory thực gần heading/list; bỏ ListEnd dư. Khi query áp dụng, cùng vị trí trở thành một status kết quả. | Một dòng inventory hữu ích; caps12/100 là tối đa, không là tổng thư viện. Desk announce sau 700ms ngừng gõ, bỏ qua composition. |

## Cây vùng và trạng thái

```text
Shell/rail/bottom bar hiện có
  Explore
    native GET / role=search
      SearchField Bar [submit search icon | input q + hidden native label | conditional clear link /]
      Trong danh sách đã tải (help ngoài shell)
    heading hiện có
    collection [một inventory/result status | tonal full-row links có gap]
    writing prompt/context hiện có
  Desk
    heading + create
    resume primary surface + Viết tiếp
    recent heading
    tools [SearchField Bar + scope | native sort | named refresh icon + shared plain tooltip]
    một inventory/result status, không announce mỗi phím/IME composition
    tonal full-row links [full title + version/date | decorative forward arrow]
```

Placeholder chính xác: `Tìm tác phẩm` / `Tìm bản thảo`. Query dài cuộn trong native
input; không shrink font, hard height hay label/hint thứ hai trong bar. Native label
ẩn bằng utility clip1px, không `display:none` hoặc `aria-hidden`. Search thường có
target48, shell min56; vùng text/controls tự tăng theo chữ. Các label khác giữ nguyên.

Màn hẹp và root text200 giữ reflow #91, full title và một link/hàng.
Trong WorkCard hẹp (container ≤18rem), avatar trang trí ẩn để tên
tác giả và ngày dùng toàn bộ measure, tránh bẻ những từ Việt thông thường. Reader
byline ngoài WorkCard không đổi. Tách nền bằng
role hiện hành, không resting border/underline/rim/shadow/gradient. Focus input
tô một shell outline; submit/clear chỉ tô target đang focus. Forced-colors giữ
system outline cho field/search cần phân biệt; không reset outline toàn cục.

Public query chỉ áp dụng bằng GET/Enter và đi theo URL/reader return. Desk query
chỉ ở local memory; sort, resume, create, uncertain/retry và last-success giữ nguyên.
Empty dataset, no-result, unavailable và refresh-failed có trạng thái riêng. No-result
không tự announce thêm ngoài status kết quả. Plain tooltip chỉ chứa text, hover/focus
persistent, Escape đóng riêng nó và giữ focus. Native supporting details giữ toggle/focus
bàn phím và khả năng no-JS. Không giấu warning, blocker hay unsaved state trong detail.

## Owners và consumer tiếp theo

- `field.rs` / `vink-ui.css`: single-row Bar, hidden native label và ListEnd không rule.
- `support.rs` / `action.rs` / `vink-ui.css`: PlainTooltip, TooltipIconButton,
  SupportingDetails; exports qua `vink_ui`. ID trigger ổn định; tooltip ID `{id}-tooltip`.
- `pages/mod.rs`, `pages/desk.rs`, app CSS: scope/count, query status và list gaps.
- `vink-patterns`: tonal WorkCard/DraftRow, bỏ nhãn mở lặp; full-title/single-link còn nguyên.
- Leptos preview dùng cùng primitives. CMP không đổi API/style, chưa có Bar/support parity;
  không coi Web runtime là nghiệm thu native. #101/#67 dùng shared API này, không tạo framework mới.

Bằng chứng chạy và giới hạn ở [search-r5-evidence](../development/search-r5-evidence.md).
