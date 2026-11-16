# Review độc lập PR #265 trên Mac — 16/11/2026

Base đã fetch lại: `f1b46703ae3329ee7cd11fb06db3f8157a31e5b9`.
Source ban đầu: `06bf26db478e52075b9fc6d46fb3aaa0c90ebf49`.
Fix control: `2fc5e97e301c52ac2bd414d090cfc86037ea3ca4`.
Source cuối (thêm fix harness): `f2bf183ab960745f551c04fe3e4860bc3c4967a4`.
Các lượt quick/Web bắt đầu trên cây sạch; commit evidence sau đó chỉ đổi docs/logs.
Develop đã fetch lại sau lượt Web cuối, vẫn ở base trên; không có delta cần retest.
[Source trước sửa](source-initial.json) và [source sau sửa](source-final.json) ghi môi trường,
SHA-256 của inputs và CWD. Không lấy lời báo PASS của dot computer làm kết quả self-run.

## Finding và bản sửa

**P2 — Client-only reload hiện enabled trước hydration.** Đây là thiếu sót đã có trong base,
được phát hiện khi kiểm AC-03; module browser mới của PR chưa assert trạng thái refresh.
SSR thật ghi `discussion-refresh` có `data-state="enabled"`, không có `aria-disabled`
([attributes](ssr-action-attributes.json)). Secondary failure cũng render retry enabled.
Thêm assertion vào matrix trước sửa tạo [FAIL đúng lỗi](ssr-negative.log), không hạ guard hoặc
bỏ assertion. Sau sửa, [matrix PASS](ssr-fixed.log).

`DiscussionLoaded` và `DiscussionLoadRetry` nay dùng readiness của chính component: false ở
SSR/initial hydration, true sau client effect. Hai nút có `aria-disabled` và lý do cần JavaScript
trước khi callback có thể dùng. Browser assert refresh unavailable khi bootstrap blocked/held
và enabled sau hydration, rồi tiếp tục kiểm không replay mutation/duplicate typed Reader.
Không đổi Async, read coordinator, cache policy, public/private authority, API hoặc DB. Đây là
presentation lifecycle guard; không đổi semantic của model publication/read owner.
**P2 — Harness kiểm hold trước event pause.** DOM SSR có thể sẵn sàng trước deferred import;
lượt Web tại `2fc5e97` FAIL đúng check `bootstrap is actually paused before the click (200ms)`.
`f2bf183` chờ `Fetch.requestPaused` với deadline 10 giây rồi giữ nguyên assertion và injected
hold 200/1000 ms. Không tăng ngưỡng để bỏ qua failure và không bỏ check.
Không tìm thấy actionable regression còn lại trong phạm vi diff đã review.

## Kiểm đã chạy

| Lệnh / source / boundary | Kết quả | Evidence |
| --- | --- | --- |
| `cargo xtask verify quick`, `06bf26d` | PASS | [initial quick](quick-initial.log) |
| `cargo xtask verify quick`, `2fc5e97` | PASS toàn lane: clippy server/hydrate/preview, 32 doctest, matrix SSR, 334 App/231 Web và các unit/contracts khác, repository checks | [final quick](quick-final.log) |
| `cargo test --locked -p vink-app --test ssr_boundary -- --nocapture`, final assertion + runtime `06bf26d` | FAIL có chủ ý: retry enabled trước hydration | [negative](ssr-negative.log) |
| Cùng matrix, runtime/test content của `2fc5e97` | PASS: Async; secondary hold 0/200/1000 ms; Published/Hidden/ReviewWithdrawn/404/503; meta/privacy và retry unavailable | [fixed](ssr-fixed.log) |
| `cargo xtask verify web --scenario main --scenario discovery --scenario review`, `06bf26d` | PASS cả ba scenario | [initial Web](web-initial.log), [initial trace](reader-ssr-boundary-initial.json) |
| Cùng Web command/config, `f2bf183` | PASS cả main, discovery và review; exit 0 | [final Web](web-final.log), [final trace](reader-ssr-boundary-final.json) |
| Ba PostgreSQL/API test `work_moderation::{test_work_moderation_full_flow_and_review_withdrawn_page, review_withdrawn_page_survives_a_pending_resubmission, hidden_reader_retains_only_discussion_and_republication_restores_body}`, `06bf26d` | PASS 3/3, 208 filtered, real PostgreSQL 17.11 ICU; source API/DB không đổi trong fix | [PostgreSQL](postgres.log) |
| `node --check apps/vink-web/e2e/reader-ssr-boundary.mjs`, fmt và diff check | PASS | syntax/format; runtime riêng ở các dòng trên |

Reproduce từ repo checkout (`.../2026-11-16/task-6/vink`):

```sh
export CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
export npm_config_cache=/tmp/vink-265-review/npm
export VINK_CHROME='/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'
export VINK_TEST_DATABASE_URL=postgres://postgres@127.0.0.1:55465/postgres
cargo xtask verify quick
cargo xtask verify web --scenario main --scenario discovery --scenario review
DATABASE_URL="$VINK_TEST_DATABASE_URL" cargo test --locked -p vink-app --test postgres -- \
  --test-threads=1 --exact \
  work_moderation::test_work_moderation_full_flow_and_review_withdrawn_page \
  work_moderation::review_withdrawn_page_survives_a_pending_resubmission \
  work_moderation::hidden_reader_retains_only_discussion_and_republication_restores_body
```

