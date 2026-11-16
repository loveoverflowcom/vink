# Review độc lập PR #266 trên Mac — 16/11/2026 UTC

Implementation Dot được review ở `59779916235f46fbb9710a5eb9ffab1f5b9050e4`.
Đã đọc issue #258, body/comments/reviews/threads của PR (không có comment/review/thread lúc đọc),
AGENTS, owner skills, diff, caller và assertions. Base mới là
`develop@ffed5b5c6c004b9f55ecc8e0d9be8d863d6e6a60`; rebase sạch thành `91152b7`.
Source cuối là `0edf21c867092e9ad29a218c19608ddb295651ba`.
[Source hashes](source-final.json) pin 149 inputs; các thay đổi evidence sau đó chỉ là docs.
Review implementation ban đầu độc lập với agent Dot; các bản sửa bên dưới được self-review.

**Final selected Web result: PASS (`main + discovery`, exit 0 tại source `0edf21c`).** Không dùng kết quả CI trong review này.

## Findings và bản sửa

1. **P2 — Font literal làm Quick thất bại ở repository checks.**
   `font: 400 1rem/1.5 var(--vk-font-ui)` vượt contract token của owner UI.
   [Quick trước sửa](quick.log) đã qua compile/lint/32 doctest/SSR boundary/unit rồi FAIL đúng
   declaration này. `00ec625` dùng body tokens hiện hữu (400, 1rem, line-height 1.6), giữ phạm vi
   `.vk-empty-state__message`; LayoutSpec được đối chiếu lại. Không đổi font hay token toàn app.

2. **P2 — Submit không chuyển focus đến landmark kết quả.**
   [Web trước sửa](web-negative.log) FAIL `submitted search focuses the named landmark`;
   [ảnh](focus-failure.png) và [trace riêng](focus-trace-before.json) ghi `activeElement=BODY`,
   không có lời gọi focus tới region. Handler nội bộ của Leptos `Form` dừng propagation của
   submit, nên callback `on:submit` gắn ngoài không ghi được intent trước khi scope bị thay.
   Đây là gap trong wiring focus dùng lại từ source cũ, được xác minh trên PR hiện tại;
   không có runtime before-base để khẳng định thời điểm gap xuất hiện.
   `c0d28a1` chuyển intent/đóng picker sang `on_form_data` của chính Form, trước navigation,
   cùng callback bỏ unnamed submitter. GET, Apply, URL và input owner vẫn dùng luồng cũ.
   Browser kiểm focus region → Tab tới recovery với ring rõ, và fallback khi hàng trả về mất.

3. **P2 — Focus deferred có thể cướp caret khi người dùng đang gõ tiếp.**
   `main + discovery` đã PASS trên `9e9ad1a` ([log](web-9e9ad1a.log)), nhưng review scheduling
   phát hiện cửa sổ chưa được kiểm: submit A → giữ response → người dùng gõ query B chưa áp dụng
   → A hoàn tất. [Regression mới](pending-submit-negative.log) FAIL trên cùng runtime;
   [ảnh](pending-focus-failure.png) và trace ghi active element bị kéo từ input tới section.
   `0b39b58` bỏ `DiscoveryFocus` và effect lúc resource hoàn tất. Lượt chỉ dời focus sang rAF
   vẫn FAIL khi query B được nhập trước frame ([log](public-search-intent.log)). Guard cuối
   hủy intent nếu query/focus/caret đã đổi; chỉ cho phép picker tự trả focus về trigger như
   một phần của chính submit đó. Landmark hiện hữu cả khi loading, I/O hoàn tất không sở hữu
   focus. [Focused cuối](public-search-complete.log) PASS query B/caret, submit lặp và Apply
   changed/unchanged. Một build giữa chừng FAIL do `web_sys::FormData` không có ở SSR; đã dùng
   re-export Leptos như owner hiện hành ([history](guard-build.log), [build sau](guard-build-final.log)).

4. **P2 — Catalog pending làm Apply không đổi bỏ mất tag.**
   Loading fallback có sẵn của `FilterForm` chỉ hiện status, trong khi nút Apply vẫn dùng được.
   [Ca giữ request](catalog-loading-negative.log) xác nhận catalog thực sự pending và native
   `FormData` trả `[]` dù đang áp dụng một tag. Không sửa bằng cách đợi catalog nhanh hơn.
   `0edf21c` giữ selected tags qua shared `HiddenField` trong fallback, cùng cách nhánh lỗi
   catalog đang dùng. Focused cuối PASS đúng tag fields và URL tag sau unchanged Apply.

