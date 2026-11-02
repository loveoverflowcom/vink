# Trạng thái và phản hồi — hướng dẫn thiết kế (UIS-03)

**NEW** · [#21](https://github.com/loveoverflowcom/vink/issues/21). Hướng dẫn cho người thiết kế màn
và viết copy; contract kỹ thuật (kiểu, tổ hợp hợp lệ, props) nằm ở
[states.md](../../packages/ui-system/shared/states.md), bằng chứng chạy ở
[ui-system-states-evidence](../development/ui-system-states-evidence.md). Component đã có ở cả hai
renderer nhưng **chưa màn nào của app dùng**; adoption thuộc #23 sau qualification #24.

Nguyên tắc: giữ dữ liệu và đường phục hồi đúng quan trọng hơn làm mọi lỗi trông giống nhau. Người
đọc/viết phải biết điều gì đang xảy ra, chữ nào vẫn an toàn và có thể làm gì tiếp.

## Chọn trạng thái theo câu hỏi

| Người dùng cần biết | Trạng thái | Không được nhầm với |
| --- | --- | --- |
| Đang lấy nội dung lần đầu | Skeleton cùng hình nội dung thật (chỉ sau 300ms) | Trang trắng, spinner toàn trang |
| Vẫn đang lấy sau 8 giây | Ghi chú "Vẫn đang tải" cạnh skeleton | Lỗi hay hết thời gian |
| Đang làm mới, nội dung cũ còn | Thanh mỏng ở mép trên vùng, nội dung giữ nguyên | Xóa danh sách rồi tải lại |
| Chưa có gì (lần đầu) | Panel mời bắt đầu | Lỗi; số đếm 0 khi chưa biết số |
| Bộ lọc/tìm không khớp | Panel "Không có kết quả" + Xóa bộ lọc | Empty lần đầu |
| Ít nội dung thật | Hiện đúng phần có + ghi chú nhẹ | Nhân bản spotlight/feed để lấp chỗ |
| Đã hết danh sách | "Đã hết danh sách" dưới nội dung | Empty, nút tải thêm |
| Một phần không tải được | Ghi chú trên phần còn lại + Thử lại | Panel lỗi thay cả trang |
| Không lấy được (mạng/dịch vụ) | Panel lỗi + Thử lại do màn cấp | "Không có nội dung" |
| Không có quyền / không tìm thấy | Panel trung tính + Quay lại | Lộ tên hay nội dung nháp |
| Cần đăng nhập | Panel giải thích; **không** nút đăng nhập khi auth chưa có | Login wall trước nội dung public |
| Đã huỷ | Trạng thái trung tính | Lỗi đỏ |
| Gửi không rõ đã ghi chưa | Cảnh báo + Đối chiếu; chữ còn | Thử lại tự động, "đã gửi" |
| Lưu xung đột phiên bản | Cảnh báo cạnh nút lưu + Đối chiếu/Tải bản sao | Snackbar tự biến mất, ghi đè |
| Phiên hết hạn lúc đang viết | Hộp thoại cần quyết định, chữ còn, không tự tải lại | Snackbar, chuyển trang |
| Đã lưu (máy chủ xác nhận) | Snackbar ngắn hoặc trạng thái "Đã lưu lúc …" | Báo "đã lưu" trước khi có xác nhận |

## Vị trí thông báo theo tác động

Field (lỗi của một ô) → Inline (cạnh vùng/hành động) → Region (vùng không có dữ liệu) → Banner (cả
trang) → Dialog (cần quyết định). Snackbar chỉ cho xác nhận ngắn không cần quyết định; snackbar có
hành động không tự đóng. Lỗi, xung đột, phiên hết hạn và trạng thái không chắc chắn **không bao giờ**
chỉ nằm trong snackbar. Tone luôn có icon **và** chữ, không chỉ màu.

Chỉ đọc to (live region) khi thông điệp đổi; `Assertive` chỉ cho phiên hết hạn ở banner. Không tự
chuyển focus khi người dùng còn đang nhập, trừ gửi form lỗi (focus tóm tắt lỗi) và mở hộp thoại.
Hộp thoại phá hủy: focus ban đầu vào lựa chọn an toàn; Escape/bấm ngoài theo chính sách; đóng thì
trả focus về nút đã mở.

## Copy

- Tiêu đề nói chuyện gì xảy ra, thông điệp nói chữ/dữ liệu nào còn an toàn, hành động là động từ
  ("Thử lại", "Đối chiếu", "Tải bản sao (.txt)", "Quay lại", "Xóa bộ lọc").
- Lỗi không rõ nguyên nhân không hứa "đã thất bại chắc chắn". Lỗi mạng không suy "bạn đang
  offline" từ trình duyệt. 401 đơn lẻ không thành "phiên đã hết hạn".
- Mã chẩn đoán chỉ dạng ổn định (`works.list.network`), chữ nhỏ cuối panel; không stack trace,
  token, phản hồi thô.
- Giới hạn tần suất (429) mới là fixture: API hiện chưa trả 429.

## Chuyển động

Ba mục đích, ba component: **SurfacePulse** (skeleton nội dung lần đầu), **MicroArc** (hành động
đang chờ trong nút, giữ nhãn và vùng chạm), **EdgeProgress** (làm mới/tìm đang bay; phần trăm chỉ
khi có tiến độ thật). Giảm chuyển động → chỉ báo tĩnh + nhãn, không vòng lặp; tab ẩn, ứng dụng ở
nền hoặc vùng không hoạt động → không vòng lặp nào chạy. Không shimmer phủ mọi vùng, không chuyển
động cạnh đoạn văn đang đọc.

## Kiểm trước khi chuyển màn sang component

- Màn chiếu nguồn thật (`LoadError`, `ApiError`, `CreateState`, `EditorSession`) vào trạng thái;
  thư viện không fetch, không retry, không điều hướng, không xóa chữ.
- Mọi hành động phục hồi do màn cấp; khi chưa có auth không cấp "Đăng nhập lại".
- Xem cả hai preview ở `#state.<họ>/<kịch bản>` (hẹp/rộng, Sáng/Tối, chữ 200%, giảm chuyển động) và
  sân thử `#state.playground/<recipe>`.
