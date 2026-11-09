# Khám phá và trang thẻ — LayoutSpec TAG-PR-06

Ngày 09/11/2026. Baseline develop@bc926a8befd4c1ca2a19dece4bee9e782177ca22, checkout sạch.
Nguồn: issue #100; Desktop R1.3 và ảnh R1.4 trong thư mục Drive
https://drive.google.com/drive/folders/1XVA17VWB8JxFcZWNsuoie7a7yELddet_.
Ảnh Khám phá R1.4 đã mở trước code; thông tin và số liệu mẫu không đi vào sản phẩm.
UX-01/02, PUB-02/03. Thiết kế được agent chọn trong phạm vi triển khai; không đổi quyết định nghiệp vụ.

## Lựa chọn

Khám phá kế thừa hướng R1.4 đã chọn: danh sách bên trái, bộ lọc bên phải.
Trang thẻ chưa có mẫu desktop riêng: A đặt tên/mô tả trên đầu danh sách; B đặt thông tin
ở cột phụ. Cùng ca khó: thẻ ngừng dùng, có từ khóa nhưng không có bài công khai.
Chọn A để thông tin định danh/trạng thái luôn đứng trước kết quả, kể cả màn hẹp/chữ 200%;
B dành nhiều chỗ cho danh sách nhưng trạng thái dễ bị bỏ qua khi cột phụ xuống cuối.
Không tạo số lượng thẻ hoặc tổng số tác phẩm khi contract không cung cấp.

Phân trang chọn link Trang tiếp và Về trang đầu: API chỉ có cursor tiến, không có cursor lùi.
Back/Forward của trình duyệt vẫn đi đúng các trang URL đã ghé. Không dựng Trang trước giả
hoặc giữ một danh sách tích lũy không giới hạn.

## Cây được chọn (xuất trước code)

~~~text
Screen #explore — khách tìm trong toàn thư viện công khai
  Region #explore-heading — Khám phá, lời dẫn ngắn
  Grid #discovery-layout
    Column #discovery-main
      Region #public-search — SearchField Bar R5.1, native GET
      Region #applied-filters — thẻ đã áp dụng, từng link gỡ giữ q
      Region #community-stream — heading Kết quả / Tác phẩm mới
        Region #public-collection — trạng thái refresh độc lập
        Region #public-results — tác phẩm → title toàn hàng
          WorkByline — avatar chữ 32px, tên riêng dòng, ngày, nhãn thẻ
        Region #public-pagination — Trang tiếp / Về trang đầu
    Region #tag-dock — Dạng bài, Chủ đề, Áp dụng
  Overlay #tag-picker — cùng bản chọn thử khi bộ lọc thu gọn

Screen #tag-detail — /the/:slug, cùng giao diện danh sách
  Region #tag-context — Về Khám phá, tên chuẩn, mô tả
  Region #tag-status — không tồn tại / ngừng dùng / kế nhiệm nếu server cung cấp
  Region #public-search — Tìm trong thẻ, GET giữ tag định danh
  Region #community-stream — kết quả + phân trang như trên

Screen #reader — /tac-pham/:id
  Region #reader-return — cùng q/tag/cursor/trang thẻ + neo tác phẩm
  Region #reader-header — title → WorkByline avatar 44px → link thẻ
  Region #reader-body — tối đa 42rem, giữ nguyên văn bản/khổ thơ
  Region #reader-actions — EndorsementAction và discussion hiện hành
~~~

## Geometry, tokens và trạng thái

Danh sách co giãn; bộ lọc 20rem, khe space-6. Dock ở min-width:1200px và khi cột chính
còn đủ rộng; container tối thiểu 48rem thu gọn khi tăng chữ. Dưới breakpoint dùng Dialog hiện có,
ở màn nhỏ dùng sheet. SSR/no-JS vẫn có details và form GET truy cập được.
Mỗi form có q và các tag lặp; đổi q/thẻ reset cursor. Chọn checkbox chưa áp dụng không
thay URL/kết quả. Áp dụng đóng dialog và focus heading kết quả. Escape trả focus về Thẻ.
Bản chọn thử dùng chung giữa dock/dialog; chuyển bề rộng không xóa lựa chọn.

Quiet Lavender hiện hành: surface-container-low cho filter; primary/on-primary cho selected;
outline chỉ dùng phân cách/focus có nghĩa. Chip có slot dấu ✓ cố định, không nhảy nhãn.
Thẻ áp dụng dùng TagChip #99; checkbox của bộ lọc khác cardinality bản nháp nên không
dùng TagSelector (1 dạng + 3 chủ đề) để áp đặt luật chưa có lên discovery (tối đa 4, AND).

Controls có DOM ổn định khi kết quả đang tải; response không thay ô tìm đang gõ hoặc caret.
Loading, thư viện rỗng, không khớp, query/cursor không hợp lệ, backend lỗi, thẻ không tồn tại
và ngừng dùng tách riêng. Lỗi backend không thành số 0. Cache scoped theo q/tag/cursor/schema;
response cũ không gắn vào nhãn bộ lọc mới. Same-query refresh lỗi giữ snapshot có cảnh báo.
404 publication loại card khỏi mọi snapshot đang giữ trong tab.

## Đối chiếu cần chạy

320/390/834/1024/1440px, sáng/tối, chữ100/200, nhãn tiếng Việt dài và reduced motion.
Chụp và mở ảnh trước/sau cùng ca; kiểm riêng keyboard, focus, Apply, IME Enter, GET/no-JS,
reload/Back/Forward, trang kế, reader return, alias/retired/404/503 và race.
Ảnh mẫu Drive không chứng minh app; phiên screen reader/IME hệ điều hành cần ghi riêng.
