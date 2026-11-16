# Baseline hiệu năng Web & Request-Reason Ledger (#234)

Báo cáo baseline tái lập cho SSR, hydration, hành trình Khám phá → Reader → Back và các cơ chế freshness/lifecycle trên bundle `release`. Đây là tài liệu đo đạc và ra quyết định thử nghiệm cho [#235](https://github.com/loveoverflowcom/vink/issues/235)–[#239](https://github.com/loveoverflowcom/vink/issues/239); **không sửa bất kỳ chính sách cache, SSR mode, schema DB hay code runtime nào** trong [`apps/vink-web/src`](../../apps/vink-web/src/lib.rs) và [`services/vink-app/src`](../../services/vink-app/src/lib.rs).

**Bốn chiều trạng thái (tách biệt độc lập):**
- **Quyết định sản phẩm:** giữ nguyên các trạng thái hiện hành (`READ-02` max display age `OPEN`; `PUB-07` & `O-17@r2` `PRODUCT_CONFIRMED`). Không đặt SLA production mới.
- **Quyết định kiến trúc:** giữ `DESIGN_PROPOSED` cho `READ-02-CHOICE-01..04`; ghi nhận khuyến nghị điều kiện `#234` (`CHOICE-01 A` mở rộng `xtask` + `browser-smoke`, `CHOICE-02 A` phân phối + nhiễu + negative controls trước gate thời gian).
- **Mức triển khai (`implementation level`):** chỉ thêm kịch bản opt-in `perf-baseline` trong harness (`xtask`, `services/vink-app/tests/support/perf_journey.rs`, `services/vink-app/examples/perf_journey_corpus.rs`, `apps/vink-web/e2e/perf-baseline.mjs`); runtime sản phẩm và schema giữ nguyên.
- **Bằng chứng kiểm chứng (`verification evidence`):** chạy thực tế bằng `cargo xtask verify web --scenario perf-baseline --release` trên PostgreSQL 17.11 scratch và Headless Chrome `157.0.8103.78`, kèm unit tests thuần `perf-baseline.test.mjs`, `document_budget` và `verify quick`. Kết quả thô lưu ở [evidence/web-perf-234](evidence/web-perf-234/README.md).

## 1. Nguồn dữ liệu, môi trường và tái lập (AC-01)

- **Source pin tại lần đo:** base `develop` `ca9369653a516715fea95ca71fbd818d54c15b0a`, `sourceFingerprint = 9dd680a4e3d280e0ea9fee5979f6cf682a3ce3add06bf19307a096eb36633d09` (`dirty: true` chứa code harness `#234` trước commit tài liệu).
- **Corpus hành trình (`seed = 234`):** [perf_journey.rs](../../services/vink-app/tests/support/perf_journey.rs) tái sử dụng generator [#229](long-document-budget.md) (`seed = 229`) qua validator/serializer/hash thật (`Title::parse`, `Document::parse`, `ContentHash::of`, `CommentBody::parse`). Tổng cộng **200 tác phẩm / 1.000 bình luận** đã duyệt (`corpus_digest = e3bb862747f1be49e553e425a0a2516793c729dbebdac9b1af6dfaa8c3adbc9c`), gồm work chuẩn (`standard`, 2,4k scalar, 120 bình luận / 60 gốc 2 tầng), `long_poem_200k` (200k scalar, 50 bình luận), `long_vi_200k` (200k), `medium_emoji_30k` (30k), `withdrawal_target` (có working draft riêng tư), 192 works pool và 3 works không có bình luận.
- **Giới hạn phân trang thật được giữ nguyên:** `discovery` mặc định `12` (tối đa `24`); `discussion` mặc định `20` gốc/trang (tối đa `50`, preview `3` phản hồi mỗi gốc). Không trang nào mount toàn bộ 200 works hay 120 comments cùng lúc.
- **Host & dịch vụ:** `macOS darwin 24.6.0` (`Apple M4`, 10 cores), `rustc 1.99.1`, `Node v22.15.0`, `PostgreSQL 17.11` (`shared_buffers = 128MB`), `Headless Chrome 157.0.8103.78`, mạng `loopback_unthrottled`, `cpu_throttling_rate = 1`, `text_scale_pct = 100` (`16px`), font tự host `Be Vietnam Pro` và `Literata`.
- **Cấu hình viewport:**
  - `desktop_1440_dpr1`: `1440×900`, `DPR=1`, `mobile=false` (có mẫu 200k Reader).
  - `mobile_390_dpr2`: `390×844`, `DPR=2`, `mobile=true`.

```sh
# 1) Kiểm tra plan (không chạy side-effect)
cargo xtask verify web --scenario main --plan
cargo xtask verify web --scenario discovery --plan
cargo xtask verify web --scenario perf-baseline --release --plan

# 2) Chạy baseline release với PostgreSQL scratch riêng + Chrome
export VINK_TEST_DATABASE_URL="postgres://manhblue@127.0.0.1:55534/postgres"
export VINK_CHROME="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
cargo xtask verify web --scenario perf-baseline --release
```

### Kích thước bundle `release` (`target/site/pkg`)

| Tệp | Raw bytes | Gzip-9 bytes | Ghi chú |
| --- | ---: | ---: | --- |
| `vink.js` | 86.715 | 12.844 | JS glue của `wasm-bindgen` |
| `vink_bg.wasm` | 111.995.435 | 4.590.181 | **`custom:name` = 106.802.908 B (95,4%)**; `code` = 4.740.957 B; `data` = 373.227 B; `other` = 78.299 B |
| `vink-editor.js` | 271.882 | 88.610 | Chỉ nạp khi vào Bàn viết (`/ban-viet`), không nạp ở Khám phá/Reader |
| `vink.css` | 150.416 | 27.186 | Tokens + `vink-ui.css` + app styles |

**Phát hiện quan trọng ở baseline bundle:** Dù build bằng `--release` (`profile.wasm-release` có `opt-level = "z"`, `lto = true`, `codegen-units = 1`), `wasm-bindgen` trong [build.rs](../../xtask/src/build.rs) không gỡ custom section `name` (`--remove-name-section`), làm phần `custom:name` phình từ ~18,7 MB trong `vink_web.wasm` thành **106,8 MB** ở `vink_bg.wasm` (**112,0 MB raw** truyền không nén qua HTTP local nếu trình duyệt/proxy không nén).

## 2. Request-Reason Ledger (AC-02)

Bảng dưới tách rõ `logical_reads` (`L`), `transport_attempts` (`A`) và `validator_304_count` (`304`), giữ riêng các danh mục `public_catalog`, `public_discussion`, `own_discussion`, `capability_discussion`, `public_endorsements`, `own_endorsements` và `auth_account` khỏi `public_works_list` / `public_reader`. Nguồn chi tiết ở [ledger.json](evidence/web-perf-234/ledger.json).

| Bước / Lý do (`reason`) | Tổng L / A / 304 | `public_works_list` | `public_reader` | Endpoint phụ / ghi chú | Bytes wire |
| --- | ---: | ---: | ---: | --- | ---: |
| `cold_explore_desktop_1440_dpr1` | 16 / 16 / 0 | 0 / 0 / 0 | — | `document_ssr` (1), `asset_bundle` (13), `auth_account` (2) | 112.661.013 |
| `cold_reader_std_desktop_1440_dpr1` | 26 / 26 / 0 | — | 0 / 0 / 0 | `document_ssr` (1, 301 KB), `asset_bundle` (13), `auth_account` (2), `public_discussion` (1), `capability_discussion` (1), `own_discussion` (5), `public_endorsements` (1), `own_endorsements` (2) | 112.963.025 |
| `warm_client_nav_reader_first` (Desktop) | 17 / 17 / 0 | — | 1 / 1 / 0 | `auth_account` (1), `asset_bundle` (2, 0 B cache), `public_discussion` (2), `capability_discussion` (1), `own_discussion` (5), `public_endorsements` (3), `own_endorsements` (2) | 76.698 |
| `warm_client_nav_reader_cached` (Desktop) | 15 / 15 / 0 | — | **0 / 0 / 0** | Tái sử dụng `/reader` từ cache tab; nhưng vẫn phát 13 request API phụ (`public_discussion` 2, `own_discussion` 5, `public_endorsements` 3, `own_endorsements` 2, `capability_discussion` 1) | 70.243 |
| `warm_back` (< 120s, Desktop & Mobile) | 2 / 2 / 0 | **0 / 0 / 0** | — | Render ngay từ snapshot, không loading, giữ focus row; chỉ 1 `public_catalog` (`/api/v1/tags`) + 1 `asset_bundle` cached (0 B) | 1.159 |
| `history_back_forward` (`/ -> /toi -> history.back`) | 3 / 3 / 0 | 0 / 0 / 0 | — | Chrome khôi phục qua BFCache (`nav_type = navigate`); xem mục giới hạn cho nhánh `back_forward` reload | 456 |
| `stale_121s_burst` (4 tín hiệu đồng thời ở +121s) | 3 / 3 / 1 | **1 / 1 / 1** | — | 4 sự kiện (`focus`/`online`/`visibilitychange`/`pageshow`) gom đúng 1 `GET /api/v1/works` có `If-None-Match` → wire `304`, giữ DOM row & focus; `auth_account` (2) | 598 |
| `hidden_signals` (+130s `focus`/`online` khi ẩn) | 2 / 2 / 0 | **0 / 0 / 0** | — | Không gửi list read khi tab ẩn (`auth_account` 2) | 456 |
| `visible_after_hidden` (mở lại tab khi quá hạn) | 3 / 3 / 1 | **1 / 1 / 1** | — | Gửi đúng 1 `GET /api/v1/works` có validator → wire `304` | 598 |
| `manual_retry` (`503-transport-fixture-then-real-origin`) | 2 / 2 / 1 | **1 / 1 / 1** | — | Sau notice `503`, bấm thử lại phát đúng 1 conditional request về origin thật → `304` & xóa thông báo | 393 |
| `offline_revisit` (Chrome `Network.offline=true`, >121s) | 6 / 6 / 0 | — | 1 / 1 (failed) | Render bài viết Reader từ snapshot trong tab với notice làm mới thất bại, không rơi vào màn unavailable | 0 |
| `secondary_transport_503` (`transport-intercept-fixture`) | 7 / 7 / 0 | — | 1 / 1 (200) | `discussion/public` & `endorsements` trả 503; bài viết Reader vẫn đọc được bình thường và hiện `#discussion-load-retry` | 4.186 |
| `authoritative_withdrawal` (`real-app-postgresql`) | — | đã gỡ | `hidden` (no-store) | `POST .../publication/set-private` trả `200` (`made_private`, gen 3); sau đó `/publication` trả `404`, `/reader` trả `200 {status:"hidden"}`, SSR hiện `reader-hidden` và biến mất khỏi `/api/v1/works` | — |

**Phát hiện request fan-out quan trọng:** `READ-02` kiểm soát rất chặt các read chính (`0` read thừa cho `/api/v1/works` và `/reader` khi hydrate hoặc đọc lại còn hạn, `1` wire `304` cho burst 4 tín hiệu). Tuy nhiên, mỗi lần vào Reader qua client navigation vẫn phát **13 request API phụ không dùng cache chia sẻ** (`own_discussion` = 5, `public_endorsements` = 3, `public_discussion` = 2, `own_endorsements` = 2, `capability_discussion` = 1), và `auth_account` vẫn bắn khi mất/nhận `focus` kể cả lúc document ẩn.

## 3. Phân phối chỉ số theo cấu hình (AC-03)

Mỗi kịch bản cold/warm chạy `n = 10` lượt; thống kê báo `median [min..max]`, `range`, `lab_p95` (theo mẫu lab `n=10`), `iqr`, `cv_pct`. **Không tính hay suy diễn `field p75` (`field_p75 = null`).**

### Desktop `1440×900 @ DPR 1` (`n = 10`)

| Màn / Luồng | Chỉ số | Median | Min .. Max (Range) | Lab p95 | IQR / CV% |
| --- | --- | ---: | --- | ---: | ---: |
| Cold Explore (`/`) | TTFB (ms) | 2,3 | 2,0 .. 7,5 (5,5) | 7,5 | 0,3 / 56,52% |
| | Meaningful content (ms) | 8,9 | 8,4 .. 16,3 (7,9) | 16,3 | 0,4 / 22,89% |
| | FCP / LCP (ms) | 72,0 | 72,0 .. 88,0 (16,0) | 88,0 | 4,0 / 7,27% |
| | Hydrated (ms) | 163,85 | 152,6 .. 202,1 (49,5) | 202,1 | 12,4 / 8,21% |
| | Controls ready (`query/filter/next`, ms) | 163,85 | 152,6 .. 202,2 (49,6) | 202,2 | 12,4 / 8,22% |
| | Long tasks total (ms) | 53,5 | 53,0 .. 66,0 (13,0) | 66,0 | 2,0 / 6,85% |
| | CLS | 0 | 0 .. 0 (0) | 0 | 0 / 0% |
| | DOM elements / V8 heap used | 416 / 2.383.480 B | — | 416 / 2.383.596 B | `script`: 7.043 B |
| Cold Reader chuẩn (`standard`) | TTFB (ms) | 6,75 | 5,1 .. 16,0 (10,9) | 16,0 | 2,0 / 44,57% |
| | Meaningful content (ms) | 17,8 | 14,9 .. 26,2 (11,3) | 26,2 | 1,7 / 18,02% |
| | FCP / LCP (ms) | 68,0 | 52,0 .. 92,0 (40,0) | 92,0 | 8,0 / 15,48% |
| | Hydrated (ms) | 199,1 | 192,8 .. 241,5 (48,7) | 241,5 | 11,2 / 9,39% |
| | Controls (`return/article/composer/endorse`, ms) | 199,15 | 192,9 .. 241,7 (48,8) | 241,7 | 11,2 / 9,40% |
| | Long tasks total (ms) | 82,0 | 76,0 .. 106,0 (30,0) | 106,0 | 4,0 / 9,58% |
| | DOM elements / V8 heap used | 2.131 / 2.854.606 B | — | 2.131 / 2.910.292 B | `script`: 68.318 B |
| Cold Reader 200k (`long_poem_200k`) | TTFB (ms) | 26,5 | 25,4 .. 28,7 (3,3) | 28,7 | 1,7 / 4,38% |
| | Meaningful content (ms) | 39,8 | 38,6 .. 42,2 (3,6) | 42,2 | 1,3 / 2,89% |
| | LCP (ms) | 116,0 | 104,0 .. 128,0 (24,0) | 128,0 | 12,0 / 7,00% |
| | Hydrated (ms) | 255,15 | 252,3 .. 280,0 (27,7) | 280,0 | 2,1 / 3,01% |
| | Wall total (ms) | 280,751 | 273,941 .. 306,442 (32,501) | 306,442 | 4,489 / 2,98% |
| | DOM elements / V8 heap used | 2.204 / 2.935.834 B | — | 2.204 / 2.943.020 B | `script`: 516.863 B |
| Warm Client Nav (Explore → Reader) | Std (`n=10`) / 200k (`n=10`), ms | 137,916 / 63,935 | 132,958..148,079 / 52,783..134,38 | 148,079 / 134,38 | CV 3,10% / 32,92% |
| Warm Back (Reader → Explore) | Thời gian tới row + focus (ms) | 125,986 | 40,007 .. 139,622 (99,615) | 139,622 | 18,925 / 23,50% |
| Warm Interactions (`n=3`) | Mở trang 2 comment (`20→40`) / Search `Khúc mưa` (ms) | 110,021 / 41,691 | 109,746..129,296 / 41,633..136,982 | — | mẫu ngắn |

### Mobile `390×844 @ DPR 2` (`n = 10`)

| Màn / Luồng | Chỉ số | Median | Min .. Max (Range) | Lab p95 | IQR / CV% |
| --- | --- | ---: | --- | ---: | ---: |
| Cold Explore (`/`) | TTFB / Meaningful content (ms) | 2,1 / 9,35 | 1,9..2,6 / 8,4..11,3 | 2,6 / 11,3 | CV 12,1% / 8,3% |
| | FCP / LCP (ms) | 72,0 | 64,0 .. 72,0 (8,0) | 72,0 | 0 / 3,62% |
| | Hydrated / Controls ready (ms) | 168,4 / 168,45 | 160,6 .. 178,9 (18,3) | 178,9 | 6,2 / 3,27% |
| | Long tasks total / CLS | 58,0 ms / 0 | 53,0 .. 62,0 ms / 0 | 62,0 ms | CV 5,18% / 0% |
| Cold Reader chuẩn (`standard`) | TTFB / Meaningful content (ms) | 5,85 / 16,75 | 5,1..8,1 / 15,1..18,0 | 8,1 / 18,0 | CV 14,74% / 4,94% |
| | FCP / LCP (ms) | 64,0 | 56,0 .. 72,0 (16,0) | 72,0 | 4,0 / 7,81% |
| | Hydrated / Controls ready (ms) | 207,3 / 207,4..207,5 | 199,1 .. 235,5 (36,3) | 235,5 | 7,8 / 5,50% |
| | Long tasks total / CLS | 85,0 ms / 0 | 76,0 .. 113,0 ms / 0 | 113,0 ms | CV 12,22% / 0% |
| Warm Client Nav (Explore → Reader) | Time to reader + composer (ms) | 133,087 | 117,346 .. 147,276 (29,93) | 147,276 | 17,231 / 7,50% |
| Warm Back (Reader → Explore) | Time to row + focus (ms) | 134,163 | 39,153 .. 149,732 (110,579) | 149,732 | 6,086 / 33,19% |

*(Lưu ý nhãn bộ nhớ: `v8_used_bytes` là JS heap proxy qua CDP `Runtime.getHeapUsage`, không đại diện RSS toàn tiến trình hay bộ nhớ linear toàn phần của WASM).*

## 4. Disposal Stress, Chi phí Instrumentation & Negative Controls (AC-04, AC-05)

- **Disposal sau 25 chu kỳ (`Explore ↔ Reader` qua 20 works khác nhau > soft cap 16):**
  - Không dùng GC (`pre_gc`): số wrapper DOM/listener tăng tạm thời trước khi V8 dọn (`pre_gc_js_event_listeners = 910` ở cycle 0, `328` ổn định ở cycle 12 và cycle 24).
  - Sau `HeapProfiler.collectGarbage`: `active_timers = 1`, `active_window_doc_listeners = 11`, `cdp_js_event_listeners = 204` (cycle 12 = 204, cycle 24 = 204, delta = 0), `cdp_nodes = 2.142` ổn định ở cả nửa giữa và cuối, `v8_used_bytes` ~4,41–4,47 MB. Không phát hiện leak sau cleanup/GC.
- **Chi phí đo đạc (`instrumentation_overhead`, 5 lượt ON vs 5 lượt OFF trên Desktop):**
  - Observer ON median `175,952 ms` (`164,216..188,098 ms`); Observer OFF median `181,778 ms` (`174,850..189,271 ms`); `median_delta_ms = -5,826 ms` (chi phí nhỏ hơn nhiễu nền ~10 ms ở `n=5` trên loopback; observer chỉ tiêm qua `Page.addScriptToEvaluateOnNewDocument` trong pha baseline, 0 byte trong production).
- **Negative controls (bắt buộc FAIL assertion tương ứng khi lỗi xảy ra):**
  - `duplicateHydrationFetch`: inject `GET /api/v1/works` khi cold hydrate → `evaluateColdHydration` FAIL đúng.
  - `warmBackRefetch`: inject `GET /api/v1/works` khi warm Back → `evaluateWarmBack` FAIL đúng.
  - `lateResponseResurrection`: giả lập phản hồi cũ đè trạng thái Hidden → `evaluateLateResponseFence` FAIL đúng.
  - `unboundedListenerLeak`: mô phỏng tăng listener cuối chu kỳ → `evaluateDisposalBounds` FAIL đúng.

## 5. Bảng quyết định Go/No-Go cho `#235`–`#239` (AC-05)

| Decision ID | Hạng mục downstream | Quyết định tại baseline | Căn cứ số đo từ `#234` |
| --- | --- | --- | --- |
| `WEB-PERF-01` | `READ-02` typed Reader coordinator | `KEEP_BASELINE` | Cold hydration = 0 `/works` hay `/reader` dư; warm Back = 0 `/works` request; burst 121s gom 1 wire `304`. Tuy nhiên **cần chú ý fan-out endpoint phụ** (13 request/lần vào Reader). |
| `WEB-PERF-02` | [#235](https://github.com/loveoverflowcom/vink/issues/235) Selective SSR Streaming vs Async | `HOLD_ASYNC_UNTIL_BOTTLENECK` | Cold Explore LCP median `72 ms`, Cold Reader 200k hydrated median `255,15 ms` trên loopback. Chỉ spike selective streaming dưới giả lập độ trễ 200ms/1000ms có kiểm soát khi nguồn phụ thật sự nghẽn. |
| `WEB-PERF-03` | [#236](https://github.com/loveoverflowcom/vink/issues/236) Navigation anchor / Keep-Alive | `EVALUATE_ANCHOR_SPIKE` | Warm Back median `125,986 ms` (Desktop) / `134,163 ms` (Mobile) dù 0 network request; cần đo/đánh giá phục hồi vị trí cuộn & anchor qua 50 chu kỳ trước khi cân nhắc giữ DOM sống. |
| `WEB-PERF-04` | [#238](https://github.com/loveoverflowcom/vink/issues/238) Secondary `content-visibility` containment | `NO_CONTAINMENT_NEEDED_YET` | Cold 200k Reader chỉ dựng `2.204` DOM element, hydrated `255,15 ms`. Giữ nguyên DOM bài đọc đầy đủ; chỉ spike containment trên phần phụ khi bình luận/thẻ lớn và phải kiểm find/selection/print. |
| `WEB-PERF-07` | [#237](https://github.com/loveoverflowcom/vink/issues/237) RAM retention hard byte budget | `MEASURE_BYTE_PRESSURE_NEXT` | 25 chu kỳ bounded sau GC; nhưng `__RESOLVED_RESOURCES` của một bài 200k nặng `516.863 B` JSON và soft cap `16` entry chưa có trần tổng byte cho nhiều snapshot 200k liên tiếp. |
| `WEB-PERF-06` | [#239](https://github.com/loveoverflowcom/vink/issues/239) Public intent-prefetch | `NO_GO_BEFORE_BUDGET_AND_SSR_DECISION` | Prefetch sớm làm tăng nguy cơ wasted bytes và late-response khi bundle WASM thô còn `112 MB` và endpoint phụ Reader còn fan-out 13–15 request. Đóng tới sau `#235` và `#237`. |

## 6. Giới hạn trung thực và đường rollback

- **Mạng & CPU:** Toàn bộ số đo thực hiện trên loopback Apple M4 (`cpu_throttling_rate = 1`), không có 3G/4G packet loss hay low-end mobile CPU throttling; không phản ánh độ trễ truyền `vink_bg.wasm` ngoài loopback.
- **Lịch sử trình duyệt:** Headless Chrome khôi phục `history.back()` qua BFCache (`nav_type = navigate`), giữ trang sống nên không phát network read; nhánh document reload có `NavigationTiming.type = 'back_forward'` (`Millis::NEG_INFINITY` trong `public_reads.rs`) được phủ ở mức unit/contract, chưa ép reload được qua BFCache mặc định của Chrome 157 trong CDP.
- **Tác giả / dữ liệu:** Dùng fixture principal lặp (`SAMPLE_AUTHOR` / `OTHER_PRINCIPAL` / `PERF_MODERATOR`) và corpus tổng hợp; các kịch bản lỗi `503` / `A→B→A` là `transport-intercept-fixture`, chỉ bước thu hồi (`set-private`) là qua App + PostgreSQL thật.
- **Rollback:** Chỉ cần bỏ kịch bản `Scenario::PerfBaseline` và file [perf-baseline.mjs](../../apps/vink-web/e2e/perf-baseline.mjs) + [perf_journey.rs](../../services/vink-app/tests/support/perf_journey.rs) (9 kịch bản `verify web` mặc định và runtime ứng dụng không bị ảnh hưởng).