5. **Harness — Corrupt image fixture từng bị cache bỏ qua.**
   [Lượt focused thất bại](public-search-cache-fixture-fail.log) và
   [ảnh](corrupt-fixture-cache-hit.png) cho thấy WebP hợp lệ vẫn được dùng từ cache.
   `9e9ad1a` tắt/xóa cache trong browser profile tạm của đoạn failure, đếm đúng một response
   corrupt thực sự được inject, và tăng assertion blocked image thành `naturalWidth=0`.
   Giữ nguyên yêu cầu decode failure, ratio, một HTML status và href thật.
   [Focused sau sửa fixture](public-search-fixed.log) PASS cả blocked và corrupt trước hydration;
   [focused trên source cuối](public-search-complete.log) kiểm lại cùng các assertion này.

Giả thuyết freshness remount làm mất focus **không tái hiện**: regression giữ request rồi trả
503 đã xác nhận cùng node ảnh/link, focus vẫn ở recovery, và đúng một status có qualifier
“Bản đã tải”. Vì vậy không sửa API hay renderer của `EmptyState` để xử lý một lỗi chưa quan sát.

## Các lượt chạy thật

| Lệnh / boundary | Kết quả / source | Evidence |
| --- | --- | --- |
| Raw fetch ZIP nguồn qua Drive connector, so SHA-256 và bytes trong archive | PASS ZIP 1.054.999 bytes khớp hash issue; cả 3 WebP byte-identical với repo, tự kiểm lại trên Mac | [proof](source-zip-proof.json) |
| `node --test apps/vink-web/assets/empty/verify.test.mjs` | PASS 3 regressions; bytes/manifest không đổi qua rebase/fixes | [assets](assets.log) |
| `dwebp <mỗi WebP> -o <task PNG>` | PASS cả 3: full decode lossy/alpha 512×427; đã mở cả ba WebP | [decode](full-decode.log) |
| `./tools/local-budget run -- cargo xtask verify quick` | PASS toàn lane tại `0edf21c`: clippy server/hydrate/preview, 32 doctest, SSR boundary, 334 App / 235 Web tests, các unit/contracts khác và repository checks | [final Quick](quick-final.log); các lượt c0d28a1/0b39b58 giữ làm lịch sử |
| `cargo xtask build web` qua launcher | PASS source `0edf21c`, actual SSR binary/bundle/static assets | [build](catalog-build.log), [runtime Web cuối](final-web-runtime.sha256) |
| `node apps/vink-web/e2e/browser-smoke.mjs --base http://127.0.0.1:55570 --chrome <Chrome Mac> --state review-266/guard-state.json --phase public-search` | PASS `0edf21c`; real PostgreSQL/public routes, transport holds/503 được ghi riêng; app/browser cùng lifecycle owner qua [one-off script](run-search.py) | [focused](public-search-complete.log) |
| `cargo xtask verify web --scenario main --scenario discovery` | PASS trên source cuối `0edf21c`, exit 0; 5.669 browser assertions; SSR/hydration/editor/save/reload/conflict/leave guard/preview/draft tags, discovery corpus/pagination/reader return/transport race và graceful stop | [final Web](web-final.log); [lượt 9e9ad1a lịch sử](web-9e9ad1a.log) |
| Matched Explore/tag capture; fresh empty database | PASS 40 before + 40 after + 20 corpus captures; 32 filtered + 16 corpus geometry cases và 12 end-of-page checks | [before](before-manifest.json), [after](after-manifest.json), [corpus](corpus-manifest.json) |

Quick và focused browser đều chạy trên source cuối `0edf21c`. Node syntax và diff checks
đã chạy lại; combined Web cuối PASS sau khi toàn command hoàn tất.
Các verdict trên là self-run trên Mac, không lấy lời báo PASS từ Dot làm execution của Mac.

