# Logo Vink — V/ngòi bút R1, 08/11/2026

Nguồn hiện hành: [#117](https://github.com/loveoverflowcom/vink/issues/117),
[hồ sơ nguồn V-pen R1](../../../../docs/design/brand/vpen-r1/README.md).
ZIP SHA-256: `3e490cb95171139ab23404fb0d01e7a8a7505ea25c5a0e07ab60df7cdcc01b9e`.
Bộ vòng mực [#34](../../../../docs/design/brand/final-selected/README.md) và Enso #10
là lịch sử; không còn nằm trong thư mục được build phục vụ.

## Consumer và lựa chọn

- `vpen-r1-lockup-{light,dark}.svg`: chữ Vink đã là paths, gồm giọt mực trên i.
  CHOICE-01 A: một lockup duy nhất, cao 2,25rem, tỷ lệ 1018/410 được giữ trước decode;
  link góc phải Khám phá giữ accessible name và focus. Mobile có hàng riêng trên tìm kiếm.
- `vpen-r1-mark-{light,dark}.svg` và PNG 24/32/48/96/192/512 được chép nguyên byte.
  CHOICE-02 B: dùng cùng silhouette, không thêm optical variant hoặc quay về circle.
- Favicon theo preference thật dùng mark; fallback `vpen-r1-favicon.svg` ghép nội dung
  nguyên SVG Light/Dark, theo OS khi không có JavaScript. Head/bootstrap trước CSS giữ
  cùng authority Light/Dark/System, kể cả storage bị từ chối.
- Manifest theo theme dùng PNG 192/512 và SVG mark, `purpose: any`, `id: /`.
  Không khai báo maskable; chưa có service worker/native production app.
- `vpen-r1-lockup-mono.svg` làm CSS mask trong forced colors; mark mono giữ trong pack.
  Không áp gradient lên nền UI hoặc thêm viền/halo/shadow.

## Build và provenance

Mọi URL asset dùng prefix `vpen-r1-`; stylesheet dùng `vink.css?brand=vpen-r1` để tách cache khỏi #34. `cargo xtask build` kiểm brand,
xóa output `/pkg/brand/` cũ và chỉ copy SVG/PNG/webmanifest. ZIP, PNG tham chiếu,
manifest nguồn và concept nằm dưới docs, không được phục vụ.

`provenance.json` ghi bytes/hash và source_path trong manifest gốc. `xtask` pin độc lập
SHA của ZIP và manifest nguồn, so từng bản chép với manifest, kiểm asset bắt buộc và từ chối
file runtime ngoài record. Test âm sửa artwork rồi sửa hash, cùng file circle ngoài record,
phải bị phát hiện. Không chỉ đổi hash để hợp thức hóa logo mới.

SVG là trace từ PNG, không phải Bézier gốc; interior gradient là refinement của pack.
Xem [bằng chứng và giới hạn](../../../../docs/design/brand/vpen-r1/README.md).
