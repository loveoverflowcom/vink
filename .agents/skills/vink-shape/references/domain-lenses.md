# Bản đồ miền Vink khi định hình tính năng

Dùng các câu hỏi dưới đây để tìm luật chi phối tình huống. Với mỗi ID, tra revision hiện hành bằng
`cargo xtask decision query <ID>` rồi đọc yêu cầu hoặc đặc tả của miền. Chỉ hỏi chủ sản phẩm
phần còn bỏ ngỏ có ảnh hưởng đến công việc đang làm. Các ID `CHOICE-NN` của lượt 10/11 mang
tiền tố `PRODUCT-DECISIONS-20261110-`. [Cách đọc registry](../../../../decisions/README.md)

## Xuất bản, audience và xóa bài

- Tác giả muốn công khai revision nào? Bản nào đang được đọc trong lúc chờ duyệt? Đối chiếu
  audience, quyết định kiểm duyệt và ý định công khai của đúng revision/generation.
  [PRD §14.1](../../../../prd/requirements.md#141-audience-xuất-bản-và-xóa-aud-pub) · [Publication core](../../../../docs/development/publication-core.md)
- Khi URL cũ được mở, xác định tác giả đã ẩn hoặc xóa bài từng công khai, người duyệt bác
  revision đang đăng, hay admin gỡ cả bài. `O-17@r2` đã chốt trang giữ tên tác giả và bình
  luận đủ điều kiện khi moderator hoặc admin bác revision đang đăng. Trang tác giả tự ẩn giữ
  bình luận đủ điều kiện và ẩn cả tên tác giả. Tra hành vi HTTP riêng cho publication và Reader.
  [Signoff O-17@r2](../../../../decisions/signoffs/O-17.r2.toml) · [Hành vi ở URL cũ](../../../../docs/development/moderation-runtime.md#5-trang-ở-url-cũ-o-17r2)
- Khi xóa bài, truy `O-03` cho quyền ghi và tự gỡ bình luận, `O-05` cho bài từng công khai
  rồi chuyển riêng tư. Bình luận chờ duyệt có được hiện trên tombstone sau khi duyệt
  (`O-04`)? Task có đụng tới purge, thông tin hiển thị hay khôi phục (`O-08..O-10`)?
  [Đặc tả xuất bản](../../../../docs/architecture/publication-moderation.md) · [API tác giả](../../../../docs/development/publication-runtime.md)

## Kiểm duyệt và phản đối

- Ai có quyền xử lý đúng tác phẩm hoặc bình luận, và quyền còn hiệu lực lúc kết luận không?
  Khi AI trả muộn, kiểm phiên bản và quyền override của quản trị viên. Lưu nháp riêng không
  gửi bản thảo tới AI. Khi dùng provider, làm rõ dữ liệu gửi đi, thời hạn lưu và nơi xử lý
  khi provider lỗi. [PRD §14.2](../../../../prd/requirements.md#142-kiểm-duyệt-bằng-trợ-lý-ai-quản-trị-viên-moderator-và-phản-đối-mod-0715)
- Ai ra quyết định bị phản đối và trên revision nào? Tác giả đã có thể phản đối quyết định
  bác tác phẩm của evaluator hoặc moderator. Admin kết luận phản đối. Tra `O-15` nếu yêu cầu
  thêm cấp xét lại sau kết luận admin, `O-18` cho hạn của grant/hồ sơ điều tra, `O-19` cho
  sự kiện thông báo. Phân biệt ghi nhận hồ sơ với việc đã giao thông báo đến người nhận.
  [Moderation runtime](../../../../docs/development/moderation-runtime.md) · [Thông báo](../../../../docs/development/notifications.md)

## Thảo luận hai tầng và ghi nhận

- Người viết bình luận, tác giả bài và người duyệt được làm gì? Với bản sửa đang chờ duyệt,
  chữ nào hiển thị, khi nào hiện “Đã sửa”, người viết có được hủy bản sửa (`CHOICE-01`, `O-16`)?
  [Quyền sở hữu thảo luận](../../../../docs/architecture/discussion-ownership.md) · [Sửa bình luận](../../../../docs/architecture/comment-editing.md)
- Nếu revision đổi, lượt Ưng của bình luận cũ và điểm xếp hạng của bài còn áp dụng cho
  nội dung nào (`O-06`, `O-07`)? [Ghi nhận văn học](../../../../docs/architecture/literary-endorsement.md)

## Tài khoản và xác thực

- Người dùng đang đăng ký mới, đăng nhập, thêm mật khẩu vào tài khoản đã xác minh hay khôi
  phục tài khoản? `ACC-03` phân biệt username đăng nhập với bút danh công khai. Tra `O-12`
  cho cách đếm ký tự mật khẩu và `O-11` cho luật khôi phục. [PRD §14.6](../../../../prd/requirements.md#146-tài-khoản-acc-03) · [Khôi phục mật khẩu](../../../../docs/architecture/password-recovery.md)
- Trên Web, `/toi` có đăng ký, đăng nhập và thêm mật khẩu vào cùng tài khoản bằng mã xác minh mới.
  Kiểm luồng trong [Auth Web](../../../../docs/development/auth-web.md), API trong [hợp đồng App](../../../../services/vink-app/contracts/http/v1/README.md#sign-in-d-05),
  phạm vi native trong [tài liệu CMP](../../../../packages/ui-system/cmp/README.md), và cấu hình cho
  phép đăng nhập ở production trong [runbook Auth](../../../../docs/development/auth-operations.md).

## Bộ sưu tập, phân loại và khám phá

- Chỉ hiện bài khi có membership và người xem được đọc cả bộ sưu tập lẫn bài. Khi cần sửa
  membership hoặc thứ tự, tra `O-01`, `O-02`. [Bộ sưu tập](../../../../docs/architecture/collections.md)
- Tác giả đang dùng nhãn theo catalog hay hashtag của bài công khai? Đọc `CHOICE-04`,
  `O-13` khi thay nguồn hoặc giới hạn hashtag. [Phân loại](../../../../docs/architecture/work-taxonomy.md) · [Hashtag](../../../../docs/architecture/hashtags.md)
- Với Đề xuất và Top 100, bài nào còn đủ điều kiện lúc đọc trang tiếp theo? Tra `CHOICE-02`,
  `O-14` cho công thức và mốc thời gian, `CHOICE-03` cho snapshot của cursor.
  [Đặc tả khám phá và xếp hạng](../../../../docs/architecture/discovery-ranking.md)
