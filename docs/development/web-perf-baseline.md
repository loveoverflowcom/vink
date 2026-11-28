# Web Performance #234 — baseline Khám phá → Reader → Back

**Hiện trạng:** Scenario `perf-baseline` trong catalog `xtask` và harness CDP hiện hữu, chạy
opt-in với App/PostgreSQL scratch thật. Baseline release của khách đã chạy 10 cold + 10 warm.
Số liệu và cấu hình ban đầu ở [evidence #234–#236](evidence/web-perf-234-236/README.md). Lượt debug
[`997d97d`](evidence/web-perf-234/README.md) là lịch sử, khác cả profile và phiên khách, không dùng
làm mốc để tuyên bố tăng tốc. Đây là kiểm thử lab, không phải RUM hay SLA.

## Phương pháp

- `verify web --scenario perf-baseline --release` build App release và bundle `wasm-release` bằng
  planner/executor hiện có. Bỏ `--release` chọn dev, không dùng chung hai profile trong một lượt.
  Full Web không tự chạy scenario này. Rollback phần đo bằng cách bỏ selector.
- App chạy không có LocalFixture principal cho baseline công khai. #100 seed 14 work;
  [fixture #234](../../apps/vink-web/e2e/fixtures/web-perf-journey.sql) thêm 186 work theo document
  v1 (cùng encoding đã đối chiếu ở #229) và 1.000 nhận xét trong một thread (800 gốc + 200 phản
  hồi). PostgreSQL kiểm số đếm. Trình duyệt giữ page size thật, **40 comment nodes** trong lượt
  đã đo; không mount cả corpus. Main kiểm actor/own/capability trong phiên fixture riêng.
- Cold: xóa HTTP cache trước một document mới tại đường tìm kiếm có `q=kiem thu`. OS, PostgreSQL
  và process Chrome vẫn nóng. Warm: liên kết mở hot Reader rồi liên kết trở về đúng scope trên
  cùng document. Control history dựng document thật có Navigation Timing `back_forward`, tắt
  BFCache riêng cho scenario; đừng gộp ca này với Back trong SPA.
- Ledger ghi **intent quan sát được** (điều hướng, refresh, retry, invalidation) và từng
  **transport attempt** theo loại/key ẩn danh. Không suy số logical flight nội bộ từ số GET.
  `validator_sent` dùng thêm request ExtraInfo; `wire_status` có thể là 304 khi browser giao 200.
  Chuỗi redirect giữ từng hop nhưng để wire status/validator chưa xác định nếu ExtraInfo mơ hồ.
  Catalog, actor, own và endorsement tách khỏi Reader/list. Path dữ liệu mẫu, tên tham số và mã
  key là đủ cho đối soát; không ghi giá trị query, header, cookie hoặc body.
- Raw RUM-free: TTFB, thời điểm hàng đầu tiên/hydration, main text Reader, readiness của reload
  thảo luận, bytes tài liệu/bundle (bao gồm raw, `gzip -6`, `brotli q6`, `brotli q11` và SHA-256
  cho `vink.css`, `vink_bg.wasm`, `vink.js`, `vink-editor.js` cùng kích thước SSR HTML thực của
  `discovery_filtered` và `reader_hot` từ #417), long task, layout shift và Event Timing (≥16 ms) khi Chrome
  hỗ trợ. Milestone warm chứa chi phí CDP/poll. DOM/listener counters và V8 heap chỉ là proxy;
  probe release đo owner/timer trực tiếp. RSS và WASM heap attribution có giới hạn ghi ở báo cáo. Báo median/range/p95 lab;
  không dùng p95 với 10 mẫu làm gate, không suy field p75.

~~~sh
./tools/local-budget run -- cargo xtask verify web --scenario perf-baseline --release --plan
cargo xtask verify web --scenario perf-baseline --release
cargo xtask verify web --scenario main --scenario discovery --scenario review --release
node --test apps/vink-web/e2e/web-perf-baseline.test.mjs
~~~

Trên macOS: launcher dùng được cho build, unit và planner. Lane có chạy Chrome dùng lệnh trực
tiếp theo [local-resource-budget](local-resource-budget.md). Runtime cần `VINK_TEST_DATABASE_URL`
đến PostgreSQL scratch thật; đích output là `target/verify-web/<run-id>/`, không có deployed collector.

## Kết quả release ngày 18/11/2026

Năm AC đã được kiểm trên corpus ngắn và corpus 200.000 đoạn. Median click → article khi Reader chưa cache là 43,05 ms ở bài ngắn và 1.240,55 ms ở bài dài trong phép giữ discussion bằng CDP. Phần release hiện đo trực tiếp số owner và timer của READ-02. Chi phí thân hai observer trên lượt cold bài ngắn có median 0,85 ms. [Số đo, giới hạn, cách chạy và kết luận từng AC](evidence/web-perf-234-canonical/README.md).

## Tải lại và mở lại trên production (#473)

“Warm” ở trên là điều hướng SPA trong cùng document, không phải reload hay mở lại. Ngày 24/11/2026
đã đo normal reload, mở lại tab, mở lại trình duyệt, Back, hard reload và SPA trên release live
`e9872556`, kèm so sánh với Cantos, phân rã TTFB theo chặng và đối chứng nén cục bộ.
[Số đo, kết luận và đề xuất sửa](evidence/web-reload-473/README.md).

## Nhận xét Markdown (#618)

Corpus Reader gồm 4.000 Unicode scalar tiếng Việt và emoji trong 200 dòng. Đo textarea SSR, thời gian từ nhấn Định dạng đến Comment view, input/caret trên trang tác phẩm và kích thước asset thực từ release. Xem [hồ sơ cùng máy trước và sau](evidence/discussion-markdown-618/README.md).
