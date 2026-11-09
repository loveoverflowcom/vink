# V-pen R1 — migration Web #117

Nguồn được người dùng chọn ngày 08/11/2026: V cách điệu ngòi bút, nét trang giấy,
chữ Vink có giọt i, hồng → tím; nền trong suốt, hình học Light/Dark đồng nhất.

- [Ảnh tham chiếu đã chọn](https://drive.google.com/file/d/1ke1DyPhbPXeD7fRy4gunb5Zzk7-64jnR/view), bản lưu [approved-reference.png](approved-reference.png).
- [Concept Light/Dark](https://drive.google.com/file/d/1nZXJs7Gc0hLENEcx0atE8No5id8IK3lk/view), bản lưu [review-concept.png](review-concept.png); đây không phải screenshot app.
- [ZIP nguồn](https://drive.google.com/file/d/167pls4LRLgWXNIahTxrKvAmzSP7ambma/view), bản lưu [Vink_Brand_LightDark_VPen_R1_2026-11-08.zip](Vink_Brand_LightDark_VPen_R1_2026-11-08.zip), SHA-256 `3e490cb95171139ab23404fb0d01e7a8a7505ea25c5a0e07ab60df7cdcc01b9e` (36 entries).
- [Manifest gốc](asset-manifest.json), [runtime provenance](../../../../apps/vink-web/assets/brand/provenance.json).
- [#34](../final-selected/README.md) giữ nguyên ZIP và chứng cứ lịch sử vòng mực. CMP thuộc #118.

SVG là vector trace từ PNG, không phải original Bézier. Gradient nội bộ là refinement
của pack, không pixel-identical với mọi sắc độ PNG. Không tạo lại artwork trong migration.

## LayoutSpec R1

```text
Khám phá /
  #discovery-tools — desktop: tìm kiếm bên trái, link brand bên phải
    .vk-brand — href /, aria-label Vink — trang chủ, target >=44px
      .vk-brand__lockup — một full lockup, aria-hidden, không thêm chữ Vink riêng
  #explore-heading — H1/lời dẫn giữ nguyên
head — favicon/manifest/theme bootstrap trước stylesheet, preload lockup
```

CHOICE-01 A: full lockup giữ wordmark/giọt i của reference, tỷ lệ 1018/410; cao 2,25rem,
rộng `2.25rem * 1018 / 410` (khoảng 89,38px ở root16), khung giữ trước decode.
Mobile đặt logo ở hàng riêng trên tìm kiếm; 200% root text tăng khung theo rem. Giữ layout
borderless, focus/link/navigation guard hiện có; không logo trong rail/route khác.
Light/Dark cùng geometry; forced colors dùng mono lockup mask với LinkText.
CHOICE-02 B: cùng path mark ở mọi cỡ, không optical redraw; favicon fallback tự theo OS,
JS theo preference thật. Manifest `purpose: any`, không tự nhận đủ safe area maskable.

## Inventory trước/sau

Baseline source `origin/develop@f23df33e6776dd08bd089bf8a0fd767dcd3cbe0e`.
Quét toàn repo (kể cả hidden tracked paths) các từ brand/logo/vink-mark/favicon/manifest/enso/splash/icon,
rồi trace caller/CSS/URLs; broad matches như catalog alias, Kani manifests, Noto và UI glyph
được phân loại, không xem là logo. Audit thứ hai sẽ kiểm cả output build.

| Consumer | Trước | Sau / owner |
| --- | --- | --- |
| Explore `pages/mod.rs` | circle SVG + Literata text | một outlined full lockup |
| app `vink.css` | mark Light/Dark, square | lockup Light/Dark + mono forced colors, intrinsic ratio |
| SSR `lib.rs` | favicon fallback/manifest + preload mark | prefix vpen-r1 cho head assets; CSS cache key brand=vpen-r1; preload lockup |
| `appearance.rs` | compact circle theo preference/OS | mark V-pen + manifest R1 theo cùng authority |
| manifest icons | 192/512 PNG + SVG circle | 192/512 PNG + SVG V-pen, any; stable id / |
| assets/brand | 6 SVG, 12 PNG, 2 manifest, fallback | 6 SVG, 12 PNG, 2 manifest, fallback R1; xóa family cũ |
| xtask build/check | copy thư mục, kiểm hash record | kiểm pinned ZIP/manifest + source bytes/hash; từ chối asset ngoài record; clear output cũ |
| smoke browser | circle URLs/square geometry | R1 URLs, decode, lockup ratio/duplicate guard, theme/placement/focus giữ nguyên |
| docs/design/README + feature map | #34 current | V-pen R1 current; chứng cứ #34/Enso gắn lịch sử |
| shared UI navigation/previews/CMP | slots/glyph mẫu, không consumer logo Web | giữ nguyên; native/CMP scope #118 |
| Noto expressions/fonts/UI icons | assets phi brand | giữ nguyên |
| ZIP/boards/evidence cũ trong docs | lịch sử | giữ nguyên, không được site copy |

## Kiểm chứng

Xem [báo cáo kiểm chứng local](evidence/README.md) cùng ảnh/log theo revision thực. Sự tồn tại của test,
concept hay SHA khớp không phải chứng nhận app/PWA/OS PASS. Không chạy/chờ GitHub CI,
không deploy. PWA cài đặt/OS mask và screen reader cần môi trường riêng; giữ gap rõ ràng.
