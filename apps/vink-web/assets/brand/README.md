# Logo Enso Vink — nguồn và rendition

Nguồn duy nhất: [ảnh đối chiếu light/dark đã chốt trong #10](https://drive.google.com/file/d/1hHQguADQ2tbiNjQ9r8WqMnYUSEnkAyYs/view),
`vink-enso-feather-v3-light-dark-comparison.png`, 1774 × 887, sửa ngày 01/11/2026.
SHA-256: `ee10182451ce3b658d5d5160f3011e9334ab1f5fa4388d9cb751bb6076cf6cad`.
Không dùng `ink-v2` hay hình được tạo lại. Đây là nhận diện Vink/Việt Ink theo hướng
Enso, không phải tuyên bố biểu tượng tôn giáo chính thức.

Hai nửa 887 × 887 được lưu nguyên trong evidence khi trích. Cùng crop `(80, 80, 800, 800)`
trong mỗi nửa giữ vòng lông vũ, khoảng hở, giọt mực và ngòi bút. Tách nền mềm bằng khoảng
cách màu so với trường màu nội suy từ viền ảnh; không xoay, vẽ thêm hay đảo màu.
AVIF giữ màu/highlight của từng nửa riêng, alpha trong suốt, quality 95 và chroma 4:4:4;
24/32/48/96/192/512 px đều được xuất trực tiếp từ crop 720 px bằng Lanczos.

SVG là bản vector giản lược bằng VTracer spline, palette rút gọn, bỏ nhiễu nhỏ và halo nền.
Cặp vector dùng đường bao trích từ nửa light để giữ cấu trúc chung; màu vẫn lấy từ pixel
của từng nửa. Đây là giản lược có chủ đích, không khẳng định SVG/AVIF lossy khớp từng pixel
với PNG. AVIF là rendition chính trong shell vì giữ chi tiết/highlight tốt hơn.

- Shell giữ vùng mark 2.25rem hiện có, dùng AVIF 96px cho cả DPR 1/2; không đẩy layout khi decode.
- CSS theo cùng `data-theme`/`prefers-color-scheme` với token owner; không dùng filter invert.
- Hai SVG riêng làm favicon theo theme. `favicon.svg` có media query cho fallback khi không
  chạy JavaScript; bootstrap trước stylesheet đồng bộ favicon/manifest với preference thật.
- Manifest light/dark có AVIF 192/512 cùng SVG dự phòng; không có icon maskable vì chưa thiết kế
  safe area. Manifest không có nghĩa đã có service worker, offline hay native app.
- Build chỉ copy `.avif`, `.svg`, `.webmanifest` tới `/pkg/brand/`; không phục vụ ảnh nguồn,
  script hay provenance. Repository checks đối chiếu hash và đủ sáu kích thước mỗi theme.

`extract.py` là công cụ authoring tùy chọn, không cần khi build app. Dùng Python với Pillow
12.0.0, numpy 2.3.4, vtracer 0.6.12; truyền đường dẫn PNG đã tải từ Drive và thư mục evidence.
Script từ chối hash nguồn khác. `provenance.json` ghi crop, kích thước, dung lượng và hash
của mọi derivative được phục vụ. PNG chỉ ở evidence; xem phạm vi kiểm trong
[design](../../../../docs/design/README.md#logo-enso-10) và
[feature map](../../../../docs/development/feature-map.md).
