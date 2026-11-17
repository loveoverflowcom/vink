# Empty artwork R1 — #258 / consumer #259

Các WebP này là **đúng bytes** của bộ đính kèm [issue #258](https://github.com/loveoverflowcom/vink/issues/258),
không phải ảnh dựng lại. Nguồn ImageGen gốc và script encoder nằm trong
[ZIP Drive](https://drive.google.com/file/d/1lvsay1waxJ380GkMykzPRDLB4PEwS5yE/view),
SHA-256 `b98ea4991ed63fb85742a13703c3b1879f416f6eaf995ea2e856b19cd7a24b75`.
ZIP gồm PNG nguồn, preview, `build_assets.py`, LayoutSpec và manifest. PNG nguồn có SHA-256
`4ec467ab2a055992d6abb394e6378e8708cbfd5eac632f79bf8677233d305840`.
Giữ nguyên [manifest nguồn](asset-manifest.json), gồm generation ID, crop, encoder và các hash.
Đây là **design candidate**, chưa là golden được chủ sản phẩm duyệt.

| ID / enum `EmptyArtwork` | Bytes | Vai trò |
| --- | ---: | --- |
| `no-results` / `NoResults` | 20.070 | Tìm không khớp hoặc trang kết quả rỗng, không khẳng định corpus rỗng |
| `no-public-works` / `NoPublicWorks` | 16.444 | Successful unfiltered head rỗng, không continuation |
| `first-draft` / `FirstDraft` | 19.190 | First-use riêng tư của Desk #259; #258 nhập asset/composition |

Mỗi ảnh: 512×427, VP8 lossy + alpha, một frame, ≤30 KiB (30.720 bytes, theo `verify.mjs`).
Một tệp phục vụ cả hai theme; Web không filter/invert, Drive hotlink, inline base64, preload
hoặc tải cả bộ.

`cargo xtask build web` / `verify web` dùng `xtask/src/build.rs`: chạy `verify.mjs` rồi chỉ
chép **ba WebP** vào `target/site/pkg/empty/`; Axum static handler hiện hữu phục vụ `/pkg/empty/`.
Manifest, script, PNG và preview không được chép vào site. `verify quick` cũng kiểm manifest/hash/container.

```bash
node apps/vink-web/assets/empty/verify.mjs
node --test apps/vink-web/assets/empty/verify.test.mjs
```

Validator kiểm RIFF/chunks, alpha/static/VP8, dimensions, SHA-256, byte budget; không phải pixel decoder.
Browser regression gọi `HTMLImageElement.decode()` và kiểm kích thước tự nhiên. Phiên nhập asset
cũng full decode cả ba bằng Pillow và mở preview sáng/tối. Không chạy lại encoder trên WebP trung gian.
Muốn tái tạo: tải ZIP đã pin, kiểm SHA-256, chạy script ở thư mục ZIP với Pillow 12.3.0 / libwebp 1.6.0;
không ghi đè derivatives hoặc manifest nếu bytes mới chưa được review.

Consumer dùng `crate::ui::empty_state::{EmptyArtwork, EmptyState}` và CSS `.vk-empty-state*`.
Composition chỉ render ảnh decorative + một câu HTML + controls caller-owned; `announce=true`
chỉ khi caller không còn result status khác. Không chuyển quyền fetch/create/focus vào composition.
[LayoutSpec và state contract](../../../../docs/design/empty-artwork-258.md).
Desk #259 tiếp nhận đúng composition/bytes này; xem [LayoutSpec Desk](../../../../docs/design/empty-artwork-desk-259.md).

Consumer ngoài Web: CMP diagnostics #261 đọc chính manifest/bytes này qua Gradle
`:diagnostics:generateEmptyArtwork` (ba ID cố định, 512×427, alpha, bytes ≤30 KiB, SHA-256)
và chỉ sinh base64 vào build output, không check-in bản sao. Đổi bytes, ID hoặc manifest phải
chạy lại cả `cargo xtask ui-preview cmp verify`;
[LayoutSpec/decode CMP](../../../../docs/design/empty-artwork-cmp-261.md).
