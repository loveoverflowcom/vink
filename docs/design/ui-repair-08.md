# UI-REPAIR-08 — Font CMP Desktop trên Linux

Baseline: `31a62b1dd7a20e445d9788b967613902f7dc1b91` (`origin/develop`, PR #49 đã merge).
Phạm vi: Be Vietnam Pro 400/600 và Literata variable cho preview Desktop; Web giữ WOFF2,
Android/iOS giữ font hệ thống. Không đổi token, anatomy, typography role hoặc native product.

## Xác nhận finding và nguyên nhân

| Finding | Source / cách tái hiện | Kết luận |
| --- | --- | --- |
| F08-01: Linux không nạp đủ ba face | `PreviewFonts.load`, `PreviewTest.selfHostedFacesDecodeOnDesktop`; `cargo xtask ui-preview cmp verify` trên Linux/JDK 17 | CONFIRMED: baseline 33 tests, 1 FAIL, đúng assertion ba face; không bỏ test hoặc đổi kỳ vọng |
| F08-02: resource sai/thiếu hoặc dữ liệu hỏng? | `syncPreviewFonts`, classpath `vink-fonts/`, byte length/SHA-256 so với provenance; fontTools 4.68.0 giải nén và so bảng | REJECTED trong lần tái hiện: đủ resource, đúng hash, font dữ liệu hợp lệ |
| F08-03: decoder của runtime pin | Skiko 0.9.22.2 `FontMgr.default.makeFromData`, cùng JDK/host/byte nguồn, WOFF2 so với TTF | CONFIRMED trên Linux: WOFF2 trả `null` cả ba; TTF đọc đủ family/weight/trục/glyph. Không suy ra WOFF2 luôn hỏng ở mọi OS |

[CI lịch sử PR #49](https://github.com/loveoverflowcom/vink/actions/runs/41623793555/job/125875933640)
chạy synthetic merge `ed8abe181db4d570388ca2b5c9263094e7a7e01d`, head `58ab5be…`,
base `69057433…`; không phải lần chạy trên develop `31a62b1`. Lần local mới tái hiện trên
Ubuntu 26.04.1 x64, kernel 7.0.0-42, Temurin JDK 17.0.20.1; CI Linux dùng Ubuntu 24.04.
Kết quả macOS cũ trong PR #48 là lịch sử, không thay gate của HEAD mới.

## Bản sửa và LayoutSpec

```text
Region #font-host — Desktop inject VinkFonts vào VinkTheme
  Region #typography — role UI 400/600, đọc Literata 400/600; tiếng Việt NFC/NFD, Đ/đ
  Region #byline — tên dài wrap, metadata theo sau
  Region #reader — title, byline, đoạn/khổ thơ giữ thứ tự
Before → After: WOFF2 bị decoder Linux từ chối → TTF từ cùng dữ liệu được nạp thật.
Narrow/theme/state: cùng specimen rộng 390dp, density 1, OS scale 1, text 100/200%, light/dark.
Check: family/weight/trục/glyph, đo chữ Compose, missing/corrupt, mở ảnh cùng fixture.
```

Spec đã xuất trước patch. Không đổi cỡ chữ/line-height/padding để nhét nội dung.
`derive_desktop.py` giải nén lossless bằng fontTools 4.68.0 + brotli 1.2.1; mọi bảng ngoài
`head` giống byte, mọi field `head` ngoài checksum giữ nguyên. Không instance/subset lại.
[Provenance duy nhất](../../apps/vink-web/assets/fonts/provenance.json) giữ commit Google Fonts,
hash nguồn WOFF2, hash/length derivative, tool và lệnh; OFL vẫn đi kèm trong classpath.
Ba TTF có tổng 844 748 bytes; build/runtime dùng file đã commit, không tải CDN, không cần Python.

Loader vẫn fallback toàn bộ khi một face thiếu/hỏng và nói rõ tên/lý do; `loaded=[]` chỉ cho
fallback. Trường hợp bình thường vẫn yêu cầu đủ ba face. Probe native Skia được đóng sau khi
kiểm tra, font byte được Compose tiêu thụ qua host hiện có.

## Kiểm chứng

| Boundary | Kết quả hiện có |
| --- | --- |
| Baseline Linux `cargo xtask ui-preview cmp verify` | FAIL đúng `selfHostedFacesDecodeOnDesktop`, 33 tests / 1 failure |
| Reproduce derivative + `derive_desktop.py --check` | PASS, cùng bytes/hash; outlines/metrics/layout/name/variation giữ nguyên |
| Focused `FontTest` + font test cũ + `FontCaptureTest` | PASS trên Linux: đủ 3 face, 154/154 điểm mã tiếng Việt NFC/NFD mỗi face; Literata opsz 7–72/wght 200–900; Compose 400/600 khớp metrics font thật |
| Missing/corrupt mỗi face | PASS, 6 case đều fallback toàn bộ với lý do đúng |
| Negative control: giả `loaded=3` nhưng trả font hệ thống | PASS: test đo chữ FAIL đúng (94.848206 thay vì 90.7522px cho Be Vietnam Pro 400), sau đó khôi phục loader thật; không là production code |
| `cargo xtask verify quick` | PASS; fmt, clippy SSR/WASM/preview, doctest, unit/contract, repo checks; gồm guard derivative và ca hash bị sửa cùng độ dài/header |
| `cargo xtask ui-preview cmp verify`, bản sửa cuối | PASS Linux local: common + Desktop; preview 37 tests, 0 failures/skips; 447 PNG gồm 24 đối chiếu font |
| Linux CI `ui-cmp`, code head `f051599…` | PASS [job](https://github.com/loveoverflowcom/vink/actions/runs/41640577776/job/125932763636), Ubuntu 24.04/x64, Temurin 17.0.21; đủ ba TTF, WOFF2 trả `null` |
| macOS CI `ui-cmp-macos`, code head `f051599…` | PASS [job](https://github.com/loveoverflowcom/vink/actions/runs/41640577776/job/125932763191), macOS 15.8.2/arm64, Temurin 17.0.21; TTF và WOFF2 đều đọc đủ ba face |
| Preview Leptos / Web ở `f051599…` | FAIL Web tại `clear query` của Bàn viết; Leptos NOT_RUN sau đó. Đây là lịch sử lượt đầu, không phải kết quả cuối; Web font bytes/token/CSS không đổi |

CI run 71 checkout synthetic merge `6ba1f59d16f9eb9c0941706ecd76ed85daa5f854`, từ PR head
`f0515995d27478f4f4761720a1a5cbf2334fcacd` + base `31a62b1…`. Hai job cùng build fingerprint
`6e9ee4d453c965e7e60704c2650be40a6d80da7abedfdfac7e97424997944b14` với local, mỗi job
đạt 37 tests preview và 2 diagnostics replay trong XML rút gọn, cùng common tests của lane.

Kết quả **final PR head**, tested SHA và run/job cuối được pin trong
[PR #51](https://github.com/loveoverflowcom/vink/pull/51) và
[checks hiện hành](https://github.com/loveoverflowcom/vink/pull/51/checks).
Chỉ merge khi Linux font gate, macOS regression, quick và Web/Leptos của đúng head đạt;
không biến lịch sử PASS trên `f051599…` thành PASS cho head khác.

Lượt focused đầu FAIL ở oracle đo: Skia Font mặc định snap advance theo pixel, Compose paragraph
dùng advance phân số. Test đã so cùng linear/subpixel metrics với hinting NONE, không nới kỳ vọng
về font hoặc số face. Self-review bỏ giả định font mặc định của host luôn có metrics khác:
oracle cuối yêu cầu `FontListFontFamily` từ asset và vẫn so phép đo Compose với font bytes.
[Ca âm `loaded=3` + generic family và lần khôi phục](evidence/ui-repair-08/portable-family-oracle.xml)
đều kiểm đúng; ba FontTest cuối đạt trên Linux. Giữ XML/log thất bại và lần đạt trong evidence. Glyph ID tồn tại và phép đo
không tự chứng minh shaping đầy đủ, OS IME hoặc accessibility trên thiết bị.

[Web CI lượt đầu](https://github.com/loveoverflowcom/vink/actions/runs/41640577776/job/125932763681)
ở head `f051599…` timeout chờ 100 kết quả sau click xóa truy vấn. Artifact cho thấy vẫn giữ query
và panel no-results sau vòng capture đổi viewport/font scale; driver lấy tọa độ ngay sau scroll/
restore, nên race reflow là giả thuyết cần kiểm, chưa phải nguyên nhân đã xác nhận. Không thay Web
product, không bỏ assertion và không dùng PASS font để bỏ qua lỗi này; cần kết quả Web/Leptos
đạt trên final head trước merge.

## Ảnh và phạm vi review

`FontCaptureTest` giữ 24 PNG và `font-contexts.json`: typography/combining-marks,
author-byline/long-label, reader/long × light/dark × text100/200 × trước/sau. `before-woff2`
chạy lại **byte WOFF2 gốc qua cùng seam decoder/construction**, không phải ảnh cũ gán HEAD mới.
Linux ghi rõ `loadedFaceCount=0`, system fallback; `after-ttf` ghi 3 face. Capture baseline
nguyên bản của lượt FAIL cũng được giữ riêng để đối chiếu lịch sử. Lịch sử FAIL nằm trong
[XML rút gọn](evidence/ui-repair-08/local-history.xml), [log baseline](evidence/ui-repair-08/baseline-linux-log.txt)
và [probe decoder](evidence/ui-repair-08/decoder-probe.txt); kết quả mới nằm ở
[check-results](evidence/ui-repair-08/check-results.txt), [XML cuối](evidence/ui-repair-08/linux-final-tests.xml)
và [diagnostics family/weight/glyph/hash](evidence/ui-repair-08/linux-font-diagnostics.txt).

| Linux local, cùng fixture/390dp/theme/scale | Trước WOFF2 | Sau TTF |
| --- | --- | --- |
| Typography NFC/NFD, light/text100 | [Ảnh](evidence/ui-repair-08/linux-typography-before.png) | [Ảnh](evidence/ui-repair-08/linux-typography-after.png) |
| Byline tên dài, light/text200 | [Ảnh](evidence/ui-repair-08/linux-byline-before.png) | [Ảnh](evidence/ui-repair-08/linux-byline-after.png) |
| Reader thơ, dark/text200 | [Ảnh](evidence/ui-repair-08/linux-reader-before.png) | [Ảnh](evidence/ui-repair-08/linux-reader-after.png) |

[Manifest ảnh đại diện + hash source](evidence/ui-repair-08/linux-manifest.json) và
[24 context gốc](evidence/ui-repair-08/linux-font-contexts.json) giữ fingerprint đúng lượt local.
Đã mở cả sáu raw đại diện sau lượt cuối; các vùng có nội dung khớp focused capture đã inspect.

Đã mở 24 ảnh focused Linux qua sáu contact sheet crop toàn bộ vùng có nội dung (không đổi
tỷ lệ), cùng raw byline đại diện. Title dài ở 390dp/text100 chuyển từ 9 dòng fallback thành
8 dòng Literata; byline vẫn giữ đủ tên và wrap ở text200, reader giữ line break và khoảng khổ.
Đây là delta metrics có chủ ý khi font đúng thay fallback, không phải thay layout/token.
Dấu tiếng Việt NFC/NFD không bị cắt trong các mẫu đã mở; không khái quát thành toàn bộ shaping.

Sidecar giữ source SHA, fingerprint bytecode/font của build dirty, JVM/OS, renderer, density,
scale, input files và font thực. Ảnh là **CMP JVM off-screen**, nội dung tổng hợp; không phải
app native, browser hay mockup.

| macOS CI, cùng fixture/390dp/theme/scale | Trước WOFF2 | Sau TTF |
| --- | --- | --- |
| Typography NFC/NFD, light/text100 | [Ảnh](evidence/ui-repair-08/macos-typography-before.png) | [Ảnh](evidence/ui-repair-08/macos-typography-after.png) |
| Byline tên dài, light/text200 | [Ảnh](evidence/ui-repair-08/macos-byline-before.png) | [Ảnh](evidence/ui-repair-08/macos-byline-after.png) |
| Reader thơ, dark/text200 | [Ảnh](evidence/ui-repair-08/macos-reader-before.png) | [Ảnh](evidence/ui-repair-08/macos-reader-after.png) |

Đã mở 24 ảnh macOS qua sáu contact sheet crop toàn bộ vùng có nội dung ở tỷ lệ gốc.
12/12 cặp trước/sau trên **cùng host macOS** trùng byte PNG; không có delta metrics/wrapping
quan sát được trong matrix này. Không suy pixel parity giữa macOS và Linux. Theo
[manifest macOS](evidence/ui-repair-08/macos-manifest.json), cả trước và sau đều dùng đủ ba
face thật; nguồn/hash/family/weight/trục/glyph ghi trong
[diagnostics macOS](evidence/ui-repair-08/ci71-macos-font-diagnostics.txt) và
[diagnostics Linux CI](evidence/ui-repair-08/ci71-linux-font-diagnostics.txt).
[XML Linux](evidence/ui-repair-08/ci71-linux-tests.xml),
[XML macOS](evidence/ui-repair-08/ci71-macos-tests.xml) và
[context macOS gốc](evidence/ui-repair-08/ci71-macos-font-contexts.json) giữ đúng run/checkout.

Self-review diff/caller/oracle và ảnh, gồm negative control; chưa có independent approval. Không nghiệm thu
Android/iOS branded fonts, OS IME, screen reader hoặc pixel parity giữa các rasterizer.
