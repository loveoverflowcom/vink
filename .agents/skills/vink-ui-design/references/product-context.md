# Vink: ngữ cảnh sản phẩm

Snapshot đọc ngày 31/10/2026, nhánh `develop`, commit
`85cc0b2763aaf87461a6f0382a71388251ad9614`. Các tài liệu vision/requirements v0.2
ngày 30/10/2026 vẫn là đề xuất để review; không nâng chúng thành quyết định Accepted.
Tra [bản đồ nguồn](source-map.md) khi cần đối chiếu mới.

## Lời hứa và người dùng

Vink — Việt Ink giúp người Việt viết đều hơn, được đọc với sự tôn trọng và trưởng
thành qua phản hồi có ích. Vòng lặp cần hỗ trợ: **viết → được đọc → nhận phản hồi → viết tiếp**.
Ưu tiên người viết cá nhân và người đọc chủ động, bắt đầu từ thơ, tản văn, truyện
ngắn, nhật ký công khai và bài tập sáng tác. Mô hình chuẩn bị cho nhiều chương;
phạm vi UI nhiều chương vẫn cần quyết định theo giai đoạn.

Vai trò: khách đọc, tác giả, người duyệt chính sách, biên tập Tạp Chí và quản trị.
Một người có thể giữ nhiều vai trò, nhưng đăng nhập hoặc quyền biên tập không tự
cấp quyền đăng thay tác giả. Thiết kế trẻ trung không đồng nghĩa chính sách nhận
người chưa thành niên đã được chốt.

## Ba không gian

| Không gian | Mục tiêu | Điều phải nhìn thấy |
| --- | --- | --- |
| Bàn viết | Soạn và sửa riêng tư | Bản đang làm, trạng thái lưu, phiên bản và việc tiếp theo. |
| Xưởng Viết | Chia sẻ và trưởng thành | Tác phẩm công khai, mong đợi phản hồi, góp ý gắn với văn bản. |
| Tạp Chí | Khám phá có biên tập | Tuyển tập hữu hạn, lời dẫn, lý do giới thiệu, bản được tuyển. |

Đây là phạm vi trải nghiệm, không phải yêu cầu ba ứng dụng/tab/dashboard riêng.
Trang tác phẩm và component có thể dùng chung. Khách có thể đọc bài công khai trọn
vẹn; hành động lưu/góp ý dùng quyền tài khoản theo implementation đã xác minh.

## Ý nghĩa bắt buộc phải giữ khi thiết kế

- **Nhật ký là thể loại; riêng tư là quyền truy cập.** Mặc định pilot đề xuất là nháp
  riêng và bản công khai; chưa coi nhóm kín hoặc link không niêm yết là chức năng có sẵn.
- **Lưu không phải đăng.** Bản đang sửa, snapshot bất biến gửi duyệt, quyết định duyệt,
  ý định đăng của tác giả và con trỏ bản công khai là những thứ khác nhau.
- **Được duyệt không tự thành công khai.** Ý định đăng phải gắn đúng snapshot và quyền
  hiện tại. Bản công khai cũ vẫn đọc được khi bản mới đang sửa/chờ/bị từ chối.
- **Tuyển chọn gắn với phiên bản.** Pilot đề xuất tạm ngừng vị trí Tạp Chí khi bản mới
  thay bản đã tuyển, chờ biên tập xác nhận lại; không tự thừa kế lời giới thiệu cũ.
- **Rút bài và dọn dữ liệu đồng bộ là hai trạng thái.** Không gọi hoàn tất toàn hệ
  thống khi chỉ ẩn card; không hứa xóa bản sao bên ngoài hệ thống.
- **Nháp riêng không mặc định gửi AI.** Autosave không tạo đồng ý. Kiểm duyệt, gợi ý
  tuyển chọn và writing coach có mục đích/quyền riêng. AI không cấm đăng vì bài chưa hay.
- **Quyết định lỗi/mơ hồ cần đường xử lý.** Hiển thị lý do, phiên bản, hành động sửa
  hoặc khiếu nại theo policy; không tự approve khi provider lỗi.
- **Cantos audio cần đồng ý riêng.** Không mặc định dùng chung tài khoản/session hoặc
  biến bài chữ thành bản audio; snapshot, quyền sử dụng và ghi công cần chủ riêng.

## Cộng đồng ít nội dung

Tổ chức trang hữu hạn: lời dẫn, một tuyển tập nhỏ, đề bài đang mở, CTA đọc/góp ý/viết
phù hợp. Đừng xây feed dài rỗng hay bịa lượt đọc, bình luận, tác giả và tương tác.
Nội dung minh họa trong concept phải được ghi là mẫu, không nhập vào sản phẩm như dữ liệu thật.

Ba tín hiệu phản hồi của tác giả: **Chỉ chia sẻ**, **Muốn nghe cảm nhận**,
**Xin góp ý kỹ thuật**. Đây là ngữ cảnh cộng đồng, không miễn trừ quy tắc hay báo cáo.
Lưu tác phẩm và phản hồi có ngữ cảnh là ưu tiên pilot; không lấy điểm sao, bảng xếp
hạng hoặc “độ chân thành” làm cổng đăng bài. Chỉ số hữu ích phải phản ánh vòng viết–phản hồi,
không tự đặt KPI/SLA hoặc biến lượt thích thành thước đo văn chương.

## Repository reality tại snapshot

**Đã có skeleton:** Leptos 0.8 SSR/hydration, Axum, PostgreSQL/SQLx, public reader từ
snapshot seed, Bàn viết soạn bằng CodeMirror 6 với lưu thủ công, version conflict, xem
trước, tải `.txt` và guard rời trang, pure save-state machine, worker chỉ có lifecycle, CSS Vink và lanes `cargo xtask verify quick|postgres|web`.

**Chưa có:** production auth, lệnh publication/moderation/curation hoàn chỉnh, tích hợp
Discourse, AI jobs/provider, rich text/WYSIWYG hay autosave. Không gọi editor chữ nguồn
hiện tại là S-01 đã hoàn tất. Trạng thái được mô hình hóa không chứng minh luồng người dùng đã chạy.

**Đề xuất cần tiếp tục kiểm chứng:** Hybrid B (Vink sở hữu nội dung, Discourse sở hữu
thảo luận), JS editor bridge, identity provider, age policy, nhiều chương public beta,
chi tiết moderation/curation và tự động duyệt. Skeleton không chốt các quyết định đó.

**Ngoài phạm vi mặc định:** cộng tác thời gian thực, offline-first, DM, paywall, chi trả
tác giả, chợ bản quyền, native mobile, SSO chung Cantos và chuyển thể tự động. Có thể
đề xuất khi người dùng yêu cầu, nhưng ghi rõ đó là mở rộng phạm vi.
