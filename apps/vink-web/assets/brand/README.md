# Logo Vink — bộ đã chọn ngày 03/11/2026

Nguồn hiện hành: [#34](https://github.com/loveoverflowcom/vink/issues/34),
[ZIP gốc](../../../../docs/design/brand/final-selected/Vink_Logo_Final_Selected_Pack.zip).
SHA-256: `46acba3f22e5b684a1a72f4518d3de6cce0906f0511ef11472a92a056008eac4`.
ZIP có 24 tệp; bản trong repo khớp byte với file người dùng cung cấp.
[Hồ sơ lựa chọn và khác biệt board](../../../../docs/design/brand/final-selected/README.md).
Logo Enso trích raster của #10/PR #17 là lịch sử, đã được thay trong consumer Web.

## Asset và cách dùng

- Sáu `vink-mark-*.svg` và 12 `vink-{light,dark}-{24,32,48,96,192,512}.png`
  được chép **nguyên byte** từ selected pack. Không crop, vẽ lại, invert, nén lossy hay đổi gradient.
- Vùng đầu trang Khám phá (`/`) dùng SVG chuẩn Light/Dark, khung vuông 2,25rem và chữ Vink
  bằng Literata. CSS chọn theme qua `data-theme`/`prefers-color-scheme`; preload hai SVG
  nhỏ trước stylesheet.
- Favicon có JS dùng SVG compact theo preference thật. `favicon.svg` ghép nguyên defs/path
  của hai bản compact, dùng media query theo OS khi không có JavaScript. Compact lược khe
  trang trí trên vòng và mở lỗ ngòi cho 16/24/32px; PNG 24/32 trong pack dùng bản compact.
- Manifest Light/Dark dùng PNG 192/512px và SVG chuẩn dự phòng, `purpose: any`.
  Chưa có thiết kế safe area maskable, service worker hay app native production.
- `vink-mark-mono*.svg` được giữ cho bàn giao, chưa có consumer hiện hành.

## Build và provenance

`cargo xtask build` chỉ copy `.png`, `.svg`, `.webmanifest` trong thư mục này tới
`/pkg/brand/`, xóa output brand cũ trước khi copy. ZIP/board/tài liệu/provenance không được
phục vụ trong site. Tên SVG/PNG mới tách cache khỏi `enso-*`; head/CSS dùng cùng tên mới.

`provenance.json` ghi hash/byte của 24 tệp gốc và mọi asset được phục vụ. Repository checks
kiểm ZIP gốc, hash runtime, đủ 6 SVG/12 PNG và hash các bản chép so với source manifest.
Không cần CairoSVG/Pillow hoặc công cụ trích raster khi build app. Thay artwork cần đối chiếu
lại nguồn đã chọn; không chỉ cập nhật hash để hợp thức hóa một bản khác.

Bằng chứng app và giới hạn ở [baseline logo](../../../../docs/design/README.md#logo-final-selected-34)
và [feature map](../../../../docs/development/feature-map.md). Browser smoke kiểm MIME/decode,
manifest, theme/first paint trước hydration và giữ các regression editor/navigation hiện có.