Môi trường: macOS 15.6 arm64, Rust 1.99.1, Node 22.15.0, wasm-bindgen 0.2.129,
Chrome 157.0.8103.78, PostgreSQL 17.11 Homebrew với ICU, dwebp/libwebp 1.5.0.
CWD implementation: `/Users/manhblue/Documents/Codex/2026-11-17/task/vink`;
before: sibling `vink-266-before` tại exact base. Dùng
`CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0`, Cargo jobs/libtest
threads tối đa 2, `npm_config_cache=/tmp/vink266-npm` và
`VINK_CHROME='/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'`.
PostgreSQL mới riêng trên loopback 55566; `VINK_TEST_DATABASE_URL=postgres://vink266@127.0.0.1:55566/postgres`.
Browser verification là UNSUPPORTED qua launcher; chạy trực tiếp riêng sau các heavy lane khác,
không nhận đó là evidence tham gia host-user slot. Build/Quick dùng launcher.

Lượt Quick mặc định debug ban đầu được dừng chủ động khi thiếu disk:
[SIGTERM history](quick-debug-interrupted.log), later steps NOT_RUN; sau đó dùng debug-info=0.
Setup capture đầu thiếu local profile khi seed, rồi thiếu tag fixture; không có mutation bị
retry mù. [Server chưa sẵn sàng](before-capture-setup-fail.log) và
[catalog thiếu](before-capture-catalog-fail.log) được giữ. Capture DB được seed một tác phẩm mẫu
và tag Thơ qua CLI thật, giữ nguyên dữ liệu giữa before/after. DB corpus chỉ migrate, không seed.
DB focused dùng cùng catalog sáu thẻ/alias/description như owner Web, qua [script](seed-focused.py).
Một lần diagnostic không còn listener, nên page-load FAIL và focus scenario NOT_RUN
([log](pending-server-gone.log)); lần sau ghép app/browser chung owner và probe readiness
trước khi chạy. Negative pending-submit thật ghi server còn live, không dùng lỗi setup làm finding.

## Render, mạng và semantics

Ma trận 320/390/834/1440 × light/dark × text100/200: art 192px mobile / 240px desktop,
ratio 512:427, body 16/32px, ảnh không nhân đôi theo chữ. Không overflow ngang; recovery sau
cuộn nằm trên bottom navigation. Text200 ở đây là root font-size, không phải browser zoom.
Font thực tải gồm Be Vietnam Pro 400/600 và Literata; fonts/tokens chung giữ nguyên.
Focused browser đo contrast status/actions cả hai theme; lượt Web cuối ghi
[light](empty-contrast-light.json) minimum 6,72 và [dark](empty-contrast-dark.json) minimum 10,83 (yêu cầu ≥4,5).
[Hashes runtime cũ](after-runtime.sha256) thuộc lượt `9e9ad1a`; [hashes cuối](final-web-runtime.sha256)
được lấy sau build của lượt selected Web cuối. Capture pin checkout/source và hashes PNG/WebP;
không ghi riêng binary hash ngay trước lượt capture cuối.
After/corpus và các PNG after/corpus được dẫn bên dưới đều được chụp lại trên source cuối
`0edf21c`; 48 geometry cases và 12 end-clearance cases được đối chiếu hash/geometry lần cuối.

Mỗi document settled-empty có đúng một resource entry của current-state artwork qua mọi lần
đổi theme/text/viewport. Ready, invalid cursor và missing tag có 0 artwork request trong
[after probes](after-manifest.json). Ba derivatives được HTTP-check MIME/bytes/SHA-256 và
decode qua browser `Image.decode()`; đây là decoder probes riêng sau capture, không phải app
preload ba ảnh. Build chỉ chép ba WebP, không có Drive request hoặc source PNG trong bundle.
[AX Explore](after-explore-ax.json), [AX tag](after-tag-ax.json) và
[AX corpus](corpus-corpus-ax.json) expose một region “Tác phẩm khám phá” và một status;
art trang trí không xuất hiện như image có tên trong AX tree.

Một screenshot after đầu ở 1440/light/100 thiếu cả nhóm content mặc dù geometry/decode/status
đúng. Giữ [PNG](capture-transient.png) và [manifest đầu](after-initial-manifest.json).
Chạy lại **nguyên matrix, cùng source/fixture/script/viewport policy**, không chỉnh app hoặc
ngưỡng, không tái hiện; ảnh 1440/light/100 rerun được mở cùng ảnh before. Lượt đầu được coi là
capture quan sát chưa kết luận, không thay bằng lời PASS cho chính PNG đó.

