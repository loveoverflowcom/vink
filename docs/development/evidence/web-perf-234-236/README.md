# Evidence Web Perf #234–#236 · 17/11/2026

Base của phép đo: `d352dba77c2a86d18d7afe5690fcb9b10153c055`, đã có #241, #251 và #265.
[TOML query](decisions.json) giữ current/head, nguồn và digest: READ-01/02, DISC-WEB r1
`DESIGN_PROPOSED`, O-03/O-17 r2 `PRODUCT_CONFIRMED`, offline-age r1 `PENDING`. Không đổi
SSR mode, policy cache/authority hoặc rollout offline; #236 chỉ đổi đồng bộ URL và focus.

Máy local arm64; Chrome headless 157.0.8103.79, Rust 1.99.1, wasm-bindgen 0.2.129, Node 26.12,
PostgreSQL 17.11 ICU trên cluster scratch loopback. `release` cho App, `wasm-release` cho client,
HTTP loopback, không throttle. Build qua `tools/local-budget`; Web browser dùng lane trực tiếp
theo [giới hạn launcher](../../local-resource-budget.md). Dùng `gzip -cd <file>.json.gz | jq …`
để đọc raw trace. Không có log nội dung riêng, cookie hoặc giá trị validator.

## #234 — baseline và lý do request

[`verify web --scenario perf-baseline --release`](release-guest.json.gz) PASS tại `49584d6`,
cây source sạch: 200 work / 1.000 nhận xét, 10 cold + 10 warm. SHA-256 fixture, browser, phiên,
font, viewport, bytes, Event Timing và số DOM/listener nằm trong JSON. Không dùng debug #251
để so timing vì khác profile và session. Kết quả release guest ở 1280×900, DPR 1:

| Mốc lab | Median ms | Khoảng min–max / p95 ms |
| --- | ---: | ---: |
| TTFB Khám phá cold | 1,95 | 1,8–6,9 / 6,9 |
| Hàng đầu tiên cold | 6,9 | 6,1–19,5 / 19,5 |
| Hydration cold | 215,3 | 190,5–336,3 / 336,3 |
| Mở Reader warm | 51,2 | 41,2–143,7 / 143,7 |
| Nút tải lại thảo luận sẵn sàng | 61,7 | 57,4–148,0 / 148,0 |
| Quay lại Khám phá warm | 37,0 | 29,9–42,2 / 42,2 |

**0** list GET lúc hydrate fresh. Đối chứng thêm **1** GET thật làm oracle fail như dự kiến.
Full-document `back_forward` revalidate list đúng một lần. Trì hoãn thảo luận tối thiểu 200 ms,
trả 503 tổng hợp không gỡ article; manual retry đến App thật và mount 40 comment nodes. Trace cũ
[fixture FAIL](release-fixture-first-fail.json.gz) tại `95cc126` giữ nguyên: manual retry một
lần nhưng LocalFixture mount lại và đọc thêm head để đối chiếu actor; raw có hai GET cùng key
và các own/capability read riêng. Sau đó chạy baseline bằng server khách để đo đúng hành trình
công khai. Điều kiện retry một head giữ nguyên; không dùng kết quả khách để kết luận fixture
không có chi phí đọc lại.

[Main ledger](public-read-lifecycle.json.gz) tại `49584d6`: lifecycle burst sau 121 giây và manual
retry mỗi ca có validator + wire 304, giao representation 200; có pha backoff, chậm A→B→A,
injected removal, offline và origin recovery. Fixture khác baseline khách; không lấy timing giữa
hai trace để so. `intent` kiểm thử, transport và SQL/owner flight là các tầng khác nhau. Chưa đo
chi phí riêng của observer, RSS hoặc số timer/owner trực tiếp. Xem [go/no-go](../../web-perf-baseline.md).

## #235 — ranh giới Reader SSR

[`main` release](reader-ssr-boundary.json.gz) tại `49584d6`, App/PostgreSQL thật: bootstrap blocked,
held 200/1.000 ms, WASM-only held 200/1.000 ms và 503. Chờ JS đã tải xong và loader gọi WASM
trước khi thử control; article/link còn, thao tác ghi không chạy hoặc replay, hydrate fresh có
0 GET `/reader` trùng. Blocked-bootstrap kiểm thêm native GET search và marker draft vắng mặt.

Cùng lệnh `cargo xtask verify web --scenario main --scenario discovery --scenario review --release`
đã PASS cả ba scenario. Review dùng App/PG thật cho Hidden/ReviewWithdrawn và công khai lại.
Giữ Async dựa trên [ma trận SSR đã có và ranh giới đã review](../../reader-ssr-boundary.md).
Streaming B/C, proxy chunking, thử rollback từ mode khác và OS a11y `NOT_RUN`.
