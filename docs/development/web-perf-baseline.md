# Web Performance #234 — baseline Khám phá → Reader → Back (opt-in)

**Trạng thái implementation:** scenario/fixture/ledger đã được thêm ở PR cho #234; **baseline runtime chưa được chạy và chưa được nghiệm thu**. Không biến source inspection hoặc unit analyzer thành số liệu Chrome/PostgreSQL. Đây là kiểm thử **lab**, không phải RUM hay SLA.

## Vì sao đo trước khi tối ưu

- #233/PR #241 đã merge (READ-02 owner/read ledger); #229/PR #232 đã merge (document-v1 corpus, serialization, budget). Phần đo của #229 không tự chứng minh latency/DOM/JS của luồng Khám phá–Reader–Back.
- Bám browser-smoke.mjs và planner/executor xtask; không thêm runner, store, cache owner, migration, SSR mode hoặc đổi policy.
- Opt-in bằng tên scenario perf-baseline. Full Web regression không tự thêm 20 navigation. Cùng catalog/doctor/plan, scratch isolation và artifact lock của #184.
- Lựa chọn kỹ thuật: **CHOICE-01 A** (scenario riêng trong catalog), **CHOICE-02 A** (distribution/noise; chỉ gate structural oracle, chưa gate ngưỡng timing). Không có signoff sản phẩm/ADR mới.

## Tập dữ liệu và đường chạy

xtask tạo database vink_verify_web mới bằng VINK_TEST_DATABASE_URL, chạy migrate + dev-seed, thêm tag catalog bằng operator thật. #100 seed 14 publication; fixture [web-perf-journey.sql](../../apps/vink-web/e2e/fixtures/web-perf-journey.sql) thêm 186 để có **200 work tổng hợp** (không tính dev-seed) và **1.000 comment** (800 root + 200 flat reply) trong một hot thread. PostgreSQL SELECT COUNT kiểm chính xác hai số trước khi chạy browser; lỗi là FAIL. Hợp đồng document v1 dùng JSONB + canonical content_hash và metadata snapshot theo seed #100 (cùng serializer v1 được kiểm bởi #229), không phân khối và không mount toàn corpus.

Trước mỗi vòng cold, CDP Network.clearBrowserCache; điều đó **không** làm lạnh OS, bộ đệm PostgreSQL, binary, process hay profile Chrome. Mười vòng cold mở tài liệu mới có SSR và hydration từ đường tìm kiếm có q=kiem%20thu; mười vòng warm dùng liên kết Reader thật rồi nút trở lại thật trên cùng tab. Mỗi trang công khai giữ page size thực (12 mặc định, giới hạn backend tối đa 24). Bình luận 1.000 dòng chỉ nằm trong SQL; không dựng hết trong DOM.

~~~sh
cargo xtask verify web --scenario main --plan
cargo xtask verify web --scenario discovery --plan
cargo xtask verify web --scenario perf-baseline --plan

# Cần local test PostgreSQL (CREATEDB), Node, wasm toolchain và Chrome.
# Không tự khởi tạo container, không dùng DATABASE_URL dev để reset.
cargo xtask verify web --scenario perf-baseline

# Oracle thống kê/ledger nhỏ, không chứng minh rendering hay SQL.
node --test apps/vink-web/e2e/web-perf-baseline.test.mjs
node --check apps/vink-web/e2e/browser-smoke.mjs
node --check apps/vink-web/e2e/web-perf-baseline.mjs
~~~

Output: target/verify-web/<run-id>/web-perf-baseline.json, server.log và log xtask. source_sha, SHA-256 từ *hai SQL fixtures*, browser version, Node, width/DPR/font, network/CPU label, build profile và iteration có mặt trong artifact. Chỉ dùng manuscript giả. Traces không ghi cookie, header value, query value, request/response body hay tên người dùng; request path + tên tham số đủ phân loại public-list, reader, public-discussion, own-discussion, catalog, bundle. Các event có phase, validator_sent, response_status, wire_status (gồm 304 nếu CDP trả về), encoded bytes. intended_reads là **user intent**, *không* giả làm số logical reads nội bộ; transport attempts là CDP-visible và không đo SQL do server SSR phát sinh.

Mỗi iteration ghi raw navigation.ttfb_ms, thời điểm meaningful-content/hydration có nhãn, resource bundle transfer, long-task/layout-shift observation, số DOM nodes/jsEventListeners từ CDP, V8 heap proxy, số hàng/comment thực đang mount. Tổng kết count/min/max/median/nearest-rank lab p95/spread cho cold và warm; **không gọi 10 mẫu lab là field p75**. Không đổi threshold để biến số xấu thành PASS. Nếu Browser không hỗ trợ metric nào, ghi null và nêu giới hạn thay vì 0.

## Acceptance, bằng chứng, giới hạn

| AC #234 | Phần có implementation | Ranh giới chưa kiểm chứng |
| --- | --- | --- |
| AC-01 | Digest fixture, pinned source SHA, browser/config và SQL counts; scratch fixture | Chưa chạy trên release build; fixture #229 worst-case cần chạy cùng config |
| AC-02 | CDP transport ledger riêng với user intents; If-None-Match/wire 304 có nhãn | Chưa có app-logical-read event từng owner; stale/manual/invalidation/offline chưa được scenario mới tái lập đầy đủ |
| AC-03 | 10 cold + 10 warm raw; TTFB/content/hydration, bytes, long tasks, CLS, DOM/V8 proxy, distribution | Chưa đo production, browser-process RSS, WASM attribution, timer allocation; local Chrome/PG chưa chạy |
| AC-04 | Structural assertion list hydration không duplicate; unit analyzer có negative duplicate sample | Chưa chạy negative duplicate production, delayed secondary, late-response, repeat/dispose, thiết bị thật |
| AC-05 | Raw JSON output và decision-ledger PENDING; chỉ khi người chạy thực mới có số go/no-go | Chưa có raw baseline trên HEAD và before/after, chưa có signoff hay đủ acceptance |

**Không đóng #234 từ PR này chỉ vì module đã tồn tại.** Khi chạy trên máy đủ Rust/PostgreSQL/Chrome, lưu raw cùng config/lệnh/build SHA, kiểm toán lại mọi hạng mục, sửa lỗi implementation rồi mở bản review cuối. Nếu CHOICE-02 B (timing gate) được đề xuất sau đó, cần noise budget với số lượt đủ lớn và quyết định rõ trước khi thêm.

Quyết định tiếp theo: #235 kiểm SSR/no-JS/secondary response; #236 kiểm Back anchor/focus; #237 budget RAM; #238 containment; #239 prefetch phải chờ #237. Không dùng kết quả metadata-only hoặc page shell để biện minh cho kiến trúc streaming, chunking hay prefetch mới.