MacOS 15.6 arm64; Rust 1.99.1, wasm-bindgen 0.2.129, Node 22.15.0, Chrome 157.0.8103.78,
PostgreSQL 17.11 Homebrew với `und-x-icu`. Cluster mới riêng trên loopback, không dùng DB sản phẩm.
Debug với debuginfo tắt để giới hạn dung lượng; không phải release benchmark.
Các lệnh thực tế chạy trực tiếp, chưa qua `tools/local-budget`; đã kiểm tiến trình và dung lượng
thủ công. Không nhận đây là evidence tham gia host-user slot của launcher.

Các lần setup/development FAIL được giữ trong [failure history](setup-and-development-failures.log):
Web chạy khi quick còn giữ artifact lock; thiếu `VINK_CHROME`; npm cache mặc định có EACCES;
lần compile đầu của fix thiếu `#[prop(into)]` cho callback, đã sửa. Những bước chưa tới ở mỗi
lần đó là NOT_RUN. Không xóa lock đang dùng, không sửa quyền cache npm của người dùng và không
bỏ check. Race Fetch kể trên cũng là FAIL có thật trong history, các check phía sau NOT_RUN ở lần đó.
Lượt kế tiếp trên cùng `f2bf183` qua boundary mới nhưng FAIL `ENOSPC` khi ghi contexts.json giữa
main; discovery/review NOT_RUN trong lần đó. Đã dọn thêm cache không dùng rồi chạy lại nguyên
command, không thay source hoặc bỏ capture.
Raw logs đầy đủ của phiên ở `/tmp/vink-265-review/`; captures đầy đủ giữ ở
`target/verify-web/1794846042388234000-44289`. [Manifest](artifacts.json) ghi SHA-256/size
của các bản sao evidence được commit (không bao gồm chính manifest). Bản `.log` commit chỉ
bỏ whitespace cuối dòng/EOF để qua diff check; [raw hashes và phép chuẩn hóa](log-normalization.json)
ghi rõ khác biệt. JSON trace giữ nguyên bytes, raw logs gốc vẫn ở thư mục tạm trên.

Quick PASS tại `2fc5e97` vẫn áp dụng cho Rust/Cargo/SSR tests của `f2bf183`: diff sau đó chỉ thêm
bounded wait trong browser module và tài liệu. Node syntax/diff check đã chạy lại sau delta đó;
browser chạy lại trên `f2bf183`. [So sánh trace](comparison.json) giữ cả source SHA và cùng
fixture/config; không dùng số này làm so sánh latency.

## Acceptance và giới hạn

- **AC-01:** PASS source/decision inspection. Giữ A; owner critical/secondary, A/B/C và điều kiện
  xem lại rõ trong [owner report](../../../reader-ssr-boundary.md). READ-02/DISC-WEB vẫn
  DESIGN_PROPOSED; O-03/O-17 r2 PRODUCT_CONFIRMED; offline age PENDING. Không có signoff mới.
- **AC-02:** matrix Leptos/Axum + real PG/API + browser direct local cover status/meta/privacy,
  hidden/withdrawn và canonical redirect. Không dùng synthetic port làm authority SQL evidence;
  proxy qualification vẫn NOT_RUN.
- **AC-03:** blocked/held bootstrap cover native article/link/GET search, unavailable send/reload,
  không mutation/replay trong cửa sổ ghi ở trace. Không suy thành mọi client-only control,
  browser setting hoặc OS accessibility đã được nghiệm thu. Chưa inject riêng lỗi/chậm WASM
  sau khi JS đã tải; đó là NOT_RUN, không gộp với lỗi bootstrap JS.
- **AC-04:** trace cùng fixture/config trước/sau fix ghi fresh typed `/reader`; main giữ assertions
  stale revalidation, wire 304 và late response A→B→A. Không phải baseline release hoặc ledger
  đầy đủ cho mọi history tuổi không rõ/secondary/race của #234.
- **AC-05:** route pin Async PASS; không có optimization hay claim gain. B/C implementation,
  actual rollback experiment và so sánh release/direct/proxy chunks NOT_RUN.

#234 vẫn partial; #235 giữ mở cho acceptance còn thiếu. Không chạy/poll GitHub CI, không deploy.
Full postgres lane, các Web scenario ngoài main/discovery/review, native/OS IME/screen-reader,
release benchmark và performance go/no-go đều NOT_RUN trong review này.

Chỉ dọn Cargo debug/wasm caches không có tiến trình dùng trong task-2/task-3/task-4/task-5
([cleanup](build-cleanup.json)); giữ source, logs, captures và DB. Đường dẫn checkout #204 lịch sử
đã vắng từ trước lần kiểm đầu của phiên; các process/PG/Chrome artifacts #204 hiện có không bị
đụng tới ([preservation](preservation.json)). Không reset/stash checkout nào.
