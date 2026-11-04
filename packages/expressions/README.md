# Biểu cảm Google Noto — EMO-01A

Owner của **asset identity và policy phát**, không sở hữu bình luận, reaction hay sticker đã lưu.
Pilot của [#25](https://github.com/loveoverflowcom/vink/issues/25); tích hợp composer ở #45,
reference/persistence ở #43. [HOW/WHY, LayoutSpec, registry và giới hạn](../../docs/development/expressions.md).

- `shared/catalog.json`: nguồn duy nhất của 16 asset, Unicode/lookup sequences, nhãn/alias Việt,
  category, kích thước, poster/animation, nguồn/revision/hash, giấy phép và ghi công.
- `assets/`: byte Google đã pin, tên có SHA-256; build không tải Google CDN. Không có font.
- `generate.mjs`: kiểm schema, provenance, hash, SVG/JSON safety, sinh Rust/Kotlin và fixture chung;
  `--check` mặc định, `--write` chỉ sau khi review nguồn. Không tự cập nhật từ `latest`.
- `src/policy.rs`: quyết định production thuần; Web gọi trực tiếp qua closure wasm-bindgen.
  Admission đồng thời giữ reservation trong một owner; release/cancel/dispose không đếm âm.
- `behavior/`: Quint canonical, trace replay vào Rust, negative control thiếu guard.
- `kani/`: manifest độc lập trỏ **đúng thư viện production**, tương thích compiler Kani;
  không sao chép reducer. Lệnh/version/bounds nằm trong [development guide](../../docs/development/README.md#kani).

Google hiện phát animation qua CDN `latest`, không công bố commit bất biến trong endpoint đó.
Revision animation của pilot là **snapshot nội dung**: Last-Modified 16/09/2026, hash từng file và
hash tổng trong manifest. Đây không phải SHA Git upstream. Byte đã pin mới là đầu vào production.
SVG tĩnh pin `googlefonts/noto-emoji@99701e07d3d1fe1bdbcdb14964547f0fd592966f`, lấy license
đúng tại `2D/svg/LICENSE` (Apache-2.0); license font ở root không áp cho SVG này.
Animation CC BY 4.0 có [declaration](LICENSE-ANIMATED.md) riêng.

Kotlin trong thư viện CMP hiện có chỉ nhận **contract lookup và Unicode fallback**, kiểm bằng
cùng fixture sinh ra. Chưa có native animation renderer, FFI hay consumer sản phẩm native.
