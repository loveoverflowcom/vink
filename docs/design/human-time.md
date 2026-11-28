# Ngày giờ trên Product Web

Quy ước trình bày của [#619](https://github.com/loveoverflowcom/vink/issues/619) áp dụng cho tiếng Việt. `Timestamp` nằm trong `packages/vink-patterns/leptos/src/timestamp.rs`. Giá trị đầu vào là instant RFC 3339 có offset. HTML ban đầu và lần hydrate đầu dùng cùng nhãn UTC. Sau mount, Web lấy múi giờ của thiết bị và giữ chuỗi gốc trong `time[datetime]`. Mốc lỗi có nhãn “Thời gian không hợp lệ”. Projection thiếu mốc đầu công khai chỉ hiện metadata còn lại.

| Vị trí | Dữ kiện | Nhãn thường |
| --- | --- | --- |
| Reader, Nhận xét | `published_at` của bản đang đọc, `created_at` của nhận xét | Tương đối rồi ngày lịch |
| Bàn viết | `updated_at` của tác phẩm/nháp | Cập nhật + nhãn tương đối |
| Khám phá, Đang theo dõi, tác phẩm trên hồ sơ | `first_published_at` từ projection, kể cả legacy có mốc | Ngày/tháng/năm để so các lần công khai đầu qua nhiều năm |
| Đã lưu | `saved_at` của membership | Đã lưu + nhãn tương đối |
| Chuông, Inbox | `updated_at` của rollup, `revision_created_at` của bản gửi nếu có | Tương đối cho sự kiện, ngày lịch cho bản gửi |
| Chi tiết/lịch sử kiểm duyệt | `decided_at` khi có quyết định, `revision.created_at` khi ghi giờ gửi | Giờ:phút, ngày/tháng/năm |
| Lượt xem | `counted_since` | Ngày báo cáo dd/mm/yyyy tại Asia/Ho_Chi_Minh theo VIEW-09/12 |
| Auth | `expires_at` của challenge | Hạn giờ:phút địa phương. Timer giây điều khiển resend/hết hạn thuộc form |
| Cache, Retry-After, retention, timeouts | Instant kỹ thuật hoặc duration | Chủ sở hữu hiện hành. Dữ kiện ngày thuần giữ nguyên ngày |

**Nhãn tương đối.** Thời điểm tương lai hiện giờ và ngày tuyệt đối. Với quá khứ, dưới 60 giây hiện “Vừa xong”, dưới 60 phút hiện số phút tròn. Sau đó, nếu cùng ngày lịch thì hiện số giờ tròn. Ngày liền trước hiện “Hôm qua”, cách 2–6 ngày hiện số ngày. Từ 7 ngày, ghi ngày/tháng và thêm năm khi khác năm. Thứ tự này giữ “15 phút trước” khi vừa qua nửa đêm và đổi một sự kiện cách hai giờ thành “Hôm qua” nếu đã sang ngày. Ngày địa phương của từng instant dùng offset tại thời điểm đó, kể cả ngày DST dài 23 hoặc 25 giờ.

**Đọc giờ đầy đủ.** Dưới tên tác giả và ở màn chi tiết, nhấn vào giờ gạch dưới để xem giây, ngày, offset và tên múi giờ. Enter/Space kích hoạt, Escape thu lại tại cùng vị trí. Trong danh sách, nút “Ngày giờ đầy đủ” đổi nhãn tại chỗ cho cả danh sách. Mỗi hàng giữ các đích mở tác phẩm, hồ sơ hoặc quyết định. Thông báo dùng mô tả truy cập cho giờ sự kiện trên link. Nhãn xuống dòng ở bề rộng hẹp và chữ lớn.

**Vòng đời.** Một đồng hồ tại gốc App phát nhịp phút khi tab hiển thị. Nhãn có thể chậm tối đa một nhịp khi trang đang mở. Khi tab trở lại, App đọc ngay giờ và timezone. Formatter Intl dùng locale vi-VN và được tái sử dụng theo zone. Đồng hồ chỉ cấp tín hiệu hiển thị. Inbox gắn key vào dữ kiện hàng và chèn tiêu đề nhóm riêng để đổi ngày trong khi link giữ focus và thứ tự từ server. Các trạng thái tải, xác nhận Seen/Read, cursor, cache và editor có vòng đời riêng.

**Quyết định.** Đây là các lựa chọn triển khai CHOICE-01 B, CHOICE-02 A, CHOICE-03 A/B của #619. DQ-05 của #410 vẫn OPEN trong hồ sơ quyết định. Mốc đầu công khai từ DQ-08 và ngày báo cáo Lượt xem có ý nghĩa riêng. Registry TOML không cần thay đổi.
