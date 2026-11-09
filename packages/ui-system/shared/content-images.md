# Ảnh raster nội dung

Web và CMP dùng **Lossy WebP** (`image/webp`) cho đầu ra raster nội dung tĩnh, gồm
thumbnail và ảnh chi tiết khi có consumer. Đầu ra theo quy ước này loại AVIF.
Profile ảnh bìa opaque dùng VP8 8-bit,
một ảnh tĩnh, không alpha. Producer chọn kích thước theo khung hiển thị và mật độ
điểm ảnh, giữ tỷ lệ nguồn và tạo mỗi kích thước từ bản gốc. Consumer quyết định
crop. Nếu nguồn có alpha, producer cần chọn ảnh phù hợp với profile hoặc composite
trên nền có chủ đích. [#115](https://github.com/loveoverflowcom/vink/issues/115)

SVG/vector, PNG thương hiệu và manifest, ảnh chụp bằng chứng và nguồn lưu trữ theo
quy ước riêng. [Asset thương hiệu hiện hành](../../../apps/vink-web/assets/brand/README.md)

Producer/adapter giữ trách nhiệm về bytes, MIME, giới hạn kích thước và quyền truy
cập. `safe_image_src`/`safeImageSrc` lọc cú pháp URL. Đuôi URL và MIME riêng lẻ
không xác nhận codec. Primitive nhận trạng thái và hiển thị ảnh hoặc fallback.
[Hợp đồng component](components.md)

## Phạm vi đã đối chiếu

Tại base `21b10a1`, hỗ trợ AVIF đang hoạt động là mục data URL trong allow-list
Web/CMP. Mẫu dương là PNG, còn AVIF xuất hiện ở fixture âm, caller và các kiểm thử
cú pháp. Sau thay đổi, test âm Rust/Kotlin giữ ca từ chối data URL AVIF. Tài liệu
Logo Enso #10 và hồ sơ UIS-02 ghi lại các lần triển khai/kiểm chứng trước đây.
Tree hiện hành không có asset AVIF. [Lịch sử logo](../../../docs/design/README.md#logo-enso-10) · [Hồ sơ UIS-02](../../../docs/development/ui-system-primitives-evidence.md)

`WorkCard`/`Publication` chưa có trường ảnh bìa và các pattern Web tương ứng hiển thị
chữ/byline. Pipeline ảnh bìa production là `NOT_IMPLEMENTED`. Đường HTTP trong
issue phục vụ fixture tĩnh ở preview loopback. Kiểm chứng Android/iOS thuộc issue
tiếp theo. [DTO Web](../../../apps/vink-web/src/contract.rs) · [Patterns Web](../../vink-patterns/leptos/src/lib.rs) · [#116](https://github.com/loveoverflowcom/vink/issues/116)

## Fixture và kiểm tra

`identity.json` giữ base64, MIME và kích thước. Generator xác nhận RIFF chứa đúng
một chunk `VP8 ` với kích thước khớp metadata. Giới hạn container này loại `VP8L`
(kể cả near-lossless) và các chunk mở rộng cho alpha/animation. Mẫu âm là 20 byte
đầu, dừng trước payload VP8. Generator sinh vectors Rust/Kotlin. Preview build
xuất cùng bytes dương tại `/fixtures/sample.webp`. Browser kiểm MIME, payload,
giải mã đầy đủ và fallback trước/sau hydration. [Fixture](fixtures/identity.json) · [cwebp](https://developers.google.com/speed/webp/docs/cwebp) · [WebP API](https://developers.google.com/speed/webp/docs/api)

Ảnh phong cảnh lavender `[Mẫu]` do Vink tạo bằng Pillow 11.3.0 / libwebp 1.5.0.
Các phép đo dùng một minh họa có gradient và cạnh xiên, mỗi biến thể resize trực
tiếp từ RGB 768 × 512. Bảng ghi dung lượng file theo byte.

| Biến thể | q65 | q78 | q90 |
| --- | ---: | ---: | ---: |
| 256 × 171 | 890 | **1.088** | 1.808 |
| 384 × 256 | 1.374 | 1.674 | 2.802 |

Chọn 256 × 171, q78 cho fixture chung. Ở ảnh mở tại kích thước gốc, q78 làm
viền trăng bớt gãy so với q65 và nhẹ hơn q90 720 byte. 384 px là ứng viên khi
khung hoặc mật độ cần nhiều pixel hơn. SHA-256 fixture đã chọn
`8a96c5984e0165cbe1a44d97207bb3a507fa418644d1e3685c7fc9ae19293b30`.

Có thể tái tạo các mẫu đo với phiên bản encoder trên.

```python
from math import sin
from PIL import Image, ImageDraw

source = Image.new("RGB", (768, 512))
source.putdata([
    tuple(round(top + (bottom - top) * y / 511 + 2 * sin(x / 37))
          for top, bottom in zip((232, 222, 252), (123, 91, 177)))
    for y in range(512) for x in range(768)
])
draw = ImageDraw.Draw(source)
draw.ellipse((505, 62, 607, 164), fill=(255, 237, 216))
draw.polygon([(0, 431), (166, 275), (350, 439), (569, 235), (768, 402),
              (768, 512), (0, 512)], fill=(97, 72, 147))
draw.polygon([(0, 489), (227, 368), (412, 472), (658, 336), (768, 426),
              (768, 512), (0, 512)], fill=(135, 108, 184))
for width in (256, 384):
    scaled = source.resize((width, round(width * 2 / 3)), Image.Resampling.LANCZOS)
    for quality in (65, 78, 90):
        scaled.save(f"{width}-q{quality}.webp", format="WEBP", quality=quality,
                    method=4, lossless=False)
```
