# Bằng chứng #273 — HTTP draft-save

Base: `9640bdef4b570fe0fc76e90e538de947c2428614`.
Source đã kiểm: **`4778e1919d4717d0ffe97c53ce16f3520982ff30`**.
Commit evidence tiếp theo chỉ thay thư mục này; không thay code/tests/contracts/decisions đã kiểm.
Không đọc/chờ/rerun GitHub CI. [Owner, bound và rollback](../../draft-http-budget.md).

## Môi trường và preflight

18/11/2026 UTC, macOS 15.6, Rust 1.99.1, Node 22.15.0, PostgreSQL 17.11,
Chrome 157.0.8103.79; [metadata](environment.json). Profile dev/test debug=0, incremental=0.
Lệnh được hỗ trợ đi qua `tools/local-budget`, tối đa 2 Cargo jobs, 2 libtest threads;
browser chạy raw lane tuần tự với jobs=2/threads=1 vì launcher chưa hỗ trợ ownership của Chrome.
Cluster mới chỉ bind loopback port 55573; không dùng/đổi các cluster cũ #265/#204.

Checkout chính sạch trên develop; không có implementation/worktree/PR #273 trùng. Fetch và
fast-forward giữ nguyên base trên; tạo worktree `codex/long-document-http-budget` riêng.
PR #285 còn Draft tại `fd5a922dd2b81d24381120ea64e041b9c1be41ea`, `http/api.rs` bằng base;
thread báo active nên giữ nguyên source, DB, evidence và artifacts của nó. Cache build mới là
bản sao APFS copy-on-write riêng, không dùng chung thư mục target có thể ghi.

Đĩa thiếu chỗ: chỉ dọn `target/debug` ở hai checkout cũ #260/#262 sau khi xác minh thread idle,
checkout sạch, không có process/open file dùng chúng. Giữ source/WIP, DB và mọi evidence/capture.

## Kết quả theo boundary

| Kiểm tra | Source / kết quả | Evidence |
| --- | --- | --- |
| Baseline `cargo test --locked -p vink-app --test postgres long_document_http_boundaries_preserve_durable_text_and_conflicts -- --nocapture` | Base; **PASS 1/1**, expectation cũ xác nhận tiny_blocks200k bị 422 | [log](baseline-http-retry.log) |
| `cargo test --locked -p vink-app --test document_budget` | `4778e19`; **PASS 4/4**, 21 corpus/golden hash v1, Unicode decomposition tối đa 4, client/provider DTO | [log](corpus.log) |
| `cargo xtask verify quick` | `4778e19`; **PASS 13/13 bước**; App lib 344, Web 240 + 1 ignored, xtask 83 + 1 ignored | [log](quick.log) |
| `cargo xtask verify postgres` | `4778e19`; **PASS 29/29 bước**; Store 6, App 209 + 7 ignored, Identity 48; roles, worker/binaries, backup/restore thật | [log](postgres.log) |
| Loopback HTTP/1 + PostgreSQL `draft_http_budget::*` | Cùng source trong lane PostgreSQL; **PASS 2/2**. Corpus/authority/refusals, Content-Length và chunked, no-store, readback/hash/version; test header chỉ là principal seam | [log có byte/timing của lượt phát triển](focused-http.log), [final lane](postgres.log) |
| `cargo xtask build web` | **PASS**, cùng nội dung runtime đã commit; cache npm riêng | [log](build-web-retry.log) |
| `cargo test --locked -p vink-app --test postgres document_budget::draft_http_budget_browser -- --ignored --exact --nocapture` | `4778e19`; **PASS 1/1**, phase `document-budget`: 9 Reader scenes, 12 editor scenes, 3 corpus | [log](browser.log), [JSON có source SHA/browser](browser.json) |
| `cargo xtask verify web --scenario main` | `4778e19`; **PASS 6/6 bước, 5.861 assertions**, SSR/hydration/manual save/reload/conflict/leave/preview/tags | [log](web-main.log) |
| Runtime request cap, lặp sau khi build sẵn | `4778e19`; **PASS 1/1**, chỉ test `draft_http_budget::draft_http_budget_limits_streamed_and_declared_bodies_without_writes` | [log + time/RSS](http-resource.log) |
| Decision query | **PASS**: USER-09 r1 PRODUCT_CONFIRMED; VDR source r1 trên base vẫn PENDING; không signoff mới | [USER-09](decision-user09.log), [source r1](decision-source-r1.log) |
| GitHub CI, production deploy/data/config, thiết bị thật/OS IME/Safari/Firefox, full Web auth/restart phases | **NOT_RUN**, không thuộc claim của lượt này | Browser headless với synthetic input; không chứng nhận production hoặc native |

