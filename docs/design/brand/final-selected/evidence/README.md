# Bằng chứng tích hợp logo #34 — 03/11/2026

Nguồn: diff `codex/final-selected-logo` trên `develop@cbcbe0d`; bộ ZIP ở thư mục cha.
macOS, Rust 1.99.1, Node 26.12.0, Chrome 156 headless, DPR 1; PostgreSQL 17.11 local riêng cho
verification. App dùng principal fixture loopback, không chứng minh production authentication.
Đây là kiểm sau sửa, không có baseline trước sửa cùng build/fixture để gọi before/after.

## Kết quả

- **PASS** `cargo xtask verify quick`: [summary](quick-summary.txt), gồm fmt, clippy SSR/WASM,
  doctests, unit/contract tests và repository links/boundaries/provenance.
- **PASS** `VINK_TEST_DATABASE_URL=<cluster-test> VINK_CHROME=<Chrome-macOS> cargo xtask verify web`:
  [summary](web-summary.txt), server Leptos/Axum thật, database tạm, Chrome thật; main và
  after-restart đều PASS, không console error/exception ngoài hai import bị chặn có chủ ý.
  Lần đầu **FAIL prerequisites** vì chưa đặt `VINK_CHROME`; các bước sau **NOT_RUN**.
  Đã chỉ định Chrome có sẵn rồi chạy lại toàn bộ lane, không bỏ hay giảm assertion.
- **PASS** đối chiếu ZIP CRC/SHA và 24 tệp giải nén, SVG XML, PNG RGBA/trong suốt/đúng kích thước;
  6 SVG + 12 PNG runtime khớp byte nguồn. [Canvas QA](logo-asset-qa.json) xác minh alpha mask
  Light/Dark của PNG 24–512 giống nhau. Không chạy lại phép render CairoSVG pixel-identical;
  đó là bằng chứng QA của selected pack đã ghi ở issue trước, không phải lệnh lần này.
- **PASS** MIME/decode SVG chuẩn/compact và 12 PNG qua HTTP app thật; manifest có PNG 192/512
  và SVG đúng URL. [Probe HTTP](logo-http-qa.json) xác minh byte phục vụ đúng source.
  Server hiện gửi `Last-Modified`, không có `Cache-Control`; request `If-Modified-Since`
  vẫn trả **200 với đúng toàn bộ byte**, không phải 304. Probe ban đầu kỳ vọng 304 đã **FAIL**;
  đã xác định đây là hành vi file handler hiện có, không thay cache policy trong PR logo.
  Probe khởi động ban đầu còn trỏ vào database vừa bị lane web dọn nên không ready; probe
  sau dùng database test riêng có migration. Tên asset mới tránh dùng cache `enso-*` cũ.
- **PASS** regression sẵn có: bootstrap trước hydration, theme override/OS/reload/storage denied,
  khung mark vuông cố định; rail mở/thu, keyboard/focus, điều hướng, một EditorView,
  selection/undo/redo, dirty guard, save/reload/conflict và text qua restart.

## Ảnh đã mở

[contexts.json](contexts.json) giữ route, viewport, font size, theme, DPR và user agent cho
12 ảnh app dưới đây. Các ảnh Light/Dark cùng viewport dùng cùng fixture của lane.

| Vùng | Light | Dark |
| --- | --- | --- |
| Rail mở 1440px | [ảnh](1440-me-light.png) | [ảnh](1440-me-dark.png) |
| Rail thu 1024px | [ảnh](1024-me-light.png) | [ảnh](1024-me-dark.png) |
| Header mobile 390px | [ảnh](390-me-light.png) | [ảnh](390-me-dark.png) |
| Header mobile 320px | [ảnh](320-me-light.png) | [ảnh](320-me-dark.png) |
| Trước hydration 1440px | [ảnh](brand-before-hydration-light.png) | [ảnh](brand-before-hydration-dark.png) |
| Asset độc lập 16–512px, hai bề mặt | [ảnh](logo-size-sheet-light.png) | [ảnh](logo-size-sheet-dark.png) |

Hai size sheet là fixture asset độc lập 1200 × 1550, màu `surface`/`surface-container` lấy từ
baseline, dùng SVG đã chọn; compact ở 16/24/32px, chuẩn ở 48/96/192/512px. Chúng **không phải**
ảnh app hay board thiết kế. Cả hai đã mở, thấy silhouette/ngòi/khoảng trong suốt đúng source;
không kỳ vọng chi tiết ngòi 16px tương đương 512px.

Cũng đã mở [320px chữ 200%](320-me-dark-text200.png) và
[editor dirty + overlay](1280-editor-dirty-overlay-dark.png). Trong vùng logo đã xem: không
tràn/cắt mark, không méo hai trục; rail thu ẩn wordmark như spec, mobile giữ wordmark.
Không sửa typography/trạng thái hoặc CTA ngoài logo.

## Giới hạn

**NOT_RUN:** launcher/PWA install thật, Android/iOS/CMP branding (không có consumer logo product
hiện hành), điện thoại thật, OS IME, screen reader, Safari/Firefox và zoom trình duyệt thật.
Root font 200% và CSS viewport không thay các kiểm tra đó. Chưa nghiệm thu bởi người dùng,
merge hay deploy. Board có hiệu ứng/hình khối khác SVG/PNG, được giữ làm tư liệu trình bày;
không gọi board và source production là pixel-match.
