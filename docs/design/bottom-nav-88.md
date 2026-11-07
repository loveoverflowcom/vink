# Bottom navigation: căn icon và dòng nhãn đầu (#88)

Ngày kiểm: 07/11/2026. Phạm vi Web, [issue #88](https://github.com/loveoverflowcom/vink/issues/88).

## Baseline và quyết định

Baseline sạch `develop@4348b1d7bbf97641d3db5901f6d93970c069749b` (SSR/hydrate dev),
Chrome headless 156.0.8078.80 trên macOS arm64, DPR 1, chiều cao 900 CSS px.
PostgreSQL tạm riêng và LocalFixture mẫu; không dùng database sản phẩm.
Root text 100/200% tương ứng 16/32px; font Be Vietnam Pro tự host đã tải.

Ảnh c017 trong issue thuộc `eb41d4b8 + dirty inputs`; đó không phải baseline hiện tại.
Tái hiện trên baseline mới: 320px, `/toi`, dark, chữ200, ba target cao 120,78125px;
icon-centreY là 791,21875 / 791,21875 / 809,40625;
dòng đầu DOM Range là 812,21875 / 812,21875 / 830,40625.
Hai trục đều lệch **18,1875 CSS px**. Regression mới FAIL vì chính độ lệch này.

Đã mở ảnh c017 và [board R2](https://drive.google.com/file/d/1qRJwdxEvnkLYDjhnPdjO5ekrEr1dWywi/view),
chỉ tham chiếu bottom bar. Board là thiết kế, không phải runtime.

CHOICE-01 chọn **B: flex căn trên**, giữ DOM hiện có. Parent grid vốn đã kéo ba target
cao bằng nhãn dài nhất; đổi `justify-content` từ `center` sang `flex-start` đủ để
chia sẻ trục icon/dòng đầu. Không cần subgrid, JavaScript đo nhãn hoặc chiều cao font
cố định. Chiều cao bar và khoảng tránh che nội dung vẫn do ResizeObserver hiện có đo.

LayoutSpec rút gọn đã trình bày trước patch:

```text
Region #bottom-nav — ba target bằng nhau, giữ safe-area
  Column #explore — icon → gap token → nhãn Khám phá
  Column #desk — icon → gap token → nhãn Bàn viết
  Column #me — icon → gap token → nhãn Tôi
Before → After: căn giữa từng cụm cao khác nhau → cùng inset trên cho cả ba cụm.
Narrow/text200: nhãn wrap đầy đủ, thanh cao theo nội dung dài nhất.
Theme/state: cùng hình học sáng/tối, selected/focus; giữ thứ tự/link/route semantics.
Check: rect icon-centreY + Range dòng đầu ≤1px; target đều, hit/fit/clearance,
       rail >=760px, editor hide_bar, cùng scenario trước/sau.
```

## Bằng chứng local

- Regression trước patch: **FAIL đúng lỗi**, tại `nav-toi-320-200-dark`.
- Phase `bottom-navigation` sau patch: **PASS** 24 ca — 320/390 × chữ100/200 × light/dark
  × `/toi`, `/`, `/ban-viet`; kiểm cả trước/sau focus bằng Tab. Icon/dòng đầu
  lệch 0px; sai số bề rộng target tối đa do làm tròn subpixel <0,02px.
- Bar 320/chữ200 vẫn cao 136,78125px; các target vẫn cao 120,78125px.
  Cả ba icon-centreY sau sửa là 791,21875; dòng đầu là 812,21875.
- Giữ assertion nhãn đầy đủ/tối đa hai dòng, hit ≥44px, fit trong bar, chiều cao bar
  ≤18% viewport, biến chiều cao khớp bar và padding tránh che nội dung.
- Kiểm resize/font-size sau hydration, cả ba route selected, focus không dịch hình học;
  834px vẫn dùng rail; editor mobile vẫn ẩn bar; không có console error/exception.
- Đã mở 8 ảnh `/toi` sau sửa (hai width × hai text scale × hai theme), ảnh rail 834px,
  và cặp baseline/sau sửa 320 dark chữ200. Đã mở specimen gallery
  `navigation.scaffold/adaptive`, light/390/chữ200 (focus tile cuối). Phần bottom bar khớp LayoutSpec;
  phần khác của trang thuộc issue #66/#67, không mở rộng scope tại đây.

[Ảnh baseline](evidence/bottom-nav-88/before-320-toi-dark-text200.png) ·
[Ảnh sau sửa](evidence/bottom-nav-88/after-320-toi-dark-text200.png) ·
[Context baseline](evidence/bottom-nav-88/before-contexts.json) ·
[Context sau sửa](evidence/bottom-nav-88/after-contexts.json).
Context giữ SHA baseline; CSS và regression là dirty inputs trong lần sau sửa;
commit chứa tài liệu này pin chính xác patch, không gọi ảnh đó là clean build SHA mới.

Lệnh từ repo root, thêm cấu hình local:

```sh
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0
export npm_config_cache=/tmp/vink88-npm
export VINK_CHROME='/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'
export VINK_TEST_DATABASE_URL=postgres://vink88@127.0.0.1:55488/postgres
cargo xtask verify quick
cargo xtask verify web
cargo xtask ui-preview leptos verify
node apps/vink-web/e2e/browser-smoke.mjs --base http://127.0.0.1:3048 \
  --chrome "$VINK_CHROME" --state /tmp/vink88-after/state.json --phase bottom-navigation
```

Kết quả cuối: `verify quick` **PASS**; `verify web` **PASS** (SSR/hydration,
CodeMirror, save/reload/conflict/leave guard, SIGTERM/restart/export và endorsement thật);
`ui-preview leptos verify` **PASS** (scenarios và captures); phase `bottom-navigation`
**PASS** 24 ca. Source đã review, `git diff --check`, `node --check` và JSON parse **PASS**.
Chỉ dùng local checks;
không đọc/chờ GitHub CI và không deploy.

Giới hạn: một browser desktop headless với viewport mobile giả lập; chưa screen reader,
thiết bị mobile/IME OS, safe-area khác0, Safari/Firefox hoặc native/CMP runtime.
Các biểu thức `env(safe-area-inset-*)` giữ nguyên; không suy native PASS từ Web.
Thử CDP `Emulation.setSafeAreaInsets` với bottom24 trả lỗi method-not-found trên Chrome
này: runner thử nghiệm **FAIL**, phép giả lập inset khác0 **UNSUPPORTED**, không đổi thành PASS.
Preflight ban đầu FAIL vì npm cache dùng chung EACCES và Chrome không nằm trong PATH;
đã dùng npm cache riêng `/tmp/vink88-npm` và `VINK_CHROME` tường minh. Lần Web đầu có
FAIL của oracle mới khi đo bar đang ẩn ở desktop; đã thêm visibility guard như helper
geometry hiện có, rồi chạy lại lane đầy đủ. Không coi những lần lỗi đó là PASS.
