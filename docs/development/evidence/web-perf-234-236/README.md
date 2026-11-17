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

[Lượt cuối release](release-guest-final.json.gz) tại `959231f` dùng runner sau sửa #236 và
assertion phân trang chặt hơn: **20 root / 40 comment nodes** trong cả 10 lượt warm. `discovery`
và `perf-baseline` cũng PASS cùng lệnh `--scenario discovery --scenario perf-baseline --release`
tại `eee78b7`. Node analyzer 4/4, syntax JS, rustfmt và `verify unit xtask` 83 PASS / 1 ignored.

## #235 — ranh giới Reader SSR

[`main` release](reader-ssr-boundary.json.gz) tại `49584d6`, App/PostgreSQL thật: bootstrap blocked,
held 200/1.000 ms, WASM-only held 200/1.000 ms và 503. Chờ JS đã tải xong và loader gọi WASM
trước khi thử control; article/link còn, thao tác ghi không chạy hoặc replay, hydrate fresh có
0 GET `/reader` trùng. Blocked-bootstrap kiểm thêm native GET search và marker draft vắng mặt.

Cùng lệnh `cargo xtask verify web --scenario main --scenario discovery --scenario review --release`
đã PASS cả ba scenario. Review dùng App/PG thật cho Hidden/ReviewWithdrawn và công khai lại.
Giữ Async dựa trên [ma trận SSR đã có và ranh giới đã review](../../reader-ssr-boundary.md).
Streaming B/C, proxy chunking, thử rollback từ mode khác và OS a11y `NOT_RUN`.

## #236 — URL, vị trí và focus khi quay lại Khám phá

**Chọn A:** giữ URL anchor; sửa đồng bộ lịch sử và giới hạn callback phục hồi theo scope. Không
thêm offset/keep-alive. [LayoutSpec](../../../design/discovery-return-position-236.md) sở hữu
dung sai theo mép hàng và `scroll-margin`; fallback là vùng kết quả có tên của #258.

- **Trước, runtime base, harness `0dde28f`: FAIL.** [Trace](return-before.json.gz),
  [log](return-before-failure.log), [ảnh lỗi đã mở](before-reader-with-discovery-url.png).
  Back và Forward hiển thị đúng hàng, nhưng mở lại cùng Reader sau Back → Forward → Back
  mount article trong khi browser URL vẫn là `/`. Raw `history.replaceState` đã đổi URL
  mà không cập nhật path stack của Leptos 0.8.17. Vì vậy cố mở lại Reader trùng đích ở đỉnh
  stack có thể bị bỏ push URL. Lượt trước dừng tại đây nên không có chuỗi 50 để so timing.
- **Sau, `eee78b7`: PASS** cùng `cargo xtask verify web --scenario return-position --release`,
  [trace 50 vòng](return-after.json.gz). Router replace giữ state hiện có cho fragment;
  Back/Forward/reopen và return link giữ q/thẻ/cursor/hàng. Frame thuộc đúng kết quả đã settle,
  được hủy khi scope đổi; không chạy lại chỉ vì list refresh. Missing row/503 dùng landmark.
  Lượt #236 chậm/503 là can thiệp đúng transport list, không là test quyền truy cập.
- Cả 50 vòng chỉ có view đang dùng; max lệch 0,016 CSS px (spec ≤2). Sau GC ở vòng 10–50:
  917 DOM nodes, 79 JS listeners, 4 documents; V8 used khoảng 2,20→2,24 MB. Embedder heap proxy
  tăng khoảng 2,82→4,47 MB; từ mẫu này chưa kết luận mọi RAM/timer/owner đều ổn định, không suy
  browser bitmap eviction. Không dùng vòng 50 làm timing gate.

| Chiều | Kết quả trên Chrome release | Bằng chứng và giới hạn |
| --- | --- | --- |
| Functional | PASS | Back/Forward/reopen, return link, modifier mở tab mới không sửa URL cũ, offline warm và fallback. |
| Geometry | PASS | Desktop 1280×900 và 390×844 chữ 200% sau resize bật reduced motion; đo khi font sẵn sàng, không forced delay font/image. |
| Visual | PASS trong phạm vi regression | Đã mở [trước](before-desktop.png) và [sau](after-desktop.png) cùng cảnh; ring keyboard, hàng trong viewport, không thay bố cục; `NO_APPROVED_REFERENCE`. |
| Accessibility/Responsive | PASS trong lab CSS | [Ảnh 390px chữ 200%](after-narrow-text200.png), focus keyboard và landmark có tên; chữ 200% là root font override. OS assistive tech chưa chạy. |
| Interaction | PASS | [Ảnh sau đáp án chậm](after-delayed-read.png) giữ caret/chuỗi đang gõ. Main trước sửa và discovery sau sửa vẫn kiểm fence READ-02 và late result. |

Web `discovery` đã PASS lại sau sửa #236 trên `eee78b7`. Đối chứng removal/late Reader từ
main và authority App/PG ở review vẫn riêng với logic cuộn; không có transition/guard domain
thay đổi cần model mới. Inject font/image, đo eviction thật trên thiết bị yếu và OS a11y còn
`NOT_RUN`. Nếu cần rollback sửa #236, hoàn tác commit sửa router/focus về URL anchor cũ;
trace trước giữ cả giới hạn của baseline đó.
