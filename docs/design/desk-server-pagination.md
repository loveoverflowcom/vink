# Bàn viết — phân trang server (#201), LayoutSpec r1

Target cho tác giả tìm/duyệt toàn bộ nháp riêng theo [UX-03 và WR-01/03](../../prd/requirements.md).
Hướng kỹ thuật CHOICE-03 B trong [#201](https://github.com/loveoverflowcom/vink/issues/201):
Tải thêm qua coordinator #202; không redesign shell/editor. Bản này được trình bày trước UI code.
Source regression reference: `ebbde22`, [R6](desk-r6.md), SearchField/Select/Button hiện hữu,
Quiet Lavender và shared primitives. `fidelity: NO_APPROVED_REFERENCE`; không style anchor mới.

```text
Screen #desk — tìm lại và viết tiếp
  Region #heading — Bàn viết, Tạo bản thảo
  Region #resume — bản cập nhật gần nhất của owner; CTA Viết tiếp
  Region #drafts
    Region #draft-controls — tìm tiêu đề, Recent/Title, Tải lại
    Region #draft-scope — phạm vi toàn bộ bản thảo; số mục đang hiển thị
    Region #draft-results — các hàng theo thứ tự server, key=work_id
    Region #draft-continuation — Tải thêm / thử lại / hết / xem phần tiếp
  Region #privacy — nháp riêng, giữ copy hiện hữu
```

Viết tiếp luôn dựa vào Recent của owner, độc lập query/Title. Không tải manuscript để làm card.
Tìm vẫn gập dấu/case/NFC–NFD/Đ/whitespace, giữ dấu câu và “Chưa đặt tên”; nhãn phạm vi mới
chỉ được bật khi consumer dùng API server. Input đang gõ khác query đã áp dụng: Enter hoặc
submit commit cùng intent, composition không commit. Clear xóa và áp query rỗng khi không
composing. Sort áp ngay với query đã commit; không nuốt chữ đang nhập.

Mỗi trang 24 summaries, tối đa 10 trang trong phần đang hiển thị (240). Trước giới hạn là
“Tải thêm”; khi còn cursor ở giới hạn là “Xem phần tiếp theo”: một intent rõ thay danh sách
bằng trang kế tiếp, giải phóng phần cũ và bắt đầu chain phần mới qua primitive #202. Luôn có
“Tải lại” để trở về đầu. Không vô hạn scroll, không giả tổng count. Scope ghi số đang hiển thị;
terminal chỉ khi server không còn cursor, không khi lỗi hoặc đạt giới hạn bộ nhớ.

| State | Hiển thị và phục hồi |
| --- | --- |
| First loading | Khung Bàn viết, trạng thái “Đang mở Bàn viết…”; chưa claim rỗng |
| First-use empty | Không có recent owner và kết quả rỗng: tạo bản thảo đầu tiên |
| Search pending | Controls/input giữ node; vùng kết quả báo đang tìm đúng query đã commit |
| No results | Giữ Viết tiếp/controls; không có kết quả cho query, không đổi thành first-use |
| Refresh | Giữ dữ liệu/focus khi chờ; thành công thay toàn chain, thông báo tải lại từ đầu |
| Append | Giữ hàng hiện tại; nút có trạng thái “Đang tải…”; không gọi trùng |
| Append lỗi | Giữ items/cursor; thông báo và “Thử tải thêm”; không thông báo hết |
| Cursor/page invalid | Giữ snapshot để đọc, yêu cầu tải lại từ đầu; không nối dữ liệu sai |
| Exhausted | “Đã xem hết kết quả”; không tiếp tục request |
| Scope/session đổi | Xóa dữ liệu tài khoản cũ ngay; request A→B→A cũ không commit |

Refresh thành công bỏ tail cũ; nếu có draft di chuyển qua cursor hoặc page overlap, không dùng
dedup để che thiếu mục. Đọc reconnect/focus chỉ dùng lifecycle #202; không phát lại create/save
và không remount editor. Create uncertain giữ đường GET đối chiếu và intent tạo thêm có sẵn.

Interaction: touch/pointer/Tab/Enter dùng cùng controls; không overlay hay vùng cuộn riêng.
DOM/focus: heading → Viết tiếp → tìm → sort/refresh → hàng → continuation. Background response
không cướp focus; input giữ caret/IME/selection. Append giữ hàng/key và anchor, không tự cuộn.
Chuyển phần là intent rõ; đưa ngữ cảnh về kết quả mới bằng thông báo/anchor thay vì trông như
đã hết. Refresh giữ anchor khả dụng, nói rõ phải tải lại phần sau. Không motion mới; reduced
motion của shared primitives giữ nguyên.

Responsive: giữ layout R6/Quiet Lavender cả light/dark; controls và continuation wrap tự nhiên
ở 390px/chữ 200%, giữ full accessible names, không giảm chữ hoặc che hành động. Cuộn trang
dọc; không overflow ngang, cắt dấu hoặc remount input khi resize/theme.

So sánh bằng main browser scenario: 390/1440 × root font-size 100/200%; baseline R6 chỉ chống
regression. Kiểm Functional, Geometry, Visual, Accessibility/Responsive và Interaction riêng,
mở ảnh trước kết luận thị giác; keyboard và composition giả lập cần thao tác, không suy từ PNG.
OS IME/screen-reader ghi `NOT_RUN` nếu chưa chạy. Tối đa hai vòng sửa có render.

Hiện trạng lúc viết: backend/contract đã có trong nhánh #201; UI target chờ #202 merge.
Fixture giả lập timing chỉ kiểm race; corpus PostgreSQL và browser API thật kiểm boundary khác.
