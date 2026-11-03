# EMO-01A — Google Noto, policy Rust và Web pilot

**Phạm vi:** lát cắt độc lập của [#25](https://github.com/loveoverflowcom/vink/issues/25), nền
`develop@27927c462244f2a70e36a686a37c8c95690775b6`. Chưa hoàn thành toàn bộ issue.
[Development guide](README.md#biểu-cảm-google-noto-emo-01a) sở hữu lệnh; PR ghi SHA và kết quả chạy.

## HOW và WHY

```text
Google SVG (commit pin) + animation (snapshot SHA-256)
  → packages/expressions/shared/catalog.json
  → validation offline + generated Rust / Kotlin / Unicode vectors
  → static Leptos SSR / original Unicode
  → user activation + host facts → production Rust Policy<16>
  → Web effects: dynamic import dotLottie Software + WASM + trusted JSON
  → callback generation check → one-shot → release/destroy
```

**Đã ghi nhận trong #25:** giữ văn học Unicode nguyên bản, chuyển động chỉ khi tương tác,
manifest dùng chung và không dựng authority thảo luận thứ hai. [Registry #41](discussion-core.md)
giữ D2T-INV; D2T-INV-11 dẫn tới registry EMO dưới đây, không copy luật bình luận.

Chọn crate thuần `vink-expressions` vì Web WASM thực sự gọi policy và Kani biên dịch cùng source;
đặt trong service sẽ kéo sai dependency vào browser. Generated contract CMP dùng lại manifest,
không nhận là Rust FFI/Kani proof của Kotlin. Thêm service/DB không giải quyết việc phát asset
chỉ đọc; không chọn phương án đó. Engine JS quản lý DOM/network, không giữ policy thứ hai. Host đọc lại media/foreground và
geometry đồng bộ trước activation và callback load; không chỉ dựa vào notification có thể đến muộn.

Software là backend tham chiếu của pilot, **chưa có bằng chứng nhanh nhất**. Worker/WebGL2/WebGPU
không được ship trong bundle. Full benchmark/thiết bị và quyết định backend cuối vẫn thuộc #25.

## LayoutSpec r1 — hướng đã chọn trước khi viết UI

Trang có nhãn Mẫu, route `/thu-nghiem/bieu-cam`, dùng shell có sẵn. Người thử xem artwork và
chủ động phát một lần; không có submit, recent, picker sản phẩm hoặc trạng thái persisted.

```text
Screen #expression-pilot — Một chút biểu cảm · Mẫu
  Region #pilot-heading — mục đích, giới hạn, link về Khám phá
  Region #unicode-sample — thơ Việt giữ nguyên FE0F/ZWJ/modifier/flag/keycap
  Grid #emoji-catalog — 16 poster; tên/nút shared Button; trạng thái yên tĩnh
  Region #expression-status — thông báo thao tác, không announce từng frame
  Region #runtime-diagnostics — disclosure budget/backend của thử nghiệm
  Region .vk-expression-attribution — Google, license, NOTICE truy cập được
```

Đọc/focus theo DOM. Grid auto-fit theo phần rộng còn lại, cell tối thiểu 11rem nhưng không vượt
100% ở màn hẹp; artwork 6rem giữ chỗ trước hydration. Trang cuộn chính, không overlay/cuộn lồng. Gutter ngang do `.vk-main` sở hữu, pilot chỉ thêm
padding theo trục dọc để tránh padding lồng ở chữ 200%.
Token surface/container/on-surface/outline/primary/focus và font của UI system; không thêm palette.
320/390/834/1024/1440px, Sáng/Tối/Hệ thống, 200% chữ; nút native được Enter/Space/touch kích hoạt,
status gắn `aria-describedby`. Root `noindex`; không đưa diagnostics vào reader/feed.

| Trạng thái/fact | Giao diện | Hành động/tài nguyên |
| --- | --- | --- |
| SSR/no JS/static | Poster có alt; văn bản nguyên Unicode | Không engine/canvas; kích thước ổn định |
| Activation hợp lệ, còn slot | Poster + “Đang tải…” | Giữ slot trước async import/fetch |
| Callback đúng generation | Canvas trang trí, tên nút giữ nguyên | Phát một lần; không loop |
| Đủ 2 slot | Poster + giải thích thử lại | Không request mới, không queue autoplay |
| Giảm chuyển động/hidden/offscreen | Poster hoặc Unicode | Hủy/pause rồi destroy; thu hồi slot; trở lại không autoplay |
| Module/WASM/JSON lỗi hoặc timeout | Poster + thử lại | Release và destroy; không đi CDN khác |
| Poster lỗi | Unicode của đúng sequence | Tên/nội dung vẫn đọc được |
| Route unmount/dispose | View được dọn | Abort fetch; observers/listeners/timers/canvas/player bị dọn; callback muộn bị chặn |

## Catalog, licensing và loader

Manifest pin 16 sequence chính, alias lookup chỉ liệt kê tường minh. Lookup là whole-string,
không strip FE0F/ZWJ/modifier hay cắt codepoint đầu. Flags/keycaps/family/unknown ngoài pilot trả
Unicode OS; không nhận đó là artwork Google. Không đổi `DocumentDto`, CodeMirror, manuscript,
undo/history hoặc API. Asset reference trong catalog không phải schema sticker/reaction #43.

Nguồn tĩnh có Git revision; animation có content-addressed snapshot, **không bịa upstream Git SHA**.
`generate.mjs` kiểm hash/size, trùng ID/Unicode, nguồn/license/attribution, resource path có hash;
SVG không có script/event/DTD/entity/external URL; Lottie vector-only không có ảnh/font/expression.
JSON tối đa 512KB, SVG 64KB, canvas tối đa 192×192; không ingest `.lottie` archive trong pilot này.
Nhãn Việt do Vink viết; artwork không thay đổi. `.gitattributes` giữ nguyên whitespace của
SVG upstream, còn validator vẫn kiểm byte theo SHA-256. NOTICE + từng license được self-host.

`@lottiefiles/dotlottie-web@0.82.1`, npm integrity và JS/WASM SHA-256 được pin; source revision
`2f3693a1d5b9d6936027bc892fa4f9d1cd16d2bb`. Build chỉ export `DotLottie`, thay hai default WASM
CDN URL bằng local hash URL, gọi `setWasmUrl` trước khởi tạo. Không worker/blob worker, external
font hay arbitrary user URL. Hỏng WASM chỉ retry buffered cùng origin. Build và browser kiểm
fallback không thoát sang CDN. `build.json` ghi byte thô; đây không phải benchmark transfer/FPS.

Tiny wasm-bindgen bridge được ship cùng app, controller chỉ tải trên route pilot; engine/WASM
chỉ tải sau admission từ activation qua event người dùng tin cậy (`isTrusted`). Không tuyên bố toàn feature +0KB: Rust/catalog/glue/CSS
vẫn có overhead. Reader/Explore không yêu cầu engine, WASM hoặc controller route.

## Registry EMO — một model lifecycle

| ID | Quy tắc và production seam | Model / bằng chứng phù hợp |
| --- | --- | --- |
| EMO-INV-01 | `Policy::activate`, `set_environment`, `set_visible`: không play nếu static/unsupported/reduced/hidden/offscreen | `eligiblePlayback`, Rust fact matrix, browser host facts/network |
| EMO-INV-02 | Một mutable owner; active tính từ Loading/Playing; reserve/release idempotent, 0..capacity ≤4 | `budgetSafe`, budget competition, Kani 3 player/cap 0..3/6 event, browser cap 2 |
| EMO-INV-03 | `ready/release/fail` ràng generation + phase; không wrap u32 | `callbackFence`, trace replay, Kani, stale callbacks; code test u32::MAX |
| EMO-INV-04 | `dispose` terminal, facts phục hồi không tự start | `terminal`/`activationRequired`, negative control, Kani, browser unmount |
| EMO-INV-05 | `catalog::lookup/fallback`: nguyên source/alt; lỗi không có authority thảo luận | Shared Unicode vectors, bounded Kani lookup 4 input, manifest safety tests, SSR/browser |

Model canonical tại [lifecycle.qnt](../../packages/expressions/behavior/lifecycle.qnt): ba player,
capacity 2, tới ba activation/player, 40 transition/sample; environment là facts tổng hợp. Init
mô hình là host đã ready; constructor Rust mặc định fail-closed cho tới khi adapter cấp facts.
Simulation TypeScript seed 25, 1000 samples, scenario 33 bước bao phủ pha/outcome; không gọi là
exhaustive model checking hoặc liveness proof. Expected states lấy từ Quint ITF; Rust replay
**input** qua production functions. Mutation bỏ callback guard tạo counterexample ba bước
activate → dispose → ready; production từ chối bước cuối. Không sửa oracle để khớp implementation.

Kani 0.67.0 dùng compiler bundled `nightly-2025-11-21`; manifest isolated trỏ cùng `src/lib.rs`,
không hạ toolchain workspace. Harness budget 6 event/3 player/cap 0..3, generation callback u32
bất kỳ, unwind 12; guard harness unwind 8; lookup bốn chuỗi đầy đủ unwind 40. Có cover full budget
và disposed. Không assume sẵn invariant, không tắt unwinding assertions. Không chứng minh Unicode
parser tổng quát, DOM, network, ThorVG, browser GC, native Kotlin, SQL hay hiệu năng bằng Kani.

## Phạm vi chưa nghiệm thu

- #45: picker/popover/sheet/composer, focus return và IME của tích hợp sản phẩm; không phải pilot.
- #43: typed sticker references, desired-state reactions, counts, auth và persistence.
- Native: chỉ generated contract/Unicode conformance trong `:ui`; không dotLottie native, không
  native product app, không FFI. Android/iOS runtime, TalkBack/VoiceOver, native lifecycle NOT_RUN.
- Benchmark: so Software/Worker/WebGL2, thiết bị mobile có tên, cold/warm/compressed transfer,
  latency percentiles, CPU/GPU/frame time và heap/worker memory sau dispose vẫn NOT_RUN.
- Browser có ca inject host fact mới mà chưa phát notification để kiểm callback load; ca ẩn tab
riêng dùng visibility thật của Chrome. Browser smoke kiểm DOM/resource teardown, không chứng minh heap GC; OS IME, screen reader và
  đa browser/device thật cần phiên riêng. Không có production CSP mới trong lát này.

Không dùng “Closes #25”. Kết quả lệnh, SHA, ảnh đã mở và giới hạn từng lớp phải được ghi ở PR;
source/test hiện diện tự nó không phải PASS.
