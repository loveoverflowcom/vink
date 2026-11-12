# Kiểm thành phần và báo cáo

## Inventory

Thành phần là vùng thị giác/semantic/tương tác có vai trò riêng và có thể sai độc
lập; không phải mọi DOM node. Đặt ID dễ theo dõi như `desk.save-status`,
`reader.poem-body`, `magazine.selection-card`, `nav.preferences`.

| Trường | Ghi gì |
| --- | --- |
| ID / parent | ID ổn định, vùng chứa. |
| Vai trò / state | Việc component giúp làm và trạng thái nhìn thấy. |
| Vị trí | Mô tả vùng; bbox và đơn vị nếu đã đo. Không bịa tọa độ. |
| Evidence | Ảnh đã mở, source đã đọc hoặc thao tác thực đã quan sát. |
| Source locator | Verified, candidate hoặc unknown; ảnh không tự xác định tên component code. |
| Coverage | Inspected, partial, blocked, not-run hoặc excluded cùng lý do. |

Một pattern lặp chỉ cần phân tích chung một lần; nêu riêng instance lệch. Với
vùng mục tiêu nhỏ, ghi các phần khác excluded. Không tuyên bố component “PASS
toàn bộ” khi chỉ xem được hình của một state.

## Dimensions và quan hệ

| Chiều | Câu hỏi cụ thể |
| --- | --- |
| Hierarchy | Có nhận ra nội dung và việc chính? CTA nào cạnh tranh? |
| Geometry | Alignment, spacing, density, radius, containment có vai trò rõ? |
| Type | Dấu, tên dài, line height, wrapping, khổ thơ, text scale có lỗi thấy được? |
| Color | Phân cấp/theme/selected/error có rõ? Contrast cần đo hay mới là nghi vấn? |
| Copy / icon | Nhãn ngắn nhưng đủ nghĩa? Icon có mô tả khi cần? Thông tin thiết yếu có bị giấu? |
| State | Loading khác empty/error? Lưu và public revision có bị lẫn? |
| Accessibility | Có bằng chứng riêng cho names, keyboard, focus, semantics hay chỉ có ảnh? |
| Interaction | Đã thao tác hay mới dự đoán? Có input/dismiss/back/retry? Focus có bị cướp khi refresh? IME tiếng Việt có giữ composition? Cử chỉ có đường hủy và phương án thay thế? Motion có tiết chế và dừng khi reduced motion? |
| Consistency | Có source/component baseline thật để so hay chỉ là preference? |

Sau khi kiểm từng thành phần, kiểm quan hệ: overlay với anchor, focus với drawer,
editor với keyboard, status với action, các cột với bề rộng đọc, scroll với sticky
controls. Gán finding cho chủ nhỏ nhất hoặc một quan hệ cụ thể.

## So sánh reference

Ghi nguồn, revision, trạng thái user đã chọn/duyệt nếu có, phần áp dụng, cùng
viewport/theme/state. Kết quả từng vùng: `MATCH`, `DEVIATION`,
`INTENTIONAL_DIFFERENCE`, `INCONCLUSIVE`, `NOT_COMPARED`.

Với [LayoutSpec](../../vink-layout-spec/references/layout-spec.md), giữ ID vùng
và so cha–con/thứ tự đọc/CTA cùng responsive, theme, state được yêu cầu. Nêu revision
của spec và chỗ lệch có lý do; không chỉ so pixel. Nếu thiếu spec, inventory kèm
cây quan sát là bằng chứng về ảnh/source đang đọc, không là thiết kế được duyệt.
Không thể suy ra layout ở viewport khác hoặc DOM/focus từ một cây quan sát ảnh.

Không tự phóng/cắt/mask/đổi baseline để làm metric tốt hơn. Nếu user đã đưa policy
pixel comparison, giữ nguyên cho recheck hoặc ghi rõ tại sao cần đổi và giới hạn.
Pixel diff chỉ đo khác biệt pixel; không tự định mức UX hoặc bắt layout responsive
phải giống từng pixel ở hai viewport khác nhau.

## Kịch bản Vink có thể chọn

| Kịch bản | Evidence phù hợp | Giới hạn |
| --- | --- | --- |
| Homepage 0/3 bài | Ảnh từng state: lời dẫn, empty, CTA, hữu hạn | Dữ liệu mẫu phải có nhãn; không giả traffic |
| Reader bài thơ/tản văn | Narrow/wide, light/dark, tăng chữ; mở ảnh | Không chứng minh screen reader hoặc persist |
| Bàn viết lưu/conflict | Thao tác gõ/lưu/reload/hai tab trên fixture được phép | Nếu chỉ có ảnh, chỉ review copy/hierarchy; không kích mutation production để tạo chứng cứ |
| Toolbar khi mở bàn phím | Recording hoặc runtime trên input/thiết bị có tên | Desktop resize không tự là mobile IME evidence |
| Feedback intent | Hiển thị đúng tín hiệu và khả năng phản hồi theo quyền | Intent không miễn trừ policy; gửi comment cần phạm vi được giao |
| r1 public, r2 pending | Source hoặc fixture state + UI render | Không coi demo là luồng publication hiện có |
| Curation cần xác nhận lại | Source/spec + UI theo snapshot và lý do | Không tự chốt policy đang Proposed |

