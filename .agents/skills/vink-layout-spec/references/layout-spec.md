# Quy ước mô tả bố cục

Đây là pseudo-markup của Vink, không phải YAML hay schema thực thi. Dùng code fence
`text`, không thêm parser/compiler/code generator. Cây thụt dòng là bản chính;
ghi chú bên ngoài giải thích điều cây chưa diễn đạt được.

## Cây và nguồn

Dùng `Screen`, `Region`, `Row`, `Column`, `Grid`, `Overlay` khi phù hợp. `Screen`
là ngữ cảnh; các vùng/bố cục có `#id` ổn định và mục đích. Thứ tự cây thể hiện thứ
tự đọc dự kiến; ghi rõ nếu vị trí thị giác khác, giữ DOM/focus hợp lý. Tên component
dự kiến phải tách khỏi tên đã có trong source. Không bịa component từ screenshot.

Nguồn hiện hành cần kiểm: [tokens/component CSS](../../../../apps/vink-web/style/vink.css),
[routes/shell](../../../../apps/vink-web/src/lib.rs), phần page liên quan và
[requirements](../../../../prd/requirements.md). Dùng
[hệ thiết kế](../../vink-ui-design/references/design-system.md) cho tiêu chí thẩm mỹ;
snapshot trong đó không thay thế việc đọc source ở revision đang làm.

## Các hướng để so sánh

Mỗi hướng gồm tên/mục tiêu, cây vùng ngắn, nội dung và CTA chính, cách xuống màn
hẹp, một trạng thái khó và đánh đổi. So cùng input; thay phân bổ không gian, thứ
tự đọc, vị trí action, mật độ hoặc cách mở nội dung phụ. Khuyến nghị theo tác vụ,
không chọn chỉ vì hình đẹp. Số lượng theo yêu cầu người dùng, mặc định 2–3 cho màn
mới/redesign chưa chọn hướng; sửa nhỏ không cần nhiều hướng toàn màn.

## Hướng được chọn

Dùng mẫu dưới theo độ phức tạp. Ghi `không có` cho overlay/cuộn riêng khi đó là
quyết định bố cục; không bịa trạng thái hoặc tính năng chỉ để điền trường.

```text
LayoutSpec: [tên hướng, revision của spec; selected/proposed và người chọn]
Context: [vai trò, tác vụ, surface/route, state; nội dung thật được phép hoặc mẫu]
Sources: [branch/SHA, dirty inputs, file đã đọc; giả định và gap]

Screen #screen — [mục tiêu]
  Column #page — [thứ tự đọc chính]
    Region #heading — [tiêu đề/ngữ cảnh; component hiện có hoặc đề xuất]
    Grid #workspace — [phân bổ không gian]
      Region #main — [nội dung, CTA chính và thao tác phụ]
      Region #support — [thông tin hỗ trợ và cách mở]

Hierarchy: [thứ tự đọc/focus; mức nhấn và lý do; CTA chính có thể đổi theo state]
Sizing: [track co giãn, giới hạn nội dung, spacing token, xử lý title dài]
Responsive: [wide → narrow: giữ/đổi hàng/vị trí/thu gọn; control để mở phần ẩn]
Scroll: [cuộn chính, cuộn riêng, sticky/fixed và lý do; bàn phím có che không]
Overlays: [anchor/trigger, dismiss, focus vào/giữ/trả; bàn phím xuất hiện]
Themes: [light/dark dùng chung cấu trúc; surface/text/action/selected/focus/disabled]
States: [loading/empty/error/pending/success/conflict có liên quan; cách khôi phục]
Accessibility: [heading/landmark, names, keyboard, reflow/tăng chữ, thứ tự focus]
Compare: [vùng/state/viewport/theme và bằng chứng cần để đối chiếu kết quả]
```

Ưu tiên độ rộng linh hoạt và giới hạn nội dung có lý do. Ghi nguồn của kích thước,
breakpoint và token; đánh dấu đề xuất khi chưa có. Không dùng tên như `--vk-sidebar`
như thể đã tồn tại khi source chưa định nghĩa. Mô tả component bằng vai trò cho
đến khi tìm được owner phù hợp.

Light/dark giữ cấu trúc và nghĩa hành động, trừ khác biệt có lý do được nêu rõ.
Ánh xạ role sang token thật; token chưa có phải ghi đề xuất ở owner CSS. Tách nền,
chữ, action, selected, focus và disabled; token đúng tên chưa chứng minh contrast.
Theo M3 Expressive qua phân cấp, nhóm, kích thước và nhấn có mục đích; vùng đọc/viết
yên tĩnh, không biến mọi vùng thành card/pill ngang mức nhấn.

## Bản rút gọn cho sửa nhỏ

Đủ một đoạn kèm cây vài dòng. Nêu nguồn/vùng/state, trước → sau, phần thay đổi ở
màn hẹp/tăng chữ, theme/focus/state liên quan và cách kiểm. Nếu không đổi cấu trúc,
nói rõ điều đó trong spec và chỉ mô tả control bị sửa; không điền cả mẫu toàn màn.

```text
Region #editor-bar — trạng thái lưu và thao tác hiện có
  Region #save-status — giữ thông tin cần xử lý
  Region #save-action — nút Lưu bản nháp
Before → After: [vấn đề đã quan sát hoặc giả thuyết có nhãn → sửa đúng vùng]
Narrow/theme/state: [wrap/spacing/reflow; token và nghĩa hành động liên quan]
Check: [source/render/interaction cần kiểm, chưa chạy thì ghi rõ]
```

Handoff UI truyền bản spec đã chọn (nội dung hoặc đường dẫn/revision truy cập
được), lý do chọn, acceptance và gap. Nếu chưa có spec, ghi việc hoàn thành và
trình bày spec trước implementation là bước đầu của agent nhận việc. Không bắt
viết lại spec đã có chỉ để đủ template.

## Grid Areas và wireframe tùy nhu cầu

Chỉ thêm Grid Areas khi các track/span khó hiểu từ cây. Ghi số track, tỷ lệ/co giãn
và ma trận màn rộng/hẹp; các hàng cùng số ô, mỗi area liên tục hình chữ nhật, ID
khớp cây. Không duy trì hai bố cục trái nhau.

Wireframe là đầu ra tùy chọn suy từ cây, có thể dùng PlantUML Salt nếu công cụ phù
hợp và cần trao đổi trực quan. Giữ ID/thứ tự hoặc bảng ánh xạ khi định dạng không
giữ ID. Trình bày cây trước thao tác tạo preview; công cụ render không là điều kiện
để giao spec văn bản. Chỉ nói đã render khi có render thật; wireframe không chứng
minh tương tác. Không cài renderer chỉ để kiểm một thay đổi skill/tài liệu.

## Đối chiếu

So theo ID vùng: hiện diện, cha–con, thứ tự, CTA, responsive, theme và trạng thái
có liên quan. Ghi khớp/lệch có chủ ý/lệch cần sửa/chưa đủ bằng chứng; dùng đúng
revision và nêu lý do thay đổi. Audit thiếu spec ghi cây quan sát cùng nguồn,
không tự gọi đó là bố cục đã duyệt hoặc bắt tạo spec hồi tố để được review.