Các ảnh sau đã mở; chúng là app render, không phải board tạo sinh hay golden được owner duyệt.
Fidelity: **NO_APPROVED_REFERENCE**, đối chiếu LayoutSpec R1 và contract thật.

| Cùng scenario/viewport/theme/text | Before | After |
| --- | --- | --- |
| Explore 1440/light/100 | [PNG](captures/before/explore-light-1440-100.png) | [PNG](captures/after/explore-light-1440-100.png) |
| Explore 390/dark/200, cuối trang | [PNG](captures/before/explore-dark-390-200-end.png) | [PNG](captures/after/explore-dark-390-200-end.png) |
| Tag 320/light/100 | [PNG](captures/before/tag-light-320-100.png) | [PNG](captures/after/tag-light-320-100.png) |
| Tag 320/dark/200 | [PNG](captures/before/tag-dark-320-200.png) | [PNG](captures/after/tag-dark-320-200.png) |

Corpus rỗng thật: [1440/light/100](captures/corpus/corpus-light-1440-100.png),
[390/dark/100](captures/corpus/corpus-dark-390-100.png),
[320/dark/200, cuối trang](captures/corpus/corpus-dark-320-200-end.png).
PNG đại diện được commit; matrix manifest giữ hashes/geometry của toàn run. Raw đầy đủ vẫn ở
`review-266/{before,after-initial,after,corpus,screenshots}` trong checkout task. Reproduce bằng
[one-off capture script](capture.mjs) với `BASE_URL OUT_DIR SOURCE_SHA [corpus|focus]` từ repo root.

## Acceptance và giới hạn

| AC | Evidence trên source cuối | Verdict |
| --- | --- | --- |
| AC-01 | Original recorded bytes/container/hash/budget; native/browser decode; real build/HTTP; one current artwork / zero unrelated branches | PASS |
| AC-02 | Opened matched Explore/tag renders; one concise sentence/actions; nonempty/pagination retained in browser | PASS |
| AC-03 | Real corpus/miss/tag/invalid; retained refresh failure/browser race; four SSR tests for cursor/continuation/retired/removed | PASS |
| AC-04 | Native GET, q/tag/cursor reset, Apply, keyboard/focus, missing-row fallback, real reader return and simulated composition | PASS |
| AC-05 | One status, named AX region, decorative image, focus ring, blocked/corrupt before hydration with actual request failure | PASS focused |
| AC-06 | 48 after geometry cases, twelve clearance checks, loaded fonts/contrast, representative 100%/200% pixels opened | PASS |
| AC-07 | Asset/Quick/focused browser and opened matched captures | PASS selected `main + discovery` |
| AC-08 | Current source map/decisions/evidence; unchanged shared composition/pipeline for #259 | PASS |

| UX dimension | Result / boundary |
| --- | --- |
| Functional | PASS selected `main + discovery` trên PostgreSQL thật |
| Geometry | PASS 48 final after cases + 12 bottom-clearance checks; no horizontal overflow |
| Visual | PASS within opened representative captures / LayoutSpec R1; no approved golden claim |
| Accessibility / Responsive | PASS named AX/status, keyboard focus/ring, measured contrast and narrow/text200 cases; named screen reader NOT_RUN |
| Interaction | PASS GET/Apply/reader return/focus/retained refresh/programmatic composition; real OS IME and real mobile keyboard NOT_RUN |

Full unselected `verify web` scenarios, named VoiceOver/TalkBack, real OS Telex/VNI and native
device/CMP are NOT_RUN. Domain/Quint and a new persistence lane are NOT_APPLICABLE to this diff:
no domain, lifecycle model, SQL/API/auth contract changed. Existing PostgreSQL supplies real
public reads for browser scenarios; this does not qualify a new persistence feature.

Implementation and before checkouts are task-owned isolated trees. Existing worktree #204,
other user trees, `.env`, caches and databases were not reset/stashed/cleaned. Only generated
artifacts of this task's own completed build/test attempts were reclaimed. No VPS deployment.
Issue #258 is the live checklist; other issue checkboxes are outside this update.
For #259, use `ui::empty_state::{EmptyArtwork, EmptyState}`, existing `.vk-empty-state*` CSS
and `/pkg/empty/`; Desk caller continues to own state, actions, focus and announcements.
