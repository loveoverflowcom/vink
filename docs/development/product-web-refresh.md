# Làm mới và phục hồi trên Product Web

Quy tắc theo [#557](https://github.com/loveoverflowcom/vink/issues/557). Các trạng thái đang tải,
rỗng hợp lệ, sẵn sàng và đã xác nhận thành công dùng vòng đời đọc của từng vùng. Người dùng
Web desktop và Web responsive có thể dùng refresh của trình duyệt. Lỗi đọc tạm thời có một
“Thử lại” cạnh vùng lỗi. Phiên hết hạn, mất quyền và nội dung không còn khả dụng dùng thông báo
và hành động phù hợp với trạng thái. Cursor hết hạn có hành động về đầu danh sách. Lần ghi chưa
xác nhận giữ chữ và dùng đường đối soát đã có của nghiệp vụ.

## Routes và các mặt giao diện

Đối chiếu [router](../../apps/vink-web/src/lib.rs), các nhánh màn hình, dialog, sheet, popover,
action slot, nhãn hiển thị/accessible name, biểu tượng và callback đọc lại. Các ca trong cột
cuối là vị trí regression tương ứng trong [Web E2E](../../apps/vink-web/e2e/).

| Surface và owner | Trạng thái, hành động và xử lý | Regression |
| --- | --- | --- |
| `/tac-pham/:id`, Nhận xét, liên kết thẳng tới nhận xét ([discussion.rs](../../apps/vink-web/src/pages/discussion.rs)) | Bỏ nút đọc lại ở hàng chế độ. Giữ Mở/Tắt theo quyền, Xem thêm theo cursor, “Thử lại” khi GET lỗi và “Tải từ đầu” khi cursor không còn hợp lệ. Giữ “Kiểm tra lần gửi” với khóa lệnh trước. Đích trả lời đã mất giữ sao chép nháp và chọn đích khác nếu có. | `discussionReadingAndWrites`, `readerBeforeHydration` |
| Reader, Bạn đã gửi và đăng nhập tại ô soạn ([discussion.rs](../../apps/vink-web/src/pages/discussion.rs), [discussion_reads.rs](../../apps/vink-web/src/discussion_reads.rs)) | Bỏ nút đầu vùng và nút của thẻ đã duyệt ngoài trang. Giữ chip, lý do riêng, Gỡ và paging own. Lỗi đọc own/capability tạm thời có một retry của vùng. | `discussionSessionRaces`, `discussionOwnFixtures` |
| Reader chính và Reader phiên bản mới ([pages/mod.rs](../../apps/vink-web/src/pages/mod.rs)) | Lỗi tải lần đầu gọi lại Reader resource, lỗi revalidation giữ bài đang đọc. “Đọc bản mới” áp dụng một phiên bản cụ thể đã đọc được. | `standalone`, `reader` |
| `/` Khám phá/Tìm kiếm/Đang theo dõi, `/the/:slug`, bộ lọc thẻ ([discovery.rs](../../apps/vink-web/src/pages/discovery.rs), [feed.rs](../../apps/vink-web/src/pages/discovery/feed.rs), [author_discovery.rs](../../apps/vink-web/src/pages/author_discovery.rs)) | Giữ thử lại kết quả/feed/tác giả khi read lỗi, giữ về đầu khi cursor hỏng và xóa lọc khi input sai. Kết quả tác phẩm và danh mục thẻ đọc lại tại chỗ. Giữ điều hướng/phân trang và các mục lọc trong dock/sheet. | `discovery`, `author-search`, `following-feed-smoke.mjs` |
| `/ban-viet` và tìm tác phẩm ([desk.rs](../../apps/vink-web/src/pages/desk.rs)) | Giữ đối soát danh sách nếu tạo nháp chưa rõ kết quả, “Thử lại” khi đọc lỗi, và tải từ đầu khi cursor hỏng. Xem thêm vẫn lấy trang tiếp theo. | `desk`, `draftPagination` |
| `/ban-viet/:id`, trình soạn thảo, Công khai, Dạng bài và chủ đề, dialog rời trang ([desk.rs](../../apps/vink-web/src/pages/desk.rs), [publication.rs](../../apps/vink-web/src/pages/publication.rs), [work_info.rs](../../apps/vink-web/src/pages/work_info.rs)) | Giữ retry lưu và đọc thẻ sau lỗi, cả hai icon Refresh có nhãn “Thử lại”. Giữ “Kiểm tra trạng thái” sau lỗi đọc hoặc ACK cần đọc lại, cùng thao tác đối soát lưu/gửi. | `editor`, `publication` |
| `/thong-bao`, notification popover ([inbox.rs](../../apps/vink-web/src/notifications/inbox.rs), [browser.rs](../../apps/vink-web/src/notifications/browser.rs)) | Bỏ “Có cập nhật mới” và icon Refresh. Poll hiện có cập nhật badge. Lần mở kế tiếp đọc head mới. Retry count chỉ hiện khi phiên online, danh sách đã tải và lần đọc số đếm trả lỗi hoặc snapshot stale. Giữ retry first/next, Seen khi mở và Read theo intent. | `notifications` |
| `/thong-bao/nhan-xet/:id/:version`, `/tac-pham/:id/tham/:revision`, sheet Thấm ([destinations.rs](../../apps/vink-web/src/notifications/destinations.rs)) | Lỗi đọc có retry tại chỗ. RevisionChanged dẫn tới “Xem danh sách hiện tại”. Nội dung đã mất chọn đường quay về. | `notifications`, `endorsement` |
| `/toi`, `/toi/chinh-ho-so`, `/toi/dia-chi`, tài khoản, Đã lưu và dialog địa chỉ ([me.rs](../../apps/vink-web/src/pages/me.rs), [profile.rs](../../apps/vink-web/src/pages/profile.rs), [account.rs](../../apps/vink-web/src/account.rs), [saved_work.rs](../../apps/vink-web/src/pages/saved_work.rs)) | Phiên, lưu hồ sơ/ảnh và lưu tác phẩm giữ recovery của owner. “Kiểm tra địa chỉ” chỉ hiện khi trạng thái địa chỉ cần đối soát. | `auth`, `password`, `author-profile-smoke.mjs` |
| `/:author`, danh sách người theo dõi/đang theo dõi ([handle.rs](../../apps/vink-web/src/pages/handle.rs), [follow.rs](../../apps/vink-web/src/pages/follow.rs)) | Giữ retry khi lỗi hồ sơ/quan hệ/đọc tiếp và đối soát nút Theo dõi. Trạng thái rỗng/riêng tư và paging giữ ý nghĩa hiện có. | `author-search`, `author-follow-smoke.mjs` |
| Menu điều hướng, giao diện, fallback 404 ([shell.rs](../../apps/vink-web/src/shell.rs), [lib.rs](../../apps/vink-web/src/lib.rs)) | Menu chọn đích/giao diện, đóng overlay và link quay về là điều hướng hoặc thiết lập người dùng. Audit cả nhãn icon và action slot. | `appearance`, `reader` |
| `/kiem-duyet*`, `/ban-viet/:id/kiem-duyet` và phần review tác giả | Chi tiết tác giả đọc khi mở và sau lệnh, retry GET khi lỗi. Các route nhân sự đã gỡ. | [Owner](moderation-runtime.md#7-web-kiểm-duyệt) |

## Native mobile về sau

Pull-to-refresh và Refresh Indicator thuộc host native. Cử chỉ và thao tác tương đương có thể
truy cập đều gửi ý định đọc tới owner của vùng. Indicator báo tiến trình. Khi lỗi, vùng giữ
snapshot còn được cấp và hiện retry phù hợp. Trạng thái trang, focus và chữ đang soạn được giữ
trong lúc đồng bộ. Hợp đồng này dành cho native app tương lai.

## Mốc trước sửa

Tám ảnh Drive c077–080 và c255–258 đã được mở. Nhóm khách có một nút tải lại, nhóm đã đăng
nhập có thêm nút trong “Bạn đã gửi”. Cả hai nút chiếm hàng riêng ở 390 × 844. Bộ ảnh gồm
390 × 844 và 1440 × 900, sáng/tối, chữ 100%. Build SHA chưa được xác định.
[Reader và Nhận xét trên Drive](https://drive.google.com/drive/folders/1htBuLMfcEjxx7S4vY0q-3kzyWIvjPgXJ).
