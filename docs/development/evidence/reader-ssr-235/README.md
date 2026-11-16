# Bằng chứng #235 — SSR Reader

Ngày 16/11/2026 UTC, dot computer Debian 13 x86_64. Base đã fetch lại:
`f1b46703ae3329ee7cd11fb06db3f8157a31e5b9` (PR #251 merged). Không có PR #235 trùng ở lượt
search trước publish; worktree #229 đang dirty được giữ nguyên. Nhánh `codex/reader-ssr-boundary`.

[Source identity](source-identity.json) ghim code commit và SHA-256 các file thực thi/fixture.
Các commit evidence sau đó chỉ đổi docs/logs. Không dùng kết quả #251 làm PASS của lượt này.
Toolchain Rust 1.99.1 + wasm32, wasm-bindgen 0.2.129 được cài đúng pin; PostgreSQL 17.11 được
build trong `/tmp/vink-235-pg`, cluster riêng chỉ TCP loopback, ICU 76.1, collation `und-x-icu`
đã query xác nhận. Cluster test đã được stop bằng trap. Không thay database/`.env` của người dùng.

| Lệnh / boundary | Kết quả thực chạy |
| --- | --- |
| `cargo test --locked -p vink-app --test ssr_boundary -- --nocapture` | **PASS** một matrix, 1,27 giây runtime: route Async, secondary hold 0/200/1000 ms, Published/Hidden/ReviewWithdrawn/404/503, safe metadata; [log](ssr-tests.log). Port tổng hợp, Leptos/Axum thật; không SQL/proxy. |
| `cargo test --locked -p vink-app --test postgres -- --test-threads=1 --exact work_moderation::test_work_moderation_full_flow_and_review_withdrawn_page work_moderation::review_withdrawn_page_survives_a_pending_resubmission work_moderation::hidden_reader_retains_only_discussion_and_republication_restores_body` | **PASS 3/3**, 208 filtered, 2,34 giây runtime, real App/API + PostgreSQL ICU; [log](postgres-tests.log). `DATABASE_URL` chỉ trỏ cluster tạm. Không phải full postgres lane hay browser. |
| `cargo xtask verify unit web --filter public_reads` | **PASS 10/10**, 221 filtered; [log](public-reads-tests.log). Production coordinator: SSR seed ordering, ticket/invalidation, removal/republish, Memory/NoOp; pure Rust, không browser/storage adapter. |
| `node --check` hai file JS thay đổi; `node --test apps/vink-web/e2e/web-perf-baseline.test.mjs` | **PASS**, analyzer 3/3 (bao gồm negative duplicate predicate); không chứng minh UI/runtime gain. |
| `cargo test --locked -p xtask --no-default-features` | **PASS** 82 unit (một ignored) + một integration ở base (xtask runner step được thêm sau; quick cuối là kiểm trên step mới). |
| `cargo xtask verify web --scenario main --scenario discovery --scenario review --plan` | **PASS** planner, selected coverage: Bundle/App, chỉ App scratch, không Identity; không execution claim. |
| `cargo xtask verify web --scenario main --scenario discovery --scenario review` | **FAIL** lúc start Chrome; [log](web-icu-failure.log). Prerequisites, visual oracle unit, build, bundle secret scan, migrate/seed App và App readiness **PASS**. Không browser assertion nào chạy; discovery/review sau đó **NOT_RUN**. |
| `cargo xtask verify quick` cuối | **FAIL**: 10 bước trước unit PASS (gồm matrix SSR mới), App unit 325 PASS / 9 FAIL tại Unix socket fixture; [failure/summary](quick-failure.log). Repository checks sau đó **NOT_RUN**. Không đổi/skips tests để làm xanh. |
| Release benchmark, raw direct/proxy chunks, B/C implementation/rollback, OS accessibility | **NOT_RUN**. Không timing gain hoặc field p75 claim. |

Chrome đã được gọi qua wrapper chỉ thêm `--no-sandbox`. Failure gốc:

```text
FATAL:chrome/browser/process_singleton_posix.cc:297
socket() failed: Operation not permitted (1)
Chrome did not open DevTools within 20s
```

Đây là giới hạn sandbox Unix socket, không phải assertion ứng dụng. Cùng restriction làm 9 test
Identity hiện hữu fail ở `services/vink-app/src/http/mod.rs:793` (`UnixListener::bind`). File
đó byte-identical với base, không bị PR sửa. Không nói đã chạy baseline suite riêng. Không sửa sandbox, framework,
mutation hay privacy guard để làm demo xanh. Browser module đã được sinh và syntax-check,
**chưa** được nghiệm thu runtime; raw `reader-ssr-boundary.json` chưa được tạo.

[Những lượt chưa hoàn tất/lỗi setup](setup-failures.log) giữ riêng: lần đầu thiếu ICU đã sửa;
library SSR parallel treo đã chuyển sang integration process; compile timeout 240 giây đã bỏ,
retry chạy matrix PASS. Không lấy các lần đó làm PASS hoặc xóa assertion.
[Decision snapshot](decisions.json) là evidence query, không signoff hay SSOT mới.

## Đối chiếu acceptance

| AC #235 | Có bằng chứng | Còn thiếu |
| --- | --- | --- |
| AC-01 | PASS source/decision inspection: A giữ Async; owner critical/secondary và điều kiện xem lại B/C ở [report](../../reader-ssr-boundary.md) | Không có architecture Accepted mới. |
| AC-02 | PASS các boundary riêng: synthetic SSR status/meta/privacy matrix; real PG/API moderation/hidden/republication | Chưa là browser end-to-end hoặc proxy qualification; redirect/canonical scenario discovery chưa chạy trên HEAD. |
| AC-03 | Test blocked/held bootstrap tích hợp main, syntax PASS | NOT_RUN browser: no-JS, WASM hỏng/chậm, native GET/link và không replay click chưa được chứng minh trên HEAD. |
| AC-04 | PASS 10 pure READ-02 tests về seed/ticket/race; owner/key giữ nguyên; test fresh SSR handoff có code | NOT_RUN browser: raw trace mới, stale/history/reordered navigation/before-after chưa có. |
| AC-05 | PASS route pin Async; không đổi runtime nên baseline vẫn tồn tại | NOT_RUN so sánh B/C, actual rollback experiment, release/direct-proxy chunks. Không suy “không đo được gain” thành “B/C không có gain”. |

Không tick/close issue #235. Draft PR dùng Refs, giữ các gap để agent có browser-capable environment
chạy tiếp. Không merge/deploy và không đọc/chờ/poll GitHub CI.

## Tiếp tục trên môi trường hỗ trợ browser

Giữ đúng Rust/wasm-bindgen pin và fixture; cần Chrome được phép tạo Unix socket và các Identity
unit fixtures cũng cần capability đó. Chạy lại `cargo xtask verify quick` và
`cargo xtask verify web --scenario main --scenario discovery --scenario review` trên source HEAD,
lưu raw `reader-ssr-boundary.json`/captures khi có. Không thay assertion để thích ứng restriction.
Sau đó mới xét gap release/secondary/proxy của #234 và #235; B/C tiếp tục không được bật.
