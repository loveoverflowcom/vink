# Vink logo v4 — sửa độ tròn và sự liền mạch

**PROPOSAL / CHỜ REVIEW.** V3 đã bị người dùng từ chối vì vòng và nét lệch; không triển khai V3. V4 giữ ý tưởng trong ảnh được chọn, sửa hình học thay vì tạo hướng thương hiệu khác. Chưa thay logo production hay quyết định trước ở #10.

## Hình học
- `viewBox="0 0 256 256"`; thân vòng dùng cung tròn SVG `A`, tâm `(128,128)`, bán kính ngoài `104`, trong `66`, độ dày hướng kính `38`.
- Lông và ngòi là phần tạo hình tự do có chủ đích; KHÔNG tuyên bố toàn bộ silhouette là một đường tròn. Các điểm nối thân vòng được căn theo tiếp tuyến.
- Một compound path, một gradient ba điểm màu; khe lông và lỗ/khe ngòi là khoảng rỗng thật qua `evenodd`. Không chồng hai shape để tạo khe, không vẽ màu trắng giả nền, không ảnh nhúng, blur/filter hay font nhúng.
- Light/dark dùng cùng dữ liệu `d`. Compact giữ nguyên silhouette, bỏ khe lông, nới lỗ/khe ngòi cho 16–32px. Ở 16px chỉ yêu cầu nhận ra silhouette; chi tiết ngòi cần cỡ lớn hơn.

## Tệp
- `vink-mark-light.svg`, `vink-mark-dark.svg`: bản đầy đủ, nền trong suốt.
- `vink-mark-light-compact.svg`, `vink-mark-dark-compact.svg`: cỡ nhỏ.
- `vink-mark-mono.svg`, `vink-mark-mono-compact.svg`: `currentColor`, nên dùng inline để kế thừa màu.
- `preview.svg`: bảng sáng/tối, dùng chính paths bàn giao.
- `comparison.svg`: đối chiếu V3 bị loại và V4.
- `construction.svg`: đường tròn kiểm hình học; đường dẫn hỗ trợ không thuộc logo.
- `preview.html`: mở cùng thư mục với các SVG; nút đổi theme, dãy kích thước thực.
- `checks.json`: kết quả kiểm asset và hash SVG.

Wordmark chữ Vink trong bảng xem thử là minh họa bằng font hệ thống, không phải thay đổi wordmark production. PNG trong ZIP/chat chỉ là bản render; SVG là nguồn.

## Kiểm chứng đã làm
- Parse XML; xác nhận một path, không image/filter; so sánh hình học light/dark và compact.
- Render PNG 16/24/32/36/48/96/192/512/1024px, xem bảng sáng/tối và đối chiếu V3/V4.
- Kiểm biên thân vòng qua mẫu xuyên tâm 45–225 độ trên render 2048px: sai số tối đa 0,075 đơn vị viewBox; đây là sai số lấy mẫu raster, không phải đo toàn bộ phần lông/ngòi.
- Chromium 144: fixture 1280px và 390px, giải mã SVG, chuyển theme, không đổi bounding box của mark, không tràn ngang, không lỗi JavaScript.
- Môi trường chặn điều hướng HTTP cục bộ; kiểm browser dùng HTML trong bộ nhớ với cùng bytes SVG qua data URI. Không coi đây là kiểm route/MIME/cache HTTP.

## Chưa làm / gate trước production
Chưa được người dùng duyệt V4; chưa kiểm trong app Vink, hydration/first paint, Safari/Firefox, CMP/native launcher hay screen reader. Không gán kết quả asset cho app. Chưa merge, chưa deploy.

Sau phê duyệt: thay asset qua PR nhỏ; xác minh palette với owner `packages/ui-system/shared/tokens.json`, dùng cùng hình học trên các nền tảng; không CSS invert, không scale riêng hai trục. Giữ nguyên auth, router, editor, dirty guard và điều hướng. Issue theo dõi: #34.