Các tình huống interaction dưới đây kiểm routing theo phạm vi; áp dụng contract tại
[Interaction UX](../../vink-ui-design/references/interaction-ux.md), không mở thêm quy trình
thiết kế đầy đủ cho một lỗi control đã rõ.

| Kịch bản | Routing phù hợp | Evidence phù hợp | Không được suy diễn |
| --- | --- | --- | --- |
| Focus SearchField | `vink-ui-fix` → renderer sở hữu control | Spec rút gọn; thao tác click/tab vào field; kiểm caret, composition và bàn phím | Sửa focus control không tự ý redesign thanh navigation hoặc đổi input Web sang CodeMirror |
| Tìm kiếm bị giật/lag | `vink-ui-fix` → renderer sở hữu tác vụ chậm | Profiling render/recomposition, kiểm debounce/I/O nền | Không thêm spinner/animation để che giấu giật main thread; ảnh tĩnh không chứng minh độ mượt |
| Sheet kéo vuốt | `vink-ui-design` → `vink-layout-spec` → renderer; bug đã rõ đi thẳng `vink-ui-fix` | Contract kéo/hủy/dismiss/Back; thả tay trước/sau ngưỡng, nested scroll và insets trên host | Không tự chốt cử chỉ phá hủy hoặc cộng insets hai lần |
| Shared transition Reader | `vink-ui-design` → `vink-layout-spec` → renderer; kiểm dependency ở `vink-cmp-mobile` nếu cần | Trace consumer/router thật; kiểm chuyển đi/về, ngắt/đảo chiều, focus/scroll và reduced motion trên host được giao | Không tự dựng app/router mới hoặc thêm navigation library khi chưa có consumer phù hợp |
| Bật Reduced Motion | Audit: `vink-ui-inspector`; sửa lỗi: `vink-ui-fix` → renderer | Bật flag reduced giữa animation; assertion dừng loop và quan sát timed frames, trạng thái cuối/nhãn | Ảnh tĩnh hoặc duration 0ms không chứng minh vòng lặp đã dừng; không làm mất trạng thái |
| Đóng dialog do navigate | `vink-ui-fix` → host/router sở hữu focus | Kích hoạt action điều hướng trong dialog; kiểm close reason và focus trang mới | Không trả focus về trigger ở trang cũ đã rời đi |
| Chỉ có JVM Desktop | `vink-cmp-mobile`; tiếp tục phần độc lập | Unit test và render capture offscreen; ghi `NOT_RUN` cho host native chưa chạy | Không báo PASS cho gesture cảm ứng Android/iOS, IME hệ điều hành hoặc TalkBack/VoiceOver |
| Yêu cầu “làm đẹp hơn” | `vink-ui-design`; chỉ qua `vink-shape` khi đề xuất đổi hành vi sản phẩm chưa chốt | LayoutSpec và đối chiếu hierarchy, spacing, tokens, theme/state ở cùng viewport; giữ nghĩa thao tác hiện có | Không coi yêu cầu thẩm mỹ là quyền thêm vuốt xóa, bỏ nháp hoặc đổi tab toàn màn |

Chọn lượng quan sát đủ để trả lời câu hỏi. Nếu phạm vi quá lớn, báo phần đã kiểm
và phần chưa kiểm; không dùng ngân sách tùy ý để âm thầm bỏ yêu cầu user.

## Finding

Mỗi finding đáng hành động có:

`ID | Component/relationship | Observed fact | User impact | Rule/source |
Severity | Confidence | Evidence | Suggested fix | Recheck`

Phân loại nội dung bằng lời: quan sát trực tiếp, suy luận có căn cứ, giả thuyết
nguyên nhân hoặc sở thích/đề xuất. Chỉ dùng đường dẫn/line khi đã đọc thực.

Severity:

- **Nghiêm trọng:** có bằng chứng lộ dữ liệu, mất bản thảo, sai quyền hoặc ngăn hoàn
  toàn tác vụ cốt lõi. Nêu rõ mức xác nhận, không nâng suy đoán từ ảnh lên incident.
- **Cao:** cản rõ tác vụ, trạng thái quan trọng gây hiểu nhầm hoặc dead end.
- **Vừa:** khó đọc/tìm/thao tác, có đường vòng; ảnh hưởng tập người dùng/trạng thái xác định.
- **Thấp:** polish hoặc lệch nhẹ không cản việc chính.
- **Đề xuất:** hướng cải thiện thẩm mỹ/sản phẩm; không mặc định là defect.

Confidence: cao khi chứng cứ trực tiếp và kịch bản đủ; vừa khi có nhiều dấu hiệu;
thấp khi chỉ là giả thuyết cần kiểm. Severity và confidence độc lập.

## Báo cáo gọn

Mở đầu bằng kết luận có giới hạn: ví dụ “Đã đọc ảnh mobile dark; có hai vấn đề ở
status và toolbar. Chưa kiểm interaction.” Sau đó đưa finding có tác động lớn,
bảng inventory/coverage nếu cần, rồi nguồn và gap. Không ép xuất file nếu user chỉ
cần phản hồi trong chat. Nếu được giao sửa tiếp, giữ ID và scenario để đối chiếu.

Check dùng `passed/failed/blocked/not-run/inconclusive` theo bằng chứng thật.
Ảnh đã mở, source đã đọc, interaction, screen reader và motion báo riêng; không
gộp thành một chữ “verified”. Không coi một imagegen preview là screenshot app.
