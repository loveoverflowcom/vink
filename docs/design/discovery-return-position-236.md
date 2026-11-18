# LayoutSpec #236 — quay lại vị trí trong Khám phá

Người đọc muốn tiếp tục duyệt trang kết quả đang xem sau khi mở một tác phẩm (`UX-01/02`).
Kế thừa [trang Khám phá](tag-discovery.md), [action surfaces #225](action-surfaces-desk-discovery-225.md)
và [landmark/empty #258](empty-artwork-258.md). Mốc: `NO_APPROVED_REFERENCE`; ảnh trước/sau là
mốc regression cho cùng bài, trang, viewport và theme.

~~~text
Khám phá / Trang thẻ
  #public-search          ô tìm và các thẻ đã áp dụng
  #community-stream      vùng kết quả có tên “Tác phẩm khám phá”
    #work-<id>            hàng kèm liên kết mở tác phẩm
    Phân trang / trạng thái lỗi, rỗng hoặc chưa tải
Reader
  #reader-return         liên kết về đúng q / tag / cursor / hàng
~~~

**Interaction.** Click thường hoặc Enter trên hàng mở Reader bằng liên kết thật. History entry
hiện tại giữ fragment của hàng; modifier/middle click dùng hành vi mở tab chuẩn. Back/Forward
và “Về Khám phá” giữ query, bộ thẻ, cursor và trang thẻ. Khi phần kết quả đã có câu trả lời,
đưa hàng vào khung nhìn rồi trả focus cho liên kết. Nếu hàng vắng hoặc kết quả lỗi, dùng vùng
kết quả có tên `#community-stream`: đây là đích thấy được thay cho `works-title` cũ đã bỏ ở #225,
theo hợp đồng #258. Giữ ring sẵn có và không thêm chuyển động. Liên kết, GET và cây nội dung SSR
có nghĩa khi JavaScript chưa sẵn sàng; focus sau điều hướng client là phần tăng cường.

Khi người dùng đã chuyển focus sang control khác, callback không đổi focus hoặc viewport.
Callback cũ không cuộn hay focus trang/query mới; background refresh không phát lại phục hồi.
`NavigationGuard` tiếp tục sở hữu điều hướng; dữ liệu và việc thu hồi vẫn do public-read owner
quyết. Không giữ cây Reader/Khám phá đã rời, không lưu offset giữa các view.

**Kiểm geometry.** Anchor là mép trên của hàng, đối chiếu `scroll-margin-block` hiện có và giới
hạn cuộn thực của tài liệu. Sai lệch cho phép **2 CSS px** sau khi layout/font ổn định để chừa
làm tròn subpixel; liên kết phải nằm trên thanh dưới và không tràn ngang. Đây là dung sai riêng
của test lab, không dùng `scrollY` cố định hoặc đặt thời gian chờ trong runtime. Đo ở 1280×900
và sau resize 390×844, chữ 200% bằng root font override, reduced motion; không so tọa độ pixel
trước/sau trên hai viewport khác nhau.

## Kiểm chứng ngày 18/11/2026

Sau khi danh sách mount lại, đợi qua lần vẽ đầu rồi kiểm đúng URL, landmark và focus trước khi căn hàng. Điều này xử lý sai lệch 218,36 px ở chữ 200%. [Raw trước/sau, 50 vòng, đối chứng rollback và năm chiều UX](../development/evidence/web-perf-236-canonical/README.md).
