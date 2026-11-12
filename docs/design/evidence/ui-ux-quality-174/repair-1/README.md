# Vòng sửa 1 — F-01 và capture dialog/sheet

Theo [báo cáo #174 §10](../../../ui-ux-quality-174.md#10-vòng-sửa-1-f-01-và-các-gap-bằng-chứng).
Ngày 12/11/2026 (UTC+07), base `5221871923ca2935988a225d827e1c4ed9c234c0`, nhánh
`codex/ui-ux-quality-docs-skills`, dirty checkout có source sửa. Dữ liệu mẫu, không có dịch vụ
hay bản thảo riêng. Đây là bằng chứng review, không phải golden/anchor được duyệt.

## Nguồn và cách chạy lại

- Web before: build bằng `cargo xtask ui-preview leptos run` trước sửa `feedback.rs`, chạy
  `preview-smoke.mjs --overlays-only 1` vào `target/ui-preview/174-before`. FAIL kỳ vọng ở
  6/16 ca. [Sidecar before](web-before/overlay-contexts.json) pin source SHA/fingerprint.
- Web after: [sidecar after](web-after/overlay-contexts.json) pin source cuối và cả 16 ca của
  full browser smoke chạy lại vào `target/ui-preview/174-full-rerun`. PNG chọn lọc giống nguyên
  byte với lượt `--overlays-only 1` vào `target/ui-preview/174-after-final` và pha overlay của
  lane đầu; lane đầu bị SIGTERM trong gallery, không có tổng kết PASS. Bảng dưới ghi lệnh
  build/browser và các kết quả riêng.
- CMP: `cargo xtask ui-preview cmp verify`; test `InteractionCaptureTest` trong runner hiện hữu.
  360×800, density 1, preview font multiplier 1/2, motion reduce, font TTF đã pin. Không có cửa
  sổ/bàn phím hệ điều hành, không thay Android/iOS.
- Lệnh đều từ repo root, trừ `./gradlew :preview:desktopTest --tests vink.ui.preview.InteractionCaptureTest`
  chạy tại `packages/ui-system/cmp`. Cách chạy Web riêng ở [development guide](../../../../development/README.md).

Không chỉnh viewport theo chiều cao document khi chụp Web. Root text scale không phải browser
zoom. Trigger focus rồi Enter qua CDP; Tab/Escape/outside dùng input event, không `element.click()`.
CMP dùng test input/semantics, ghi pointer exit và scroll offset. Overlay là ảnh phái sinh từ
chính PNG sạch cùng lượt, có hash và `derived_from`; không inject marker vào UI.

## Artifact và review

Sidecar giữ toàn ma trận; thư mục này chỉ lưu nhóm PNG chọn lọc để review. Full captures nằm ở
`target/ui-preview/174-full-rerun` và `packages/ui-system/cmp/preview/build/captures` của lượt local,
không suy rằng mọi file được sidecar nhắc tới đều đã được commit. `inspected=false` của runner
được giữ nguyên; [review.json](review.json) ghi **38/38 PNG lưu trong thư mục này đã mở**,
hash/kích thước và các đối chứng. Gồm 3 before Web, 16 clean + 2 overlay after Web,
16 clean + 1 overlay CMP. Không gọi 38 ảnh này là review toàn gallery.

Before/after khác đúng behavior focus và phần mở rộng runner; fixture, theme, viewport và cỡ
chữ giữ nguyên. Sửa selector của ảnh actions từ nút đầu sang nút cuối không ảnh hưởng ảnh
initial/INV-1 dùng để so F-01. Mã report/runner có hash riêng, không coi SHA base là source sạch.

| Vùng | Ảnh để đối chiếu |
| --- | --- |
| F-01 dialog 320×700, light, chữ 200% | [Trước](web-before/state.dialog--default--open--light--320x700--text200.png) · [Sau](web-after/state.dialog--default--open--light--320x700--text200.png) · [Overlay](web-after/state.dialog--default--open--light--320x700--text200--overlay.png) |
| F-01 sheet 320×700, light, chữ 200% | [Trước](web-before/state.dialog--sheet--open--light--320x700--text200.png) · [Sau](web-after/state.dialog--sheet--open--light--320x700--text200.png) |
| F-03 CMP dialog light, chữ 200% | [Ảnh trung tính sau pointer exit](cmp/interaction--default--light--360--text200.png) |
| F-04 CMP sheet light, chữ 200% | [Vừa mở](cmp/interaction--sheet--light--360--text200--initial.png) · [Tile 1](cmp/interaction--sheet--light--360--text200--part1.png) · [Tile 2](cmp/interaction--sheet--light--360--text200--part2.png) · [Tile 3](cmp/interaction--sheet--light--360--text200--part3.png) · [Overlay](cmp/interaction--sheet--light--360--text200--part1--overlay.png) |

[Sidecar CMP](cmp/interaction-contexts.json) có 28 ảnh sạch của 8 tổ hợp; mỗi ảnh có overlay
phái sinh. [Nguồn CMP](cmp/interaction-run.json) pin đầu vào. Nhóm sheet chữ 200% có ba tile ở
scroll offset 0/600/835px, viewport 800px; xóa tile giữa tạo khoảng trống 35px và bị phép kiểm
coverage phát hiện. Ảnh initial của cả tám tổ hợp đo được panel/title trong khung. Node
`field-label` được tìm qua accessible label nhưng bounds là field node; không phải dòng nhãn.

Đối chứng metadata và coverage dùng script reviewer một lần (Python stdlib: `json`, `struct`,
`hashlib`), không thêm runner mới: mọi PNG lưu có SHA-256/header khớp sidecar; bản sao đổi
height 800→640, đổi requested theme, đổi hash đều bị phát hiện. Test geometry trong runner
dùng số F-01 thật, thêm bounds sai/thiếu và overflow ngang. Trường hợp geometry đạt nhưng
visual chưa giải quyết là F-02 của after, giữ nguyên trong báo cáo.

## Kết quả kiểm chứng

| Lệnh / phạm vi | Kết quả |
| --- | --- |
| `cargo xtask verify quick` | PASS đủ 9 bước, bao gồm repository checks (links/skills/decisions/boundaries) |
| `cargo xtask ui-preview cmp verify` | PASS, Gradle 7m41s; `:preview` chạy lại 48 test, 0 fail/skip; `:ui` 43 và `:diagnostics` 6 là `UP-TO-DATE`, không nhận là chạy lại trong lượt này |
| `node --test .../focus-capture.test.mjs .../overlay-capture.test.mjs` | PASS 6/6 |
| `preview-smoke.mjs … --overlays-only 1` | Before: FAIL kỳ vọng 6/16; after cuối: PASS 16/16, 32 ảnh sạch + 32 overlay |
| `VINK_CHROME_NO_SANDBOX=1 VINK_UI_FOCUS_CAPTURE=1 cargo xtask ui-preview leptos verify` | Build preview/media hydration và unit geometry PASS; tiến trình bị SIGTERM trong gallery, không có tổng kết lane PASS |
| Full browser smoke chạy lại trên bundle đó | PASS, exit 0: 660 assertion, 846 capture records / 1.297 PNG (gồm ảnh phái sinh), focused capture 36/36, không lỗi console; lệnh ở dưới |
| `git diff --check` và integrity/negative controls trong `review.json` | PASS |

Lệnh chạy lại toàn bộ phần Chrome (không filter bỏ scenario/capture):

```sh
VINK_CHROME_NO_SANDBOX=1 node packages/ui-system/leptos/preview/e2e/preview-smoke.mjs \
  --base http://127.0.0.1:3019/ --chrome /usr/bin/google-chrome \
  --out target/ui-preview/174-full-rerun \
  --catalog packages/ui-system/shared/preview-catalog.json --focus 1
```

Lần browser chạy lại đầu tiên FAIL ở `preview mounted` sau 15 giây, chưa tới ma trận UI.
Runner sau đó cho **bootstrap** tối đa 60 giây vì bundle debug lớn (WASM khoảng 100 MB),
giữ timeout assertion tương tác cũ. Đây là giới hạn của test runner, không là SLA của sản phẩm.
Lỗi runner CMP lúc phát triển (`exit` khi con trỏ chưa hover) đã được sửa bằng move→exit,
và lượt CMP đầy đủ ở trên đạt sau sửa. Các FAIL này không đổi thành NOT_RUN.

Before FAIL được giữ như đối chứng; không gọi việc có PNG hay metadata là visual PASS.
Native/OS IME/screen reader: NOT_RUN trong lượt này. `verify web` của app, PostgreSQL và model
không chạy: không sửa page/API/store/domain. F-02 và lựa chọn UX F-04 vẫn mở theo báo cáo.