Regression mới đầu tiên cũng PASS (1 pure + 2 HTTP) trong working tree trước commit; final corpus
và full PostgreSQL ở bảng trên mới là evidence của source đã pin. Không dùng baseline PASS để nói
lỗi đã sửa: baseline còn cố ý chờ 422, sau sửa mới yêu cầu tiny_blocks200k trả 200.

## Byte, CPU và bộ nhớ quan sát

- Browser manual save thật: NFD/30k **67.936 B**, thơ/tab/200k **470.091 B**, tiny_blocks/200k
  **3.200.101 B**; cả ba `saved`, version +1, exact NFC readback và full live buffer giữ nguyên.
  Tiny blocks ACK khoảng **629,6 ms** trên debug build của lượt này. Cả ba UI conflict giữ buffer;
  tiny blocks thêm ca 200.001 scalar: UI failed, full buffer và durable state/version giữ nguyên.
- Warm request-cap test: không có `Compiling`, Cargo finished 0,12 s; test 1,60 s.
  Toàn lệnh `/usr/bin/time -l ./tools/local-budget run -- cargo test …` là **1,85 s wall,
  1,26 s user, 0,15 s sys**. `time` báo maximum RSS **253.050.880 B (~241,3 MiB)**;
  launcher sampled owned-session RSS sum **339.968 KiB (332 MiB), 2 mẫu cách 1 s**.
  Đây gồm client/server test cùng process và launch chain, không gồm PostgreSQL chạy trước;
  số cộng RSS có thể đếm shared pages nhiều lần, không phải RAM riêng hoặc hard per-request bound.
- Request chính xác 8.388.608 B được nhận, +1 B trả 422: khoảng **1,99 ms** có length,
  **3,68 ms** chunked. Fixture 8.388.605 B chứa khoảng 2,8 triệu line rỗng bị domain từ chối
  trong **313,3 ms**, durable state không đổi. Limit−1/limit cũng PASS với cả hai framing.
- Lượt đo [đầu](http-resource-first.log) còn compile/link (14,49 s), không dùng peak của nó
  làm RSS runtime. Các số là quan sát local/synthetic, không phải SLA hoặc test concurrency/load.

## Failure môi trường đã giữ nguyên

1. [Baseline compile FAIL](baseline-http.log): linker errno=28 hết đĩa, **chưa chạy assertions**.
   Dọn artifact đã xác minh không dùng, chạy lại baseline PASS.
2. [Web build FAIL](build-web.log): npm home cache EACCES/EEXIST; WASM đã build nhưng bundler
   chưa xong. Chuyển `npm_config_cache` sang thư mục riêng của task, retry build PASS.

Không có failure source còn tồn tại. Captures/browser fixture đầy đủ được giữ ngoài target tại
`/Users/manhblue/Documents/Codex/2026-11-18/task/evidence/browser`; captures Web main nằm trong
worktree `target/verify-web`. Lượt này dựa vào assertions của harness, không tuyên bố đã review
từng screenshot. [SHA-256 manifest](SHA256SUMS) bao gồm toàn bộ raw logs/JSON được commit.

## Acceptance và bàn giao #285

AC-01..05 **PASS trong scope v1 local**: phép tính/serializer, HTTP save/readback/hash/version,
cap hữu hạn/refusal/resources, authority/conflict/buffer và các lane + rollback đều có evidence.
Không có phụ thuộc kỹ thuật bắt buộc vào #285 để sửa v1. Khi tích hợp #285 phải đổi tests/corpus/
HTTP DTO/title handling/docs và tính lại bound cho `{format, source}`, rồi chạy lại quick,
PG/HTTP/corpus/browser trên head tích hợp. Không kế thừa bound v1 thành chứng minh canonical,
không rehash/sửa lịch sử bất biến, không giả signoff r2. Giữ PR riêng ở Draft để parent review;
không merge hoặc deploy trong lượt tạo PR này.
