# LayoutSpec R4.4 · Kiểm duyệt trong Bàn viết

Trạng thái thiết kế `DESIGN_PROPOSED`, phạm vi [#477](https://github.com/loveoverflowcom/vink/issues/477).
Kế thừa [R4.3 §5.3](layout-spec.md#53-thẻ-tác-phẩm), dựa trên mã tại `82dd51b` và
[LayoutSpec kiểm duyệt R1](../moderation-workspace.md). Drive và Canvas của issue
chưa được xem. DQ-05 và DQ-10 vẫn `OPEN`.

## Ý định và cây vùng

- `desk` → `draft-results` → một `draft-row-{WorkId}`. Giữ nhan đề, thời gian, tối đa
  ba nhãn sự thật có icon và cụm số liệu **Thấm → Nhận xét → Lượt xem** (đã bỏ nút/link “Đọc”
  theo delta 25/11 `D-02`, #323/#514 — xem [LayoutSpec Lượt xem r3](../work-views.md)). Hành động
  “Xem kiểm duyệt” (Tonal 48 có icon `verified-user` từ r3) đứng cạnh liên kết mở bản thảo, giữ
  nguyên đích, quyền và tên tác phẩm cho công nghệ hỗ trợ.
- `desk-review` → tiêu đề hộp thoại và nút Đóng → `desk-review-content` → tên/bản gửi,
  trạng thái, lý do, hồ sơ và hành động hợp lệ → “Lịch sử phiên bản và quyết định”.
- Bản hiện tại trong hộp thoại mặc định ưu tiên ý định gửi duyệt, sau đó đến bản công khai,
  rồi bản gửi gần nhất. Trạng thái “Đang công khai” gắn với pointer. Khi mở đích quyết định
  cụ thể, hiển thị đúng bản và số quyết định, kể cả khi quyết định đã được thay thế.
- Giữ token bề mặt và kiểu chữ R4.3. Hộp thoại tái dùng `DialogKind::Sheet` của Vink,
  canh giữa trên desktop và neo đáy dưới 760 px. Chữ dài có thể xuống dòng, nội dung cuộn
  trong giới hạn của overlay, các nút xuống hàng khi thiếu chỗ. Mục tiêu tại 320/390/1024/1440 px,
  sáng/tối, chữ 100/150% và stress 200% là không cuộn ngang và vẫn tới được nút Đóng.

## Bảng trạng thái thiết kế

| Dữ liệu mẫu | Thẻ theo R4.3 | Chi tiết |
| --- | --- | --- |
| Riêng tư, chưa có bản gửi | Chỉ mình tôi | Chưa có bản gửi để xem |
| Đã chụp và chưa gửi | Trạng thái tác phẩm theo danh sách | Bản chưa gửi duyệt |
| R1 công khai, R2 pending | Có bản công khai và bản mới chờ duyệt | Mở R2. R1 nằm trong lịch sử, vẫn là bản đang công khai |
| R1 công khai, R2 needs admin | Có bản công khai và bản mới chuyển người duyệt | Mở R2. Không có yêu cầu tác giả thao tác |
| R1 công khai, R2 bị từ chối | Có bản công khai và bản mới bị từ chối | Lý do dài xuống dòng. Chỉ hiện nút xem xét khi máy chủ cho phép |
| Đã được duyệt, pointer trống | Chưa có bản công khai | Quyết định duyệt. Trạng thái công khai vẫn lấy từ pointer |
| Đã rút/ẩn hoặc quyết định cũ | Nhãn đời sống từ read-model | Quyết định theo đúng số phiên bản. Hồ sơ cũ nằm cùng quyết định |
| Hồ sơ mở, giữ, đảo, đóng | Nhãn kiểm duyệt từ read-model | Hồ sơ và lý do kết luận nếu có; receipt phân biệt ghi nhận mới với đã có |
| Trạng thái mới chưa biết | Nhãn chưa rõ theo read-model | Trạng thái chưa nhận diện. Không suy ra được duyệt |
| Đang tải, lỗi mạng, sai đích hoặc mất quyền | Danh sách vẫn do Desk quản lý | Loading; tải lại khi phù hợp; thông báo riêng cho đích không thể mở |

## DecisionMap và tương tác

| Lựa chọn kỹ thuật | Dùng trong PR | Căn cứ |
| --- | --- | --- |
| CHOICE-01 | Hộp thoại/sheet thay vì mở rộng hàng | Primitive hiện có quản lý Escape, focus, nền inert và cuộn. Chi tiết dài dùng disclosure ngay trong overlay, không đẩy hàng khỏi vị trí. |
| CHOICE-02 | URL chuẩn `/ban-viet?review={WorkId}`; đích quyết định thêm `revision={RevisionId}&decision={DecisionVersion}` | URL tra đích độc lập với danh sách đã tải. URL cũ chuyển đến URL chuẩn. Mở từ thẻ thêm một mục history, Đóng/Escape đi Back. Đóng liên kết trực tiếp thay URL bằng Bàn viết. |
| Dữ liệu chi tiết | Đọc theo nhu cầu trong phạm vi tác giả | Một snapshot gồm trạng thái công khai, lịch sử quyết định duyệt/từ chối và hồ sơ phản đối. Lịch sử dùng cùng phép chiếu lý do dành cho tác giả của Gateway thông báo. |

Focus ban đầu vào tiêu đề hộp thoại. Khi đóng, primitive trả focus về nút đã mở;
Bàn viết dùng tiêu đề làm đích khi nút đó không hiện diện. Tìm/sắp xếp/phân trang không phụ thuộc
query chi tiết. Với Editor, dùng link thật để đi qua leave guard sẵn có. Không tự cuộn tới
nội dung khi dữ liệu async về. Giữ lời báo dựa trên receipt, trạng thái hồ sơ và quyền tại server.

## HOW và WHY

Trước #477, PR #170 bổ sung `AuthorReviews`, tải publication state và hồ sơ phản đối cho một
tác phẩm, mở từ PublicationPanel. R1 chọn trang riêng để có đủ chiều rộng cho snapshot/lý do
và giữ Back bằng URL thật. Từ mã có thể thấy tác giả phải đi qua Editor để tìm chi tiết.
Sau #214 và #410, Desk đã có hàng theo WorkId cùng các trục trạng thái, nên hàng đó trở thành
điểm vào tự nhiên. Phần quyết định/hồ sơ vẫn gắn từng revision. Việc gộp bề mặt không đổi đối tượng
nghiệp vụ. Các luật hiện có theo ADM-04@r1, ADM-06@r1, O-17@r2 và NOTIF-03.

Bảng trên là mẫu thiết kế. Prototype render, ảnh và tương tác runtime `NOT_RUN`.
