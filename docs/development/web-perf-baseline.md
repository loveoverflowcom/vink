# Web Performance #234 — baseline Khám phá → Reader → Back

**Hiện trạng:** Scenario `perf-baseline` trong catalog `xtask` và harness CDP hiện hữu, chạy
opt-in với App/PostgreSQL scratch thật. Baseline release của khách đã chạy 10 cold + 10 warm.
Số liệu và cấu hình ở [evidence #234–#236](evidence/web-perf-234-236/README.md). Lượt debug
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
  thảo luận, bytes tài liệu/bundle, long task, layout shift và Event Timing (≥16 ms) khi Chrome
  hỗ trợ. Milestone warm chứa chi phí CDP/poll. DOM/listener counters và V8 heap chỉ là proxy;
  số owner/timer trực tiếp, RSS và WASM heap attribution chưa đo. Báo median/range/p95 lab;
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

## Hiện trạng acceptance

| AC | Bằng chứng có | Giới hạn |
| --- | --- | --- |
| AC-01 | Release/wasm-release, browser/build SHA/seed digest, viewport/DPR/font, 200/1.000 qua App scratch | Chưa đo corpus worst-case #229 trong cùng hành trình; lab một máy |
| AC-02 | 0 list GET khi hydrate fresh; history document verify đúng một lần; ledger main lưu stale/manual/late/removal/offline và wire 304 thật | Ledger không phải trace SQL/flight nội bộ; main dùng fixture khác; nhiều ca race/removal có transport injection đã gắn nhãn |
| AC-03 | 10+10 raw với median/range/p95, readiness, bytes, interaction, DOM/listener/heap proxy | Không có RSS, timer/owner trực tiếp, thiết bị thật |
| AC-04 | Inject một GET dư để oracle FAIL; giữ/hỏng secondary không làm rơi thân bài, retry dùng App thật; main giữ ABA/disposal; #236 có lặp navigation | Các đáp án late/503 tổng hợp không chứng minh authority phía database; xem Review thật ở [#235](reader-ssr-boundary.md) |
| AC-05 | Số đo và go/no-go bên dưới, opt-in | Chưa đo chi phí riêng của observer; không chạy timing gate |

- **SSR selective streaming/defer (#235): chưa làm.** Trên cùng hành trình, thành công của main
  không cần browser secondary trả lời. Bằng chứng SSR trực tiếp hiện bảo vệ Async; chưa có
  lợi ích meaningful article hay số liệu proxy cho B/C. Giữ A.
- **Khôi phục vị trí (#236): tiếp tục với URL anchor.** Giữ sửa focus đã tái hiện; chỉ xem offset
  hoặc view giữ lại nếu có drift có cơ sở và một chi phí tương xứng.
- **RAM (#237): cần đo thêm** trên thiết bị mục tiêu. File release thử nghiệm có 109.799.191 B,
  gồm 104.614.722 B custom section `name` (đọc section từ artifact); xét riêng ngân sách ký hiệu
  và vận chuyển bundle trước khi quy vấn đề cho scheduling SSR hay retention của view.
- **Containment (#238): chưa có bằng chứng** rằng layout/style là nút thắt trên thiết bị mục tiêu.
- **Prefetch (#239): chưa bật** khi chưa có byte ceiling và lợi ích điều hướng đủ lớn ở #237.
