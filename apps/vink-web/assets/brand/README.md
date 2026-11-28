# Logo Vink — V/ngòi bút R1, 08/11/2026

Nguồn hiện hành: [#117](https://github.com/loveoverflowcom/vink/issues/117). Người dùng chọn
V cách điệu ngòi bút, nét trang giấy, chữ Vink có giọt i, hồng → tím; nền trong suốt, hình học
Light/Dark đồng nhất. Bộ vòng mực #34 và Enso #10 chỉ còn trong lịch sử Git.

## Nguồn và provenance

- ZIP nguồn (36 entries, SHA-256 `3e490cb95171139ab23404fb0d01e7a8a7505ea25c5a0e07ab60df7cdcc01b9e`),
  ảnh tham chiếu và concept nằm trong kho thiết kế trên Google Drive, không nằm trong Git và không
  là input của build. `provenance.json` ghi link lưu trữ.
- [source-manifest.json](source-manifest.json) là `asset-manifest.json` trong ZIP, nguyên byte.
- [provenance.json](provenance.json) ghi bytes/hash và `source_path` của từng file phục vụ.
- `cargo xtask` (check `repo` và `build`) pin SHA của ZIP, SHA và đường dẫn manifest nguồn, cùng ba
  bản dẫn xuất Web đã review. Mỗi URL artwork chỉ nhận đúng mục tương ứng trong manifest nguồn; asset
  hoặc bản ghi ngoài danh sách R1 đều bị từ chối. Test âm thử đổi Light sang file Dark hợp lệ, thêm
  URL cũ, thay favicon kèm hash mới và pin lại ZIP/manifest.
- Kiểm lại ZIP đã lưu trữ (tuỳ chọn, ngoài CI):
  `VINK_BRAND_REFERENCE_ZIP=/đường/dẫn/zip cargo xtask verify static repo`.

SVG là trace từ PNG, không phải Bézier gốc; interior gradient là refinement của pack, không
pixel-identical với mọi sắc độ PNG. Không tạo lại artwork khi migrate.

## Consumer và lựa chọn

- `vpen-r1-lockup-{light,dark}.svg`: chữ Vink đã là paths, gồm giọt mực trên i.
  CHOICE-01 A: một lockup duy nhất, cao 2,25rem, tỷ lệ 1018/410 được giữ trước decode;
  link góc phải Khám phá giữ accessible name và focus. Mobile có hàng riêng trên tìm kiếm;
  200% root text tăng khung theo rem. Không logo trong rail/route khác.
- `vpen-r1-mark-{light,dark}.svg` và PNG 24/32/48/96/192/512 được chép nguyên byte.
  CHOICE-02 B: dùng cùng silhouette ở mọi cỡ, không optical variant hoặc quay về circle.
- Favicon theo preference thật dùng mark; fallback `vpen-r1-favicon.svg` ghép nội dung
  nguyên SVG Light/Dark, theo OS khi không có JavaScript. Head/bootstrap trước CSS giữ
  cùng authority Light/Dark/System, kể cả storage bị từ chối.
- Manifest theo theme dùng PNG 192/512 và SVG mark, `purpose: any`, `id: /`.
  Không khai báo maskable; chưa có service worker/native production app.
- `vpen-r1-lockup-mono.svg` làm CSS mask trong forced colors; mark mono giữ trong pack.
  Không áp gradient lên nền UI hoặc thêm viền/halo/shadow.

```text
Khám phá /
  #discovery-tools — desktop: tìm kiếm bên trái, link brand bên phải
    .vk-brand — href /, aria-label Vink — trang chủ, target >=44px
      .vk-brand__lockup — một full lockup, aria-hidden, không thêm chữ Vink riêng
head — favicon/manifest/theme bootstrap trước stylesheet, preload lockup
```

## Build

Mọi URL asset dùng prefix `vpen-r1-`; stylesheet dùng `vink.css?brand=vpen-r1` để tách cache.
`cargo xtask build` kiểm brand, xóa output `/pkg/brand/` cũ và chỉ copy SVG/PNG/webmanifest;
`provenance.json`, `source-manifest.json` và README không được phục vụ. PWA cài đặt, OS mask và
screen reader chưa được kiểm trên thiết bị thật.
