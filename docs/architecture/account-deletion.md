# Vòng đời xóa tài khoản

[DEL-01–08 trong PRD](../../prd/requirements.md#147-xóa-tài-khoản) và [#297](https://github.com/loveoverflowcom/vink/issues/297) sở hữu quy tắc nghiệp vụ. [VDR-ACCOUNT-DELETION@r1](../../decisions/proposals/VDR-ACCOUNT-DELETION.r1.toml) đang `PENDING` cho tới signoff đúng revision/hash.

## Xác minh và đồng bộ hai DB

1. Web gửi tên đăng nhập và mật khẩu hiện tại hoặc mã sáu chữ số từ email đã xác minh. Identity so khớp password hash và version dưới khóa account, hoặc tiêu thụ mã theo purpose `delete_account` và email hiện hành của đúng account. Phiên hợp lệ giới hạn thao tác vào account đang dùng. Người bị đình chỉ hoặc Identity đã disable dùng cùng luồng xác minh riêng, không cấp phiên.
2. Identity ghi biên nhận `(account_id, operation_id, accepted_at)`, khóa chủ thể vĩnh viễn và xóa phiên mọi thiết bị, password, email và challenge cũ trong cùng commit. Migrator giữ khóa ngoại với account cũ. Username và email có thể được cấp lại qua đăng ký mới. App chỉ nhận biên nhận từ Identity qua socket hiện có.
3. App khóa subject cùng đường provisioning, khóa Publication theo WorkId và lấy standing của principal. Một giao dịch ghi hàng rào App và Social, thu hồi handle/alias, gỡ work, dọn quyền truy cập và phát hiệu ứng cho Notifications. Với account chưa có PrincipalId, App vẫn ghi hàng rào để lời gọi provision đến trễ bị từ chối. Hai DB có hai điểm commit. Khi App chưa ghi xong, Identity đã chặn phiên và trình duyệt nhận trạng thái đã tiếp nhận.
4. App đọc lại biên nhận chưa hoàn tất qua socket khi khởi động và mỗi 10 giây. Retry cùng biên nhận xử lý được rollback, restart hay mất ACK. Sau hàng rào App, giao dịch riêng dọn payload và ghi checkpoint. Adapter Social dọn cạnh hai chiều theo lô có checkpoint và đã bỏ chúng khỏi các projection ngay tại fence. Chỉ đánh dấu biên nhận hoàn tất ở Identity khi App đã dọn payload và adapter báo xong. Hạn 30 ngày tính từ `accepted_at`; công việc có thể dọn ngay trong lần đầu và ghi lỗi khi quá hạn.

Tại cutover #607, CLI inventory/retire thay bước xác minh của người dùng bằng phân loại dữ liệu legacy trong maintenance. Cùng hàm `retire` phát biên nhận. Các bước 2–4 áp dụng cho cả account đã có principal và account chưa hoàn tất bút danh. [Runbook](../development/auth-operations.md#cutover-email-bắt-buộc-607).

## Dữ liệu và quyền đọc

- Publication quyết định bằng public pointer hoặc author-hidden pointer thực. Trang thay thế đọc được nhận xét còn hợp lệ và đáp HTTP `no-store`; phiên bản trước đó không thể nhận 304 từ API. Bài chưa từng public và bài ở disposition gỡ sau kiểm duyệt không mở thread hoặc trang thay thế của tác giả.
- Giữ WorkId, binding chủ sở hữu, revision ID/digest, lịch sử thao tác và bằng chứng duyệt giới hạn trong DB. Xóa draft, nội dung/tiêu đề revision, slot tag và dữ liệu lượt xem. Không có owner lưu ảnh tác phẩm trong schema hiện tại. Handle đã dùng vĩnh viễn chiếm chỗ và không dẫn tới tài khoản mới.
- Hủy nhận xét pending do người bị xóa viết và nâng version để quyết định đến trễ bị từ chối. Giữ chữ của nhận xét công khai còn hợp lệ. Social bỏ link và chiếu tên trung tính ở cả nhận xét và ngữ cảnh trả lời.
- Saved Works dọn dòng thuộc người lưu ngay tại Social fence. Bookmark của người khác giữ WorkId, chiếu `author_deleted` trung tính. Moderated removal vẫn bị loại trước bước chiếu. App gọi `erase_work` cho Work Views trong giao dịch xóa vì WorkId vẫn tồn tại.
- Notifications nhận work invalidation và các điều chỉnh Thấm/Ưng qua outbox hiện có. Session mất hiệu lực đóng quyền vào inbox. Gateway đọc lại attribution và quyền hiện hành. Các dòng cũ trong DB Notifications theo retention của owner.

## Vận hành

Có thể đóng intake bằng cấu hình sign-in hiện hữu và tiếp tục xử lý biên nhận còn nợ. Giữ cả hai migration và các hàng rào khi sửa hoặc chạy lại. Khi khôi phục hạ tầng, chỉ mở traffic sau khi đối soát và áp dụng lại các biên nhận xóa mới hơn snapshot từ nguồn còn giữ chúng. Nếu cả hai DB đều trở lại thời điểm trước khi xóa và không còn biên nhận, hệ thống không có dữ liệu để suy ra tài khoản nào đã xóa. Các bản sao lưu hạ tầng hiện hữu giữ nguyên.

[HTTP App](../../services/vink-app/contracts/http/v1/README.md#xóa-tài-khoản) · [Identity RPC](../../services/vink-identity/contracts/rpc/v1/README.md#xóa-tài-khoản) · [LayoutSpec](../design/account-deletion.md)
